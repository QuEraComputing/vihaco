// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

// ===========================================================================================
// === AUTHOR: surface programs and channel-name resolution ==================================
// ===========================================================================================
//
// The surface form carries symbolic channel names. The machine's resolution step turns each name
// into the library-defined `ChannelId` used by the communication component (requirement 10).

/// A surface instruction as authored, before channel names are resolved.
#[derive(Debug, Clone, Copy)]
enum SurfaceInstruction {
    Add,
    Sub,
    Mul,
    Send(&'static str),
    Recv(&'static str),
}

/// The two directed channels wired into this machine.
const CHANNEL_A_TO_B: ChannelId = ChannelId(0);
const CHANNEL_B_TO_A: ChannelId = ChannelId(1);

/// Resolve a symbolic channel name to its runtime identifier. `to_b`/`from_a` name the A->B
/// channel; `to_a`/`from_b` name the B->A channel.
fn resolve_channel(name: &str) -> ChannelId {
    match name {
        "to_b" | "from_a" => CHANNEL_A_TO_B,
        "to_a" | "from_b" => CHANNEL_B_TO_A,
        other => panic!("unknown channel name: {other}"),
    }
}

/// Lower a whole surface program to runtime instructions, resolving channel names along the way.
fn resolve_program(surface: &[SurfaceInstruction]) -> Vec<RuntimeInstruction> {
    surface
        .iter()
        .map(|instruction| match *instruction {
            SurfaceInstruction::Add => RuntimeInstruction::IntegerAdd(Add),
            SurfaceInstruction::Sub => RuntimeInstruction::IntegerSub(Sub),
            SurfaceInstruction::Mul => RuntimeInstruction::IntegerMul(Mul),
            SurfaceInstruction::Send(name) => RuntimeInstruction::Send(Send {
                channel: resolve_channel(name),
            }),
            SurfaceInstruction::Recv(name) => RuntimeInstruction::Recv(Recv {
                channel: resolve_channel(name),
            }),
        })
        .collect()
}
