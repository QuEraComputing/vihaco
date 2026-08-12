use std::{collections::HashMap, convert::Infallible};

use vihaco::{Execute, NoEffect, NoMessage, StepResult};

vihaco::component! {
    pub component CounterGroup {
        queued: Vec<QueuedCounter>,
        playing: HashMap<CounterId, PlayedCounter>,
        next_id: CounterId,
    }

    runtime {
        instruction {
            #[derive(Debug, Clone)]
            Queue { start: u32, duration: u32 },
            #[derive(Debug, Clone)]
            Play,
        }
    }
}

use counter_group::*;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CounterId(pub u32);

impl std::ops::AddAssign<u32> for CounterId {
    fn add_assign(&mut self, rhs: u32) {
        self.0 += rhs
    }
}

struct QueuedCounter {
    id: CounterId,
    count: u32,
    duration: u32,
}

struct PlayedCounter {
    count: u32,
    time_left: u32,
}

impl QueuedCounter {
    fn increase(&mut self) {
        self.count += 1;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CounterResult {
    Advanced(u32),
    Done(u32),
}

#[derive(Debug, Default)]
pub struct PlayReport {
    pub results: HashMap<CounterId, CounterResult>,
}

impl CounterGroup {
    pub fn new() -> Self {
        Self {
            queued: vec![],
            playing: HashMap::new(),
            next_id: CounterId(0),
        }
    }

    fn queue(&mut self, start: u32, duration: u32) {
        self.queued.push(QueuedCounter {
            id: self.next_id,
            count: start,
            duration,
        });
        self.next_id += 1;
    }

    /// Move queued channels into the active set. The machine owns the clock; this component
    /// only owns channel lifetime and evaluation state.
    pub fn play(&mut self) -> usize {
        let played = self.queued.drain(..).collect::<Vec<_>>();
        let mut started = 0;

        for c in played {
            if c.duration == 0 {
                continue;
            }
            let new_c = PlayedCounter {
                count: c.count,
                time_left: c.duration,
            };
            self.playing.insert(c.id, new_c);
            started += 1;
        }

        started
    }

    /// Evaluate every active channel once. A future waveform implementation can use the same
    /// boundary while accepting an elapsed-time/sample-rate argument.
    pub fn advance(&mut self) -> PlayReport {
        let mut results = HashMap::new();

        let ids = self.playing.keys().copied().collect::<Vec<_>>();
        for id in ids {
            let counter = self
                .playing
                .get_mut(&id)
                .expect("active counter disappeared during advancement");
            counter.count += 1;
            counter.time_left -= 1;

            if counter.time_left == 0 {
                results.insert(id, CounterResult::Done(counter.count));
                self.playing.remove(&id);
            } else {
                results.insert(id, CounterResult::Advanced(counter.count));
            }
        }

        PlayReport { results }
    }

    pub fn is_playing(&self) -> bool {
        !self.playing.is_empty()
    }
}

impl Execute<runtime::instruction::Queue> for CounterGroup {
    type Message = NoMessage;
    type Effect = NoEffect;
    type Fault = Infallible;

    fn execute(
        &mut self,
        instruction: &runtime::instruction::Queue,
        _message: Self::Message,
    ) -> Result<vihaco::StepResult<Self::Effect>, Self::Fault> {
        self.queue(instruction.start, instruction.duration);
        vihaco::complete!()
    }
}

#[derive(Debug)]
pub struct Playing {
    pub channels: usize,
}

impl Execute<runtime::instruction::Play> for CounterGroup {
    type Message = NoMessage;
    type Effect = Playing;
    type Fault = Infallible;

    fn execute(
        &mut self,
        _instruction: &runtime::instruction::Play,
        _message: Self::Message,
    ) -> Result<StepResult<Self::Effect>, Self::Fault> {
        let channels = self.play();
        vihaco::complete!(Playing { channels })
    }
}
