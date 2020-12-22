// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use failure::Fail;

pub mod collections;

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

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Condvar, Mutex};
    use std::thread;
    use std::time::Duration;

    use crate::utils::{dispatch_condvar_result, try_wait_for_condvar_result, CondvarResult};

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
}
