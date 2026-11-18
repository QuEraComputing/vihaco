// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! The capture split, worker operations, and result comparison.

use super::{Machine, StartedCapture};
use crate::framework::{Compute, Wait, Worker};
use eyre::{Result, eyre};

pub(super) struct SplitCapture;
impl Compute for SplitCapture {
    type Machine = Machine;
    type Input = StartedCapture;
    type Output = (i64, Wait<Vec<i32>>);
    const NAME: &'static str = "split capture work";
    fn apply(_: &mut Machine, started: StartedCapture) -> Result<Self::Output> {
        Ok((started.expected, started.device))
    }
}

pub(super) struct PredictResponse;
impl Worker for PredictResponse {
    type Input = i64;
    type Output = i64;
    const NAME: &'static str = "spawn prediction worker";
    fn run(expected: i64) -> Result<i64> {
        Ok(expected)
    }
}
pub(super) struct AnalyzeCapture;
impl Worker for AnalyzeCapture {
    type Input = Vec<i32>;
    type Output = i64;
    const NAME: &'static str = "spawn analysis worker";
    fn run(samples: Vec<i32>) -> Result<i64> {
        samples
            .into_iter()
            .map(|sample| i64::from(sample).abs())
            .max()
            .ok_or_else(|| eyre!("empty capture"))
    }
}

pub(super) struct CompareResults;
impl Compute for CompareResults {
    type Machine = Machine;
    type Input = (i64, i64);
    type Output = i64;
    const NAME: &'static str = "compare results";
    fn apply(_: &mut Machine, (predicted, measured): (i64, i64)) -> Result<i64> {
        Ok(measured - predicted)
    }
}
