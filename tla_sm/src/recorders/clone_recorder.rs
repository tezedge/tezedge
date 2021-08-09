use crate::Recorder;

/// Recorder for values that can be simply cloned.
pub struct CloneRecorder<T> {
    value: T,
}

impl<T> CloneRecorder<T> {
    pub fn new(value: T) -> Self {
        Self { value }
    }
}

impl<'a, T> Recorder<'a> for CloneRecorder<T>
where
    T: 'a + Clone,
{
    type Value = T;
    type Recorded = T;

    fn record(&'a mut self) -> Self::Value {
        self.value.clone()
    }

    fn finish(self) -> Self::Recorded {
        self.value
    }
}
