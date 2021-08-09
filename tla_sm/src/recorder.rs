pub trait Recorder<'a> {
    type Value: 'a;
    type Recorded;

    /// Get `Value` and record it.
    ///
    /// Value's usage will be recorded, so that if recorded value is
    /// passed to same determenistic function, result will be same.
    fn record(&'a mut self) -> Self::Value;

    /// Finish recording and return recorded value.
    fn finish(self) -> Self::Recorded;
}

pub trait DefaultRecorder {
    type Recorder: for<'a> Recorder<'a>;

    /// Get the default [Recorder] for a given type.
    fn default_recorder(self) -> Self::Recorder;
}
