// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub trait DefaultRecorder {
    type Recorder;

    /// Get the default [Recorder] for a given type.
    fn default_recorder(self) -> Self::Recorder;
}
