// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::error;
use std::fmt;
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::pin::Pin;
use std::sync::mpsc::{channel, Receiver, SendError, Sender};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::thread;

use futures::executor::LocalPool;

use lazy_static::lazy_static;
use ocaml_interop::OCamlRuntime;

lazy_static! {
    /// Because OCaml runtime should be accessed only by a single thread
    /// we will create the `OCAML_ENV` singleton.
    static ref OCAML_ENV: OCamlEnvironment = initialize_environment();
}

/// OCaml execution error
pub struct OCamlBlockPanic;

impl error::Error for OCamlBlockPanic {}

impl fmt::Display for OCamlBlockPanic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "Panic during the execution of an OCaml block")
    }
}

impl fmt::Debug for OCamlBlockPanic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "Panic during the execution of an OCaml block")
    }
}

type TaskResultHolder<T> = Arc<Mutex<Option<Result<T, OCamlBlockPanic>>>>;

/// The future for the result received from OCaml side.
/// Value is not available immediately but caller will have to await for it.
pub struct OCamlCallResult<T>
where
    T: Send,
{
    /// will contain result of `OCamlTask`
    result: TaskResultHolder<T>,
    /// shared state between `OCamlTask` and `OCamlCallResult`
    state: Arc<Mutex<SharedState>>,
}

/// Allows the caller to use `await` on OCaml result.
impl<T> Future for OCamlCallResult<T>
where
    T: Send,
{
    type Output = Result<T, OCamlBlockPanic>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut result = self.result.lock().unwrap();
        match *result {
            Some(_) => Poll::Ready(result.take().unwrap()),
            None => {
                let mut state = self.state.lock().unwrap();
                // always take the current waker so `OCamlThreadExecutor` will be able to notify it
                state.waker = Some(cx.waker().clone());
                // return pending, because we had not result available
                Poll::Pending
            }
        }
    }
}

/// OCaml task is executed by `OCamlThreadExecutor`. Task holds future responsible
/// for executing OCaml function(s) and passing the result back to rust.
struct OCamlTask {
    /// this operation will be executed by `OCamlThreadExecutor` in the thread that is allowed to access the OCaml runtime
    op: Box<dyn FnOnce(&mut OCamlRuntime) + Send + 'static>,
    /// shared state between `OCamlTask` and `OCamlCallResult`
    state: Arc<Mutex<SharedState>>,
}

impl OCamlTask {
    /// Create new OCaml task
    ///
    /// # Arguments
    ///
    /// * `f` - the function will be executed in OCaml thread context
    /// * `f_result_holder` - will hold result of the `f` after `f`'s completion
    /// * `shared_state` - shared state between `OCamlTask` and `OCamlCallResult`
    fn new<F, T>(
        f: F,
        f_result_holder: TaskResultHolder<T>,
        shared_state: Arc<Mutex<SharedState>>,
    ) -> OCamlTask
    where
        F: FnOnce(&mut OCamlRuntime) -> T + Send + 'static,
        T: Send + 'static,
    {
        OCamlTask {
            op: Box::new(move |rt: &mut OCamlRuntime| {
                let mut result = f_result_holder.lock().unwrap();
                match std::panic::catch_unwind(AssertUnwindSafe(|| f(rt))) {
                    Ok(f_result) => *result = Some(Ok(f_result)),
                    Err(_) => *result = Some(Err(OCamlBlockPanic)),
                }
            }),
            state: shared_state,
        }
    }
}

/// This struct represents a shared state between `OCamlTask` and `OCamlCallResult`.
struct SharedState {
    /// this waker is used to notify that `OCamlCallResult` is now ready to be polled
    waker: Option<Waker>,
}

/// Runs `OCamlTask` to it's completion. By design of this library there will be
/// only a single instance of `OCamlThreadExecutor` running because OCaml runtime
/// is not designed to be accessed from multiple threads.
struct OCamlThreadExecutor {
    /// Receiver is used to receive tasks which will be then executed
    /// in the OCaml runtime.
    ready_tasks: Receiver<OCamlTask>,
    ocaml_runtime: OCamlRuntime,
}

