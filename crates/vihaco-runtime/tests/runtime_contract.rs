// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco_runtime::{
    Absorb, Effects, Execute, Execution, Handle, NoMessage, Observe, StepResult, Supply,
};

#[derive(Debug, PartialEq, Eq)]
struct Message(u8);

#[derive(Debug, PartialEq, Eq)]
struct Effect(u8);

#[derive(Debug, PartialEq, Eq)]
struct Fault;

#[derive(Debug, PartialEq, Eq)]
struct Route;

#[derive(Default)]
struct Component {
    supplied: u8,
    absorbed: Vec<u8>,
    observed: Vec<u8>,
    handled: Vec<u8>,
}

impl Supply<Message> for Component {
    type Fault = Fault;

    fn supply(&mut self) -> Result<Message, Self::Fault> {
        Ok(Message(self.supplied))
    }
}

impl Absorb<Effect> for Component {
    type Fault = Fault;

    fn absorb(&mut self, effect: Effect) -> Result<(), Self::Fault> {
        self.absorbed.push(effect.0);
        Ok(())
    }
}

impl Observe<Effect, Route> for Component {
    type Effect = ();
    type Error = Fault;

    fn observe(&mut self, effect: &Effect) -> Result<Effects<Self::Effect>, Self::Error> {
        self.observed.push(effect.0);
        Ok(Effects::none())
    }
}

impl Handle<Effect, Route> for Component {
    type Error = Fault;

    fn handle(&mut self, effect: Effect) -> Result<(), Self::Error> {
        self.handled.push(effect.0);
        Ok(())
    }
}

impl Execute<u8> for Component {
    type Message = NoMessage;
    type Effect = Effect;
    type Fault = Fault;

    fn execute(
        &mut self,
        instruction: &u8,
        _message: Self::Message,
    ) -> Result<StepResult<Self::Effect>, Self::Fault> {
        Ok(StepResult {
            effects: Effects::one(Effect(*instruction)),
            execution: Execution::Complete,
        })
    }
}

#[test]
fn execute_returns_effects_and_execution_state() {
    let mut component = Component::default();
    let result = component.execute(&7, NoMessage).unwrap();

    assert_eq!(result.effects, Effects::one(Effect(7)));
    assert_eq!(result.execution, Execution::Complete);
}

#[test]
fn supply_absorb_observe_and_handle_are_route_capabilities() {
    let mut component = Component {
        supplied: 3,
        ..Component::default()
    };

    assert_eq!(component.supply().unwrap(), Message(3));

    let effect = Effect(9);
    component.observe(&effect).unwrap();
    component.absorb(effect).unwrap();
    component.handle(Effect(11)).unwrap();

    assert_eq!(component.observed, vec![9]);
    assert_eq!(component.absorbed, vec![9]);
    assert_eq!(component.handled, vec![11]);
}

#[test]
fn execution_has_complete_and_parked_states() {
    assert_ne!(Execution::Complete, Execution::Parked);
}
