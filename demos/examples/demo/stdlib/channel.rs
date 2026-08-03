// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use std::cell::RefCell;
use std::collections::VecDeque;
use std::marker::PhantomData;
use std::rc::Rc;

/// A library-defined runtime channel identifier. Surface channel names resolve to this before
/// execution; the CPU and arithmetic components never see the symbolic name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ChannelId(usize);

#[derive(Debug, Clone, Copy)]
struct Send {
    channel: ChannelId,
}

#[derive(Debug, Clone, Copy)]
struct Recv {
    channel: ChannelId,
}

/// Identity used by the transport to return a completion to the endpoint that parked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EndpointId(u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ReceiveContinuation {
    endpoint: EndpointId,
    channel: ChannelId,
}

struct ReceiveCompletion<M> {
    continuation: ReceiveContinuation,
    value: M,
}

enum ReceivePoll<M> {
    Ready(M),
    Parked(ReceiveContinuation),
}

/// A transport is a capability supplied to a channel endpoint. It knows nothing about CPUs or
/// composite containment. Wakeups are owned by the transport and polled by the runtime root.
trait Transport<M> {
    type Fault;

    fn send(&mut self, channel: ChannelId, value: M) -> Result<(), Self::Fault>;

    fn receive(
        &mut self,
        endpoint: EndpointId,
        channel: ChannelId,
    ) -> Result<ReceivePoll<M>, Self::Fault>;

    fn take_wakeup(&mut self) -> Option<(ReceiveContinuation, M)>;
}

/// A reusable shared communication component. This demo uses immediate delivery; another
/// transport can implement latency or topology policy without changing `ChannelEndpoint` or `Cpu`.
struct ChannelFabric<M> {
    queues: Vec<VecDeque<M>>,
    waiters: Vec<Option<ReceiveContinuation>>,
    wakeups: VecDeque<(ReceiveContinuation, M)>,
}

impl<M> ChannelFabric<M> {
    fn with_channels(count: usize) -> Self {
        Self {
            queues: (0..count).map(|_| VecDeque::new()).collect(),
            waiters: vec![None; count],
            wakeups: VecDeque::new(),
        }
    }
}

impl<M> Transport<M> for ChannelFabric<M> {
    type Fault = std::convert::Infallible;

    fn send(&mut self, channel: ChannelId, value: M) -> Result<(), Self::Fault> {
        match self.waiters[channel.0].take() {
            Some(waiter) => self.wakeups.push_back((waiter, value)),
            None => self.queues[channel.0].push_back(value),
        }

        Ok(())
    }

    fn receive(
        &mut self,
        endpoint: EndpointId,
        channel: ChannelId,
    ) -> Result<ReceivePoll<M>, Self::Fault> {
        if let Some(value) = self.queues[channel.0].pop_front() {
            return Ok(ReceivePoll::Ready(value));
        }

        let continuation = ReceiveContinuation { endpoint, channel };
        // TODO: this currently assumes that only one endpoint can wait on a specific
        // channel at a time, so if two endpoints try to wait on the same channel,
        // say A is waiting, then B tries to listen, B will replace A and A will be
        // softlocked. change this to allow for multiple waiters on a single channel
        debug_assert!(self.waiters[channel.0].is_none());
        self.waiters[channel.0] = Some(continuation);
        Ok(ReceivePoll::Parked(continuation))
    }

    fn take_wakeup(&mut self) -> Option<(ReceiveContinuation, M)> {
        self.wakeups.pop_front()
    }
}

/// The capability copied into each endpoint. Cloning it shares the transport, not endpoint state.
#[derive(Clone)]
struct SharedTransport<M>(Rc<RefCell<ChannelFabric<M>>>);

impl<M> SharedTransport<M> {
    fn new(fabric: Rc<RefCell<ChannelFabric<M>>>) -> Self {
        Self(fabric)
    }
}

impl<M> Transport<M> for SharedTransport<M> {
    type Fault = std::convert::Infallible;

    fn send(&mut self, channel: ChannelId, value: M) -> Result<(), Self::Fault> {
        self.0.borrow_mut().send(channel, value)
    }

    fn receive(
        &mut self,
        endpoint: EndpointId,
        channel: ChannelId,
    ) -> Result<ReceivePoll<M>, Self::Fault> {
        self.0.borrow_mut().receive(endpoint, channel)
    }

    fn take_wakeup(&mut self) -> Option<(ReceiveContinuation, M)> {
        self.0.borrow_mut().take_wakeup()
    }
}

#[derive(Debug)]
enum SendEffect {}

#[derive(Debug)]
enum ReceiveEffect<M> {
    Received(M),
    Parked(ReceiveContinuation),
}

