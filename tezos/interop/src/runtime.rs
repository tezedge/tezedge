// Copyright (c) SimpleStaking and Tezedge Contributors
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

use crate::ffi;

lazy_static! {
    /// Because ocaml runtime should be accessed only by a single thread
    /// we will create the `OCAML_ENV` singleton.
    static ref OCAML_ENV: OcamlEnvironment = initialize_environment();
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

/// The future for the result received from ocaml side.
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

/// Allows the caller to use `await` on ocaml result.
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

/// Ocaml task is executed by `OCamlThreadExecutor`. Task holds future responsible
/// for executing ocaml function(s) and passing the result back to rust.
struct OCamlTask {
    /// this operation will be executed by `OCamlThreadExecutor` in the thread that is allowed to access the ocaml runtime
    op: Box<dyn FnOnce(&mut OCamlRuntime) + Send + 'static>,
    /// shared state between `OCamlTask` and `OCamlCallResult`
    state: Arc<Mutex<SharedState>>,
}

impl OCamlTask {
    /// Create new ocaml task
    ///
    /// # Arguments
    ///
    /// * `f` - the function will be executed in ocaml thread context
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
/// only a single instance of `OCamlThreadExecutor` running because ocaml runtime
/// is not designed to be accessed from multiple threads.
struct OCamlThreadExecutor {
    /// Receiver is used to receive tasks which will be then executed
    /// in the ocaml runtime.
    ready_tasks: Receiver<OCamlTask>,
    ocaml_runtime: OCamlRuntime,
}

impl OCamlThreadExecutor {
    /// Runs scheduled ocaml task to it's completion.
    fn run(mut self) -> OCamlRuntime {
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

/// Spawns ocaml task. Spawning is simply sending ocaml task into the sender queue `spawned_tasks`.
/// Ocaml tasks are then received and executed by the `OCamlThreadExecutor` singleton.
struct OCamlTaskSpawner {
    /// Sender is used to send tasks to the `OCamlThreadExecutor`.
    spawned_tasks: Arc<Mutex<Sender<OCamlTask>>>,
}

impl OCamlTaskSpawner {
    /// Spawns ocaml task. Spawning is simply sending ocaml task into the sender queue `spawned_tasks`.
    /// Ocaml tasks are then received and executed by the `OCamlThreadExecutor` singleton.
    pub fn spawn(&self, task: OCamlTask) -> Result<(), SendError<OCamlTask>> {
        self.spawned_tasks.lock().unwrap().send(task)
    }
}

/// Holds data related to ocaml environment.
struct OcamlEnvironment {
    spawner: OCamlTaskSpawner,
}

/// Create the environment and initialize ocaml runtime.
fn initialize_environment() -> OcamlEnvironment {
    let (task_tx, task_rx) = channel();
    let spawner = OCamlTaskSpawner {
        spawned_tasks: Arc::new(Mutex::new(task_tx)),
    };

    thread::spawn(move || {
        let ocaml_runtime = ffi::setup();
        let executor = OCamlThreadExecutor {
            ready_tasks: task_rx,
            ocaml_runtime,
        };
        let ocaml_runtime = executor.run();
        ocaml_runtime.shutdown();
    });

    OcamlEnvironment { spawner }
}

/// Run a function in ocaml runtime and return a result future.
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
