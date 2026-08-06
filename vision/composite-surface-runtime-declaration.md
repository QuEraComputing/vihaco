# Composite Surface and Runtime Declaration

An executable composite declares both the SST-facing instruction set and the
runtime products that execute on its components. The surface instruction is
parsed by generated syntax machinery, lowered into the composite's runtime
instruction sum, and then dispatched through a typed `Execute<I>` route.

```rust
vihaco::composite! {
    pub composite ControlMachine {
        error = ControlMachineFault;

        #[loadable]
        pub loader: ProgramImage<
            RuntimeInstruction,
            NoContext,
            Value,
            Type,
            DeviceInfo,
        >,

        #[device(0x01, alias = "processor")]
        pub processor: host_vm::Processor,

        #[device(0x02, alias = "waveform")]
        pub waveform: WaveformDevice,

        #[device(0x03, alias = "logic")]
        pub logic: LogicDevice,

        #[device(0x04, alias = "sensor")]
        pub sensor: SensorDevice,

        #[device(0x05, alias = "optical")]
        pub optical: OpticalDevice,

        pub clock: Clock,
        pub stdout: StdoutObserver,
        pub optical_devices: OpticalDevices,
        pub oscilloscope: Oscilloscope,
    }

    instructions {
        #[delegate(host_vm::Instruction)]
        Processor(host_vm::Instruction) => processor {
            message with resolve_processor;
            effects {
                observe stdout;
                handle with handle_processor;
            }
        }

        #[delegate(WaveformInstruction)]
        Waveform(WaveformInstruction) => waveform {
            message with resolve_waveform;
            effects {
                observe optical_devices, oscilloscope;
                handle with handle_waveform;
            }
        }

        #[delegate(LogicInstruction)]
        Logic(LogicInstruction) => logic {
            message with resolve_logic;
            effects {
                handle with handle_logic;
            }
        }

        #[pattern = "'get_measurement"]
        Sample(SensorDevice::instruction::Sample) => sensor {
            message with resolve_sample;
            effects {
                handle with handle_sample;
            }
        }

        #[pattern = "'pair_pulse"]
        PairPulse(OpticalDevice::instruction::PairPulse) => optical {
            message with resolve_pair_pulse;
            effects {
                observe stdout;
                handle with handle_optical;
            }
        }

        #[pattern = "'global_phase"]
        GlobalPhase(OpticalDevice::instruction::GlobalPhase) => optical {
            message with resolve_global_phase;
            effects {
                observe stdout;
                handle with handle_optical;
            }
        }

        #[pattern = "'global_rotation"]
        GlobalRotation(OpticalDevice::instruction::GlobalRotation) => optical {
            message with resolve_global_rotation;
            effects {
                observe stdout;
                handle with handle_optical;
            }
        }

        #[pattern = "'local_phase"]
        LocalPhase(OpticalDevice::instruction::LocalPhase) => optical {
            message with resolve_local_phase;
            effects {
                observe stdout;
                handle with handle_optical;
            }
        }

        #[pattern = "'local_rotation"]
        LocalRotation(OpticalDevice::instruction::LocalRotation) => optical {
            message with resolve_local_rotation;
            effects {
                observe stdout;
                handle with handle_optical;
            }
        }

        #[pattern = "'configure_sites"]
        ConfigureSites(OpticalDevice::instruction::ConfigureSites) => optical {
            message with resolve_configure_sites;
            effects {
                handle with handle_optical;
            }
        }

        #[pattern = "'read_sites"]
        ReadSites(OpticalDevice::instruction::ReadSites) => optical {
            message with resolve_read_sites;
            effects {
                observe stdout;
                handle with handle_optical;
            }
        }

        #[pattern = "'clear"]
        Clear(OpticalDevice::instruction::Clear) => optical {
            message with resolve_clear;
            effects {
                observe stdout;
                handle with handle_optical;
            }
        }
    }
}
```

The generated composite surface instruction enum is the parser product. The
generated runtime instruction enum is the execution product. For example:

```text
optical::pair_pulse
    -> ControlSurfaceInstruction::PairPulse
    -> RuntimeInstruction::PairPulse(OpticalDevice::instruction::PairPulse)
    -> resolve_pair_pulse
    -> Execute<OpticalDevice::instruction::PairPulse>
```

The route declaration owns the machine-specific association between surface
syntax, runtime product, component field, message resolution, observation, and
effect handling. Component declarations remain reusable: they provide typed
runtime products and `Execute<I>` implementations, while the composite chooses
which products become part of its SST vocabulary.
