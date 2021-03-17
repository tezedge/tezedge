// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use failure::Fail;
use failure::{Backtrace, Context};
use std::fmt::{self, Display};

/// Error produced by a [BinaryReader].
#[derive(Debug)]
pub struct EncodingError<T: Display + Send + Sync + 'static> {
    inner: Context<ErrorInfo<T>>,
}

impl<T: Display + Send + Sync + std::fmt::Debug + 'static> Fail for EncodingError<T> {
    fn cause(&self) -> Option<&dyn Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl<T: Display + Send + Sync + 'static> Display for EncodingError<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

impl<T: Display + Send + Sync + Clone + 'static> EncodingError<T> {
    pub fn kind(&self) -> T {
        self.inner.get_context().0.clone()
    }

    pub fn location(&self) -> &String {
        &self.inner.get_context().1
    }

    pub(crate) fn field(&self, name: &str) -> ErrorInfo<T> {
        self.inner.get_context().field(name)
    }

    pub(crate) fn element_of(&self) -> ErrorInfo<T> {
        self.inner.get_context().element_of()
    }
}

impl<T: Display + Send + Sync + 'static> From<T> for EncodingError<T> {
    fn from(kind: T) -> Self {
        EncodingError {
            inner: Context::new(kind.into()),
        }
    }
}

impl<T: Display + Send + Sync + 'static> From<ErrorInfo<T>> for EncodingError<T> {
    fn from(info: ErrorInfo<T>) -> Self {
        Self {
            inner: Context::new(info),
        }
    }
}

impl<T: Display + Send + Sync + 'static> From<Context<ErrorInfo<T>>> for EncodingError<T> {
    fn from(context: Context<ErrorInfo<T>>) -> Self {
        Self { inner: context }
    }
}

/// Error kind with a string describing the error context
pub(crate) struct ErrorInfo<T: Display + Send + Sync + 'static>(T, String);

impl<T: Display + Send + Sync + Clone + 'static> ErrorInfo<T> {
    pub fn field(&self, name: &str) -> Self {
        let msg = if self.1.is_empty() {
            format!("field `{}`", name)
        } else {
            format!("{} @ field `{}`", self.1, name)
        };
        ErrorInfo(self.0.clone(), msg)
    }

    pub fn element_of(&self) -> Self {
        let msg = if self.1.is_empty() {
            "list element".to_string()
        } else {
            format!("{} @ list element", self.1)
        };
        ErrorInfo(self.0.clone(), msg)
    }
}

impl<T: Display + Send + Sync + 'static> Display for ErrorInfo<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.1.is_empty() {
            write!(f, "{} @ unknown location", self.0)
        } else {
            write!(f, "{} @ {}", self.0, self.1)
        }
    }
}

impl<T: Display + Send + Sync + 'static> From<T> for ErrorInfo<T> {
    fn from(kind: T) -> Self {
        Self(kind, "".to_string())
    }
}
