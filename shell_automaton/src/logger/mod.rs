use std::fmt;
use std::ops::{Deref, DerefMut};

use derive_more::From;

mod logger_effects;
pub use logger_effects::*;

#[derive(From, Clone)]
pub struct Logger(slog::Logger);

impl Default for Logger {
    fn default() -> Self {
        Self(slog::Logger::root(slog::Discard, slog::o!()))
    }
}

impl fmt::Debug for Logger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Logger")
    }
}

impl Deref for Logger {
    type Target = slog::Logger;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Logger {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
