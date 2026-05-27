use eyre::Result;
use vihaco::__private::GeneratedMachine;
use vihaco::{Effects, Instruction, Message, component, composite, observe};

#[derive(Debug, Clone, Instruction)]
enum SchedulerInst {
    Acquire,
    Release,
}

#[derive(Debug, Clone, Instruction)]
enum DeviceInst {
    Pulse,
}

#[derive(Message)]
struct DeviceMsg(&'static str);

#[derive(Debug)]
struct SharedEffect(&'static str);

#[derive(Default)]
struct Core;

#[component(instruction = (), message = ())]
impl Core {
    fn execute(&mut self, _inst: (), _msg: ()) -> Result<Effects<()>> {
        Ok(Effects::none())
    }
}

#[derive(Default)]
struct SharedDevice(Vec<&'static str>);

#[component(instruction = DeviceInst, message = DeviceMsg, effect = SharedEffect)]
impl SharedDevice {
    fn execute(&mut self, inst: DeviceInst, msg: DeviceMsg) -> Result<Effects<SharedEffect>> {
        match inst {
            DeviceInst::Pulse => {
                self.0.push(msg.0);
                Ok(Effects::one(SharedEffect(msg.0)))
            }
        }
    }
}

#[derive(Default)]
struct SharedObserver(Vec<&'static str>);

#[observe(SharedEffect)]
impl SharedObserver {
    fn observe_shared_effect(&mut self, effect: &SharedEffect) -> Result<Effects<()>> {
        self.0.push(effect.0);
        Ok(Effects::none())
    }
}

#[composite]
#[scheduler(device = 0x00, instruction = SchedulerInst)]
struct SharedMachine {
    #[core]
    #[device(0x01)]
    core_a: Core,
    #[core]
    #[device(0x02)]
    core_b: Core,
    #[shared(core_a, core_b)]
    #[device(0x03, resolve_with = resolve_device)]
    device: SharedDevice,
    #[observe(SharedEffect)]
    observer: SharedObserver,
}

impl SharedMachine {
    fn resolve_device(&mut self, _inst: &DeviceInst) -> Result<DeviceMsg> {
        Ok(DeviceMsg("granted"))
    }
}

#[test]
fn machine_derive_generates_scheduler_backed_shared_devices() {
    let mut machine = SharedMachine {
        core_a: Core,
        core_b: Core,
        device: SharedDevice::default(),
        observer: SharedObserver::default(),
    };

    let metadata = machine.metadata();
    assert_eq!(metadata.device_by_name("device").unwrap().code, 0x03);
    assert_eq!(metadata.scheduler().unwrap().device_code, 0x00);
    assert_eq!(
        metadata.scheduler().unwrap().instruction_name,
        "SchedulerInst"
    );
    assert_eq!(metadata.shared_devices().len(), 1);
    assert_eq!(metadata.shared_devices()[0].device_code, 0x03);
    assert_eq!(
        metadata.shared_devices()[0].shared_with,
        &["core_a", "core_b"]
    );
    assert_eq!(metadata.source_symbol_device_code("device"), Some(0x03));
    assert_eq!(metadata.source_symbol_device_code("scheduler"), Some(0x00));

    machine
        .scheduler_dispatch(0x01, SchedulerInst::Acquire)
        .unwrap();
    assert!(machine.lock_holder(0x03).is_some());

    machine
        .scheduler_dispatch(0x02, SchedulerInst::Acquire)
        .unwrap();
    assert!(machine.core_is_parked(0x02));

    machine
        .dispatch_boxed_as_core(0x01, 0x03, Box::new(DeviceInst::Pulse))
        .unwrap();
    assert_eq!(machine.device.0, vec!["granted"]);
    assert_eq!(machine.observer.0, vec!["granted"]);

    let err = machine
        .dispatch_boxed_as_core(0x02, 0x03, Box::new(DeviceInst::Pulse))
        .unwrap_err();
    assert!(err.to_string().contains("lock"));

    machine
        .scheduler_dispatch(0x01, SchedulerInst::Release)
        .unwrap();
    assert!(!machine.core_is_parked(0x02));
}
