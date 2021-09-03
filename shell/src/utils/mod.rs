// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use thiserror::Error;

pub type OneshotResultCallback<T> = Arc<std::sync::mpsc::SyncSender<T>>;

#[derive(Error, Debug)]
pub enum DispatchOneshotResultCallbackError {
    #[error("Failed to dispatch result, reason: {reason}")]
    UnexpectedError { reason: String },
}

pub fn dispatch_oneshot_result<T, E, RC>(
    result_callback: Option<OneshotResultCallback<Result<T, E>>>,
    result: RC,
) -> Result<(), DispatchOneshotResultCallbackError>
where
    RC: FnOnce() -> Result<T, E>,
{
    if let Some(result_callback) = result_callback {
        result_callback.send(result()).map_err(|e| {
            DispatchOneshotResultCallbackError::UnexpectedError {
                reason: format!("{}", e),
            }
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    use crate::utils::dispatch_oneshot_result;

    #[test]
    fn test_wait_and_dispatch() -> Result<(), anyhow::Error> {
        let (result_callback_sender, result_callback_receiver) =
            std::sync::mpsc::sync_channel::<Result<(), ()>>(1);

        // run async
        {
            thread::spawn(move || {
                thread::sleep(Duration::from_secs(1));
                assert!(
                    dispatch_oneshot_result(Some(Arc::new(result_callback_sender)), || Ok(()))
                        .is_ok()
                );
            });
        }

        // wait
        let result: Result<(), ()> =
            result_callback_receiver.recv_timeout(Duration::from_secs(4))?;
        assert!(result.is_ok());

        Ok(())
    }
}
