// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use failure::Fail;

// TODO: TE-386 - remove not needed
// pub mod collections;

/// Simple condvar synchronized result callback
pub type CondvarResult<T, E> = Arc<(Mutex<Option<Result<T, E>>>, Condvar)>;

#[derive(Fail, Debug)]
pub enum WaitCondvarResultError {
    #[fail(display = "Timeout exceeded: {:?}", duration)]
    TimeoutExceeded { duration: Duration },

    #[fail(display = "No result received")]
    NoResultReceived,

    #[fail(display = "Mutex/lock poison error, reason: {}", reason)]
    PoisonedLock { reason: String },
}

#[derive(Fail, Debug)]
pub enum DispatchCondvarResultError {
    #[fail(display = "Failed to set result, reason: {}", reason)]
    DispatchResultError { reason: String },
}

pub fn dispatch_condvar_result<T, E, RC>(
    result_callback: Option<CondvarResult<T, E>>,
    result: RC,
    notify_condvar_on_lock_error: bool,
) -> Result<(), DispatchCondvarResultError>
where
    RC: FnOnce() -> Result<T, E>,
{
    if let Some(result_callback) = result_callback {
        let &(ref lock, ref cvar) = &*result_callback;
        match lock.lock() {
            Ok(mut result_guard) => {
                *result_guard = Some(result());
                cvar.notify_all();
                Ok(())
            }
            Err(e) => {
                if notify_condvar_on_lock_error {
                    cvar.notify_all();
                }
                Err(DispatchCondvarResultError::DispatchResultError {
                    reason: format!("{}", e),
                })
            }
        }
    } else {
        Ok(())
    }
}

pub fn try_wait_for_condvar_result<T, E>(
    result_callback: CondvarResult<T, E>,
    duration: Duration,
) -> Result<Result<T, E>, WaitCondvarResultError> {
    // get lock
    let &(ref lock, ref cvar) = &*result_callback;
    let lock = lock
        .lock()
        .map_err(|e| WaitCondvarResultError::PoisonedLock {
            reason: format!("{}", e),
        })?;

    // wait for condvar and handle
    match cvar.wait_timeout(lock, duration) {
        Ok((mut result, timeout)) => {
            // process timeout
            if timeout.timed_out() {
                return Err(WaitCondvarResultError::TimeoutExceeded { duration });
            }

            // process result
            match result.take() {
                Some(result) => Ok(result),
                None => Err(WaitCondvarResultError::NoResultReceived),
            }
        }
        Err(e) => Err(WaitCondvarResultError::PoisonedLock {
            reason: format!("{}", e),
        }),
    }
}

/// Simple try_lock without Mutex/RwLock
///
/// Inspired by: https://morestina.net/blog/749/exploring-lock-free-rust-2-atomics
pub struct DeadlineTryLock(Arc<AtomicBool>, Duration);

impl DeadlineTryLock {
    pub fn new(timeout: Duration) -> DeadlineTryLock {
        DeadlineTryLock(Arc::new(AtomicBool::new(false)), timeout)
    }

    pub fn try_lock(&self) -> Option<DeadlineTryLockGuard> {
        let was_locked = self.0.swap(true, Ordering::Acquire);
        if was_locked {
            None
        } else {
            Some(DeadlineTryLockGuard {
                lock: self.0.clone(),
                deadline: Instant::now() + self.1,
            })
        }
    }
}

pub struct DeadlineTryLockGuard {
    lock: Arc<AtomicBool>,
    deadline: Instant,
}

impl DeadlineTryLockGuard {
    pub fn is_deadline_reached(&self) -> bool {
        self.deadline.le(&Instant::now())
    }
}

impl Drop for DeadlineTryLockGuard {
    fn drop(&mut self) {
        self.lock.store(false, Ordering::Release);
    }
}

// TODO: TE-386 - remove not needed
// /// Unility to help manage [`UniqueBlockData`] structure
// pub(crate) struct MissingBlockData<D> {
//     pub missing_data: UniqueBlockData<D>,
// }

// impl<D: BlockData + Ord> MissingBlockData<D> {
// TODO: TE-386 - remove not needed
// pub fn push_data(&mut self, missing_data: D) {
//     self.missing_data.push(missing_data);
// }

// TODO: TE-386 - remove not needed
// #[inline]
// pub fn has_missing_data(&self) -> bool {
//     !self.missing_data.is_empty()
// }
//
// #[inline]
// pub fn missing_data_count(&self) -> usize {
//     self.missing_data.len()
// }

// TODO: TE-386 - remove not needed
// #[inline]
// pub fn drain_missing_data<F>(&mut self, n: usize, filter: F) -> Vec<D>
// where
//     F: Fn(&D) -> bool,
// {
//     (0..std::cmp::min(self.missing_data.len(), n))
//         .filter_map(|_| {
//             if self
//                 .missing_data
//                 .peek()
//                 .filter(|block| filter(block))
//                 .is_some()
//             {
//                 self.missing_data.pop()
//             } else {
//                 None
//             }
//         })
//         .collect()
// }
// }
//
// impl<D: BlockData + Ord> Default for MissingBlockData<D> {
//     fn default() -> Self {
//         Self {
//             missing_data: UniqueBlockData::default(),
//         }
//     }
// }

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Condvar, Mutex};
    use std::thread;
    use std::time::Duration;

    use crate::utils::{
        dispatch_condvar_result, try_wait_for_condvar_result, CondvarResult, DeadlineTryLock,
    };

    #[test]
    fn test_wait_and_dispatch() -> Result<(), failure::Error> {
        let condvar_result: CondvarResult<(), failure::Error> =
            Arc::new((Mutex::new(None), Condvar::new()));

        // run async
        {
            let result = condvar_result.clone();
            thread::spawn(move || {
                thread::sleep(Duration::from_secs(2));
                assert!(dispatch_condvar_result(Some(result), || Ok(()), true).is_ok());
            });
        }

        // wait
        assert!(try_wait_for_condvar_result(condvar_result, Duration::from_secs(4))?.is_ok());

        Ok(())
    }

    #[test]
    fn test_atomic_try_lock() {
        let lock = DeadlineTryLock::new(Duration::from_secs(2));

        // get lock
        let lock_guard = lock.try_lock();
        assert!(lock_guard.is_some());

        // get next lock
        assert!(lock.try_lock().is_none());
        assert!(lock.try_lock().is_none());
        assert!(lock.try_lock().is_none());

        // release lock
        drop(lock_guard);

        // try next lock
        assert!(lock.try_lock().is_some());
        assert!(lock.try_lock().is_some());
    }
}
