/// Recorder for values that can be simply cloned.
pub struct CloneRecorder<T> {
    value: T,
}

impl<T: Clone> CloneRecorder<T> {
    pub fn new(value: T) -> Self {
        Self { value }
    }

    pub fn record(&mut self) -> T {
        self.value.clone()
    }

    pub fn finish_recording(self) -> T {
        self.value
    }
}