impl OCamlThreadExecutor {
    /// Runs scheduled OCaml task to it's completion.
    fn run(mut self) -> OCamlRuntime {
        // This loop will run until the other side is closed.
        // When that happens the OCaml runtime handle will be dropped
        // causing the OCaml runtime to be shutdown.
        while let Ok(task) = self.ready_tasks.recv() {
            // execute future from task
            (task.op)(&mut self.ocaml_runtime);
            // notify waker that OCamlCallResult (it implements Future) is ready to be polled
            if let Some(waker) = task.state.lock().unwrap().waker.take() {
                waker.wake()
            }
        }
        self.ocaml_runtime
    }
}

/// Spawns OCaml task. Spawning is simply sending OCaml task into the sender queue `spawned_tasks`.
/// OCaml tasks are then received and executed by the `OCamlThreadExecutor` singleton.
struct OCamlTaskSpawner {
    /// Sender is used to send tasks to the `OCamlThreadExecutor`.
    spawned_tasks: Arc<Mutex<Sender<OCamlTask>>>,
}

impl OCamlTaskSpawner {
    /// Spawns OCaml task. Spawning is simply sending OCaml task into the sender queue `spawned_tasks`.
    /// OCaml tasks are then received and executed by the `OCamlThreadExecutor` singleton.
    pub fn spawn(&self, task: OCamlTask) -> Result<(), SendError<OCamlTask>> {
        self.spawned_tasks.lock().unwrap().send(task)
    }

    /// Shuts down the spawner loop causing the OCaml runtime to shutdown too.
    pub fn shutdown(&self) {
        // We drop the channel so that the receiving end loop stops.
        drop(self.spawned_tasks.lock().unwrap())
    }
}

/// Holds data related to OCaml environment.
struct OCamlEnvironment {
    spawner: OCamlTaskSpawner,
}

/// Create the environment and initialize OCaml runtime.
fn initialize_environment() -> OCamlEnvironment {
    let (task_tx, task_rx) = channel();
    let spawner = OCamlTaskSpawner {
        spawned_tasks: Arc::new(Mutex::new(task_tx)),
    };

    thread::Builder::new()
        .name("ffi-ocaml-executor-thread".to_string())
        .spawn(move || {
            let ocaml_runtime = super::setup();
            let executor = OCamlThreadExecutor {
                ready_tasks: task_rx,
                ocaml_runtime,
            };
            executor.run();
        })
        .expect("Failed to spawn thread to initialize OCamlEnvironment");

    OCamlEnvironment { spawner }
}

/// Run a function in OCaml runtime and return a result future.
///
/// # Arguments
///
/// * `f` - the function will be executed in OCaml thread context
pub fn spawn<F, T>(f: F) -> OCamlCallResult<T>
where
    F: FnOnce(&mut OCamlRuntime) -> T + 'static + Send,
    T: 'static + Send,
{
    let result = Arc::new(Mutex::new(None));
    let state = Arc::new(Mutex::new(SharedState { waker: None }));
    let result_future = OCamlCallResult {
        result: result.clone(),
        state: state.clone(),
    };
    let task = OCamlTask::new(f, result, state);
    OCAML_ENV
        .spawner
        .spawn(task)
        .expect("Failed to spawn OCaml task");

    result_future
}

/// Synchronously execute provided function
///
/// # Arguments
///
/// * `f` - the function will be executed in OCaml thread context
pub fn execute<F, T>(f: F) -> Result<T, OCamlBlockPanic>
where
    F: FnOnce(&mut OCamlRuntime) -> T + 'static + Send,
    T: 'static + Send,
{
    LocalPool::new().run_until(spawn(f))
}

/// Shutdown the OCaml runtime.
pub fn shutdown() {
    OCAML_ENV.spawner.shutdown()
}