/// The component on which the communication instructions execute. Its transport is supplied at
/// construction, so its behavior is independent of where the CPU is placed in a machine.
struct ChannelEndpoint<M, T> {
    id: EndpointId,
    transport: T,
    parked: Option<ReceiveContinuation>,
    _message: PhantomData<fn() -> M>,
}

impl<M, T> ChannelEndpoint<M, T> {
    fn new(id: EndpointId, transport: T) -> Self {
        Self {
            id,
            transport,
            parked: None,
            _message: PhantomData,
        }
    }

    fn is_parked(&self) -> bool {
        self.parked.is_some()
    }
}

impl<M, T> Execute<Send> for ChannelEndpoint<M, T>
where
    T: Transport<M>,
{
    type Message = M;
    type Effect = SendEffect;
    type Fault = T::Fault;

    fn execute(
        &mut self,
        instruction: &Send,
        value: M,
    ) -> Result<StepResult<Self::Effect>, Self::Fault> {
        self.transport.send(instruction.channel, value)?;
        Ok(StepResult {
            effects: Effects::none(),
            execution: Execution::Complete,
        })
    }
}

impl<M, T> Execute<Recv> for ChannelEndpoint<M, T>
where
    T: Transport<M>,
{
    type Message = NoMessage;
    type Effect = ReceiveEffect<M>;
    type Fault = T::Fault;

    fn execute(
        &mut self,
        instruction: &Recv,
        _message: NoMessage,
    ) -> Result<StepResult<Self::Effect>, Self::Fault> {
        match self.transport.receive(self.id, instruction.channel)? {
            ReceivePoll::Ready(value) => Ok(StepResult {
                effects: Effects::one(ReceiveEffect::Received(value)),
                execution: Execution::Complete,
            }),
            ReceivePoll::Parked(continuation) => {
                self.parked = Some(continuation);
                Ok(StepResult {
                    effects: Effects::one(ReceiveEffect::Parked(continuation)),
                    execution: Execution::Parked,
                })
            }
        }
    }
}

impl<M, T> Resume<ReceiveCompletion<M>> for ChannelEndpoint<M, T> {
    type Effect = ReceiveEffect<M>;
    type Fault = std::convert::Infallible;

    fn resume(
        &mut self,
        completion: ReceiveCompletion<M>,
    ) -> Result<StepResult<Self::Effect>, Self::Fault> {
        debug_assert_eq!(self.parked, Some(completion.continuation));
        self.parked = None;
        Ok(StepResult {
            effects: Effects::one(ReceiveEffect::Received(completion.value)),
            execution: Execution::Complete,
        })
    }
}

#[cfg(test)]
mod channel_tests {
    use super::*;

    #[test]
    fn queued_values_are_fifo() {
        let fabric = Rc::new(RefCell::new(ChannelFabric::with_channels(1)));
        let mut endpoint = ChannelEndpoint::new(EndpointId(0), SharedTransport::new(fabric));

        endpoint
            .execute(&Send { channel: ChannelId(0) }, 10)
            .unwrap();
        endpoint
            .execute(&Send { channel: ChannelId(0) }, 20)
            .unwrap();

        let first = endpoint
            .execute(&Recv { channel: ChannelId(0) }, NoMessage)
            .unwrap()
            .effects
            .into_iter()
            .next();
        assert!(matches!(first, Some(ReceiveEffect::Received(10))));

        let second = endpoint
            .execute(&Recv { channel: ChannelId(0) }, NoMessage)
            .unwrap()
            .effects
            .into_iter()
            .next();
        assert!(matches!(second, Some(ReceiveEffect::Received(20))));
    }

    #[test]
    fn send_execute_wakes_a_parked_recv_execute() {
        let fabric = Rc::new(RefCell::new(ChannelFabric::with_channels(1)));
        let mut receiver = ChannelEndpoint::new(EndpointId(1), SharedTransport::new(fabric.clone()));
        let mut sender = ChannelEndpoint::new(EndpointId(0), SharedTransport::new(fabric.clone()));

        let parked = receiver
            .execute(&Recv { channel: ChannelId(0) }, NoMessage)
            .unwrap()
            .effects
            .into_iter()
            .next();
        assert!(matches!(parked, Some(ReceiveEffect::Parked(_))));
        assert!(receiver.is_parked());

        sender
            .execute(&Send { channel: ChannelId(0) }, 42)
            .unwrap();

        let (continuation, value) = receiver.transport.take_wakeup().unwrap();
        let effects = receiver
            .resume(ReceiveCompletion { continuation, value })
            .unwrap()
            .effects;
        assert!(matches!(
            effects.into_iter().next(),
            Some(ReceiveEffect::Received(42))
        ));
        assert!(!receiver.is_parked());
    }
}
