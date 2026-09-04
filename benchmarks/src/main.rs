// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

fn main() -> eyre::Result<()> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("workloads");
    vihaco_benchmark::validate(&vihaco_benchmark::discover(&root)?)?;
    println!("Rust and SST workload validation passed");
    Ok(())
}
