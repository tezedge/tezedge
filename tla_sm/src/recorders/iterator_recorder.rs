// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub type RecordedIterator<T> = Vec<T>;

pub struct IteratorRecorder<I, T> {
    iterator: I,
    recorded: RecordedIterator<T>,
}

impl<I, T> IteratorRecorder<I, T> {
    pub fn new(iterator: I) -> Self {
        Self {
            iterator,
            recorded: Default::default(),
        }
    }

    pub fn record(&mut self) -> &mut Self {
        self
    }

    pub fn finish_recording(self) -> RecordedIterator<T> {
        self.recorded
    }
}

impl<I, T> Iterator for IteratorRecorder<I, T>
where
    I: Iterator<Item = T>,
    T: Clone,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        match self.iterator.next() {
            Some(item) => {
                self.recorded.push(item.clone());
                Some(item)
            }
            None => None,
        }
    }
}
