use std::future::Future;
use std::pin::Pin;
use std::ptr;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{channel, Receiver, Sender, SendError};
use std::task::{Context, Poll, Waker};

use ocaml::core::callback::caml_startup;

use lazy_static::lazy_static;

lazy_static! {
    /// Because ocaml runtime should be accessed only by a single thread
    /// we will create the `OCAML_ENV` singleton.
    static ref OCAML_ENV: OcamlEnvironment = initialize_environment();
}

/// This function will start ocaml runtime.
/// Ocaml runtime should always be called from a single thread.
pub fn start_ocaml_runtime() {
    let mut ptr = ptr::null_mut();
    let argv: *mut *mut u8 = &mut ptr;
    unsafe {
        caml_startup(argv);
    }
}

/// The result received from ocaml side.
/// Value is not available immediately but caller will have to await for it.
pub struct OcamlResult<T>
    where T: Send
{
    result: Arc<Mutex<Option<T>>>,
    state: Arc<Mutex<SharedState>>,
}

/// Allows the caller to use `await` on ocaml result.
impl<T> Future for OcamlResult<T>
    where T: Send
{
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut result = self.result.lock().unwrap();
        match *result {
            Some(_) => Poll::Ready(result.take().unwrap()),
            None => {
                let mut state = self.state.lock().unwrap();
                state.waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

/// Ocaml task is executed by `OcamlThreadExecutor`. Task holds future responsible
/// for executing ocaml function(s) and passing the result back to rust.
struct OcamlTask {
    future: Box<dyn FnOnce() + Send + 'static>,
    state: Arc<Mutex<SharedState>>,
}

impl OcamlTask {
    /// Create new ocaml task
    ///
    /// # Arguments
    ///
    /// * `f` - the function will be executed in ocaml thread context
    /// * `f_result_holder` - will hold result of the `f` after `f`'s completion
    /// * `shared_state` - shared state between `OcamlTask` and `OcamlResult`
    fn new<F, T>(f: F, f_result_holder: Arc<Mutex<Option<T>>>, shared_state: Arc<Mutex<SharedState>>) -> OcamlTask
        where
            F: FnOnce() -> T + Send + 'static,
            T: Send + 'static
    {
        let ocaml_future = Box::new(move || {
            let mut result = f_result_holder.lock().unwrap();
            *result = Some(f());
        });
        OcamlTask {
            future: ocaml_future,
            state: shared_state,
        }
    }
}

/// This struct represents a shared state between `OcamlTask` and `OcamlResult`.
struct SharedState {
    waker: Option<Waker>
}

/// Runs `OcamlTask` to it's completion. By design of this library there will be
/// only a single instance of `OcamlThreadExecutor` running because ocaml runtime
/// is not designed to be accessed from multiple threads.
struct OcamlThreadExecutor {

    /// Receiver is used to receive tasks which will be then executed
    /// in the ocaml runtime.
    ready_tasks: Receiver<OcamlTask>
}

impl OcamlThreadExecutor {
    /// Runs scheduled ocaml task to it's completion.
    fn run(&self) {
        while let Ok(task) = self.ready_tasks.recv() {
            task.future.call_once(());
            if let Some(waker) = task.state.lock().unwrap().waker.take() {
                waker.wake()
            }
        }
    }
}

/// Spawns ocaml task. Spawning is simply sending ocaml task into the sender queue `spawned_tasks`.
/// Ocaml tasks are then received and executed by the `OcamlThreadExecutor` singleton.
struct OcamlTaskSpawner {

    /// Sender is used to send tasks to the `OcamlThreadExecutor`.
    spawned_tasks: Arc<Mutex<Sender<OcamlTask>>>
}

impl OcamlTaskSpawner {

    /// Spawns ocaml task. Spawning is simply sending ocaml task into the sender queue `spawned_tasks`.
    /// Ocaml tasks are then received and executed by the `OcamlThreadExecutor` singleton.
    pub fn spawn(&self, task: OcamlTask) -> Result<(), SendError<OcamlTask>> {
        self.spawned_tasks.lock().unwrap().send(task)
    }
}

/// Holds data related to ocaml environment.
struct OcamlEnvironment {
    spawner: OcamlTaskSpawner
}

/// Create the environment and initialize ocaml runtime.
fn initialize_environment() -> OcamlEnvironment {
    let (task_tx, task_rx) = channel();
    let spawner = OcamlTaskSpawner { spawned_tasks: Arc::new(Mutex::new(task_tx)) };
    let executor = OcamlThreadExecutor { ready_tasks: task_rx };
    std::thread::spawn(move || {
        start_ocaml_runtime();
        executor.run()
    });

    OcamlEnvironment { spawner }
}

/// Run a function in ocaml runtime and return a result.
pub fn spawn<F, T>(f: F) -> OcamlResult<T>
    where
        F: FnOnce() -> T + 'static + Send,
        T: 'static + Send
{
    let result = Arc::new(Mutex::new(None));
    let state = Arc::new(Mutex::new(SharedState { waker: None }));
    let result_future = OcamlResult { result: result.clone(), state: state.clone() };
    let task = OcamlTask::new(f, result, state);
    OCAML_ENV.spawner.spawn(task).expect("Failed to spawn task");

    result_future
}