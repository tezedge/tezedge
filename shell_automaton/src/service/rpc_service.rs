use hyper::{
    service::{make_service_fn, service_fn},
    Body, Request, Response, StatusCode,
};
use redux_rs::{ActionId, ActionWithId};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;
use std::thread;
use storage::{PersistentStorage, ShellAutomatonActionStorage, ShellAutomatonStateStorage, StorageError};

use crate::{action::Action, State};

use super::service_channel::{
    worker_channel, ResponseTryRecvError, ServiceWorkerRequester, ServiceWorkerResponder,
    ServiceWorkerResponderSender,
};

pub trait RpcService {
    /// Try to receive/read queued message, if there is any.
    fn try_recv(&mut self) -> Result<RpcResponse, ResponseTryRecvError>;
}

#[derive(Debug)]
pub enum RpcResponse {
    GetCurrentGlobalState {
        channel: tokio::sync::oneshot::Sender<State>,
    },
}

type ServiceResult = Result<Response<Body>, Box<dyn std::error::Error + Sync + Send>>;

#[derive(Debug)]
pub struct RpcServiceDefault {
    worker_channel: ServiceWorkerRequester<(), RpcResponse>,
}

#[derive(Serialize, Deserialize)]
struct ActionWithState {
    #[serde(flatten)]
    action: ActionWithId<Action>,
    state: State,
}

/// Generate 404 response
fn not_found() -> ServiceResult {
    Ok(Response::builder()
        .status(StatusCode::from_u16(404)?)
        .header(hyper::header::CONTENT_TYPE, "text/plain")
        .header(hyper::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(hyper::header::ACCESS_CONTROL_ALLOW_HEADERS, "Content-Type")
        .header(hyper::header::ACCESS_CONTROL_ALLOW_HEADERS, "content-type")
        .body(Body::empty())?)
}

/// Function to generate JSON response from serializable object
fn make_json_response<T: serde::Serialize>(content: &T) -> ServiceResult {
    Ok(Response::builder()
        .header(hyper::header::CONTENT_TYPE, "application/json")
        // TODO: add to config
        .header(hyper::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(hyper::header::ACCESS_CONTROL_ALLOW_HEADERS, "Content-Type")
        .header(hyper::header::ACCESS_CONTROL_ALLOW_HEADERS, "content-type")
        .header(
            hyper::header::ACCESS_CONTROL_ALLOW_METHODS,
            "GET, POST, OPTIONS, PUT",
        )
        .body(Body::from(serde_json::to_string(content)?))?)
}

/// Helper for parsing URI queries.
/// Functions takes URI query in format `key1=val1&key1=val2&key2=val3`
/// and produces map `{ key1: [val1, val2], key2: [val3] }`
fn parse_query_string(query: &str) -> HashMap<String, Vec<String>> {
    let mut ret: HashMap<String, Vec<String>> = HashMap::new();
    for (key, value) in query.split('&').map(|x| {
        let mut parts = x.split('=');
        (parts.next().unwrap(), parts.next().unwrap_or(""))
    }) {
        if let Some(vals) = ret.get_mut(key) {
            // append value to existing vector
            vals.push(value.to_string());
        } else {
            // create new vector with a single value
            ret.insert(key.to_string(), vec![value.to_string()]);
        }
    }
    ret
}

impl RpcServiceDefault {
    async fn get_current_global_state(
        mut sender: ServiceWorkerResponderSender<RpcResponse>,
    ) -> Result<State, tokio::sync::oneshot::error::RecvError> {
        let (tx, rx) = tokio::sync::oneshot::channel();

        sender.send(RpcResponse::GetCurrentGlobalState { channel: tx });
        rx.await
    }

    async fn get_action(
        action_storage: &ShellAutomatonActionStorage,
        action_id: u64,
    ) -> Result<Option<ActionWithId<Action>>, StorageError> {
        let action_storage = action_storage.clone();
        tokio::task::spawn_blocking(move || {
            Ok(action_storage
                .get::<Action>(&action_id)?
                .map(|action| ActionWithId {
                    id: ActionId::new_unchecked(action_id),
                    action,
                }))
        })
        .await
        .unwrap()
    }

    async fn get_state_before_action_id(
        snapshot_storage: &ShellAutomatonStateStorage,
        action_storage: &ShellAutomatonActionStorage,
        target_action_id: u64,
    ) -> Result<State, Box<dyn std::error::Error>> {
        let closest_snapshot_action_id = target_action_id / 10000;
        let mut state = match snapshot_storage.get(&closest_snapshot_action_id).unwrap() {
            Some(v) => v,
            None => return Err("snapshot not available".into()),
        };

        if target_action_id > closest_snapshot_action_id {
            for action_id in (closest_snapshot_action_id + 1)..target_action_id {
                let action = match Self::get_action(action_storage, action_id).await.unwrap() {
                    Some(v) => v,
                    None => break,
                };
                crate::reducer(&mut state, &action);
            }
        }

        Ok(state)
    }

    async fn get_state_after_action_id(
        snapshot_storage: &ShellAutomatonStateStorage,
        action_storage: &ShellAutomatonActionStorage,
        target_action_id: u64,
    ) -> Result<State, Box<dyn std::error::Error>> {
        let mut state =
            Self::get_state_before_action_id(snapshot_storage, action_storage, target_action_id)
                .await?;

        if let Some(action) = Self::get_action(action_storage, target_action_id)
            .await
            .unwrap()
        {
            crate::reducer(&mut state, &action);
        };

        Ok(state)
    }

    async fn handle_global_state_get(
        sender: ServiceWorkerResponderSender<RpcResponse>,
        snapshot_storage: &ShellAutomatonStateStorage,
        action_storage: &ShellAutomatonActionStorage,
        target_action_id: Option<u64>,
    ) -> ServiceResult {
        make_json_response(&match target_action_id {
            Some(target_action_id) => {
                Self::get_state_after_action_id(snapshot_storage, action_storage, target_action_id)
                    .await
                    .ok()
            }
            None => Some(Self::get_current_global_state(sender).await.unwrap()),
        })
    }

    async fn handle_actions_get(
        sender: ServiceWorkerResponderSender<RpcResponse>,
        snapshot_storage: &ShellAutomatonStateStorage,
        action_storage: &ShellAutomatonActionStorage,
        cursor: Option<u64>,
        limit: Option<u64>,
    ) -> ServiceResult {
        // TODO: optimize by getting just part of state, instead of whole state.
        let limit = limit.unwrap_or(20).max(1).min(1000);

        let end = match cursor {
            Some(v) => v,
            None => {
                let state = Self::get_current_global_state(sender).await.unwrap();
                state.last_action_id.into()
            }
        };
        let start = end.checked_sub(limit - 1).unwrap_or(0);

        let mut state =
            match Self::get_state_before_action_id(snapshot_storage, action_storage, start).await {
                Ok(v) => v,
                Err(err) => {
                    dbg!(err);
                    return make_json_response::<Vec<()>>(&vec![]);
                }
            };

        let mut actions_with_state = VecDeque::new();

        let start = match start {
            // Actions start from 1.
            0 => 1,
            v => v,
        };

        for action_id in start..=end {
            let action = match Self::get_action(action_storage, action_id).await.unwrap() {
                Some(v) => v,
                None => break,
            };
            crate::reducer(&mut state, &action);
            actions_with_state.push_front(ActionWithState {
                action,
                state: state.clone(),
            });
        }

        make_json_response(&actions_with_state)
    }

    fn run_worker(
        bind_address: SocketAddr,
        channel: ServiceWorkerResponder<(), RpcResponse>,
        storage: PersistentStorage,
    ) -> impl Future<Output = Result<(), hyper::Error>> {
        let sender = channel.sender();

        let snapshot_storage = ShellAutomatonStateStorage::new(&storage);
        let action_storage = ShellAutomatonActionStorage::new(&storage);

        hyper::Server::bind(&bind_address).serve(make_service_fn(move |_| {
            let sender = sender.clone();
            let snapshot_storage = snapshot_storage.clone();
            let action_storage = action_storage.clone();

            async move {
                Ok::<_, hyper::Error>(service_fn(move |req: Request<Body>| {
                    let sender = sender.clone();
                    let snapshot_storage = snapshot_storage.clone();
                    let action_storage = action_storage.clone();
                    async move {
                        let path = req.uri().path();
                        if path == "/state" {
                            let query = req
                                .uri()
                                .query()
                                .map(|query_str| parse_query_string(query_str))
                                .unwrap_or(HashMap::new());

                            Self::handle_global_state_get(
                                sender,
                                &snapshot_storage,
                                &action_storage,
                                query.get("action_id").map(|x| x[0].parse().ok()).flatten(),
                            )
                            .await
                        } else if path.starts_with("/actions") {
                            let query = req
                                .uri()
                                .query()
                                .map(|query_str| parse_query_string(query_str))
                                .unwrap_or(HashMap::new());

                            Self::handle_actions_get(
                                sender,
                                &snapshot_storage,
                                &action_storage,
                                query.get("cursor").map(|x| x[0].parse().ok()).flatten(),
                                query.get("limit").map(|x| x[0].parse().ok()).flatten(),
                            )
                            .await
                        } else {
                            not_found()
                        }
                    }
                }))
            }
        }))
    }

    // TODO: remove unwraps
    pub fn init(waker: Arc<mio::Waker>, storage: PersistentStorage) -> Self {
        let (requester, responder) = worker_channel(waker);

        thread::spawn(move || {
            let rpc_listen_address = ([0, 0, 0, 0], 18732).into();
            let threaded_rt = tokio::runtime::Runtime::new().unwrap();
            threaded_rt.block_on(async move {
                Self::run_worker(rpc_listen_address, responder, storage)
                    .await
                    .unwrap();
            });
        });

        Self {
            worker_channel: requester,
        }
    }
}

impl RpcService for RpcServiceDefault {
    #[inline(always)]
    fn try_recv(&mut self) -> Result<RpcResponse, ResponseTryRecvError> {
        self.worker_channel.try_recv()
    }
}
