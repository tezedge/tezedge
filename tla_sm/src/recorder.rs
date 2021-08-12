pub trait DefaultRecorder {
    type Recorder;

    /// Get the default [Recorder] for a given type.
    fn default_recorder(self) -> Self::Recorder;
}
