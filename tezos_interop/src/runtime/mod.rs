use std::future::Future;
use std::pin::Pin;
use std::ptr;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::task::{Context, Poll, Waker};

use futures::task::Spawn;
use ocaml::core::callback::caml_startup;
use ocaml::core::memory::caml_initialize;

const MAX_QUEUED_TASKS: usize = 10;

pub fn initialize_ocaml() {
    let mut ptr = ptr::null_mut();
    let argv: *mut *mut u8 = &mut ptr;
    unsafe {
        caml_startup(argv);
    }
}

pub struct OcamlResult<T> where T: Send {
    result: Arc<Mutex<Option<T>>>,
    state: Arc<Mutex<SharedState>>
}

pub struct OcamlTask {
    future: Box<dyn FnOnce() + Send + 'static>,
    state: Arc<Mutex<SharedState>>,
}

struct SharedState {
    waker: Option<Waker>
}

impl <T> Future for OcamlResult<T> where T: Send {
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


struct OcamlThreadExecutor {
    ready_tasks: Receiver<OcamlTask>
}

impl OcamlThreadExecutor {

    fn run(&self) {
        while let Ok(task) = self.ready_tasks.recv() {
            task.future.call_once(());
            if let Some(waker) = task.state.lock().unwrap().waker.take() {
                waker.wake()
            }
        }
    }

}



pub fn run<T, F>(f: F) -> OcamlResult<T>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static
{

    let (task_tx, task_rx) = channel();
    let executor = OcamlThreadExecutor { ready_tasks: task_rx };
    std::thread::spawn(move || {
        //initialize_ocaml();
        executor.run()
    });

    let result = Arc::new(Mutex::new(None));
    let state = Arc::new(Mutex::new(SharedState { waker: None }));
    let result_future = OcamlResult { result: result.clone(), state: state.clone() };
    let ocaml_future = Box::new(move || {
        let mut result = result.lock().unwrap();
        *result = Some(f());
    });
    let task = OcamlTask {
        future: ocaml_future,
        state: state.clone()
    };
    task_tx.send(task);

    result_future
}