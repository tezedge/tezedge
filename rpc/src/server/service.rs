use hyper::{Body, Response, Error, Server, Request, StatusCode, Method};
use hyper::service::{service_fn, make_service_fn};
use std::net::SocketAddr;
use lazy_static::lazy_static;
use std::sync::Mutex;
use futures::Future;
use crate::rpc_actor::RpcServerRef;
use riker::actors::ActorSystem;
use crate::server::ask::ask;
use serde_json;
use chrono::prelude::*;
use crate::encoding::base_types::*;
use tezos_encoding::hash::{HashEncoding, HashType};

lazy_static! {
    static ref ACTOR_SYSTEM: Mutex<Option<ActorSystem>> = Mutex::new(None);
    static ref RPC_ACTOR: Mutex<Option<RpcServerRef>> = Mutex::new(None);
}

type ServiceResult = Result<Response<Body>, Box<dyn std::error::Error + Sync + Send>>;

pub fn spawn_server(addr: &SocketAddr, sys: ActorSystem, actor: RpcServerRef) -> impl Future<Output=Result<(), Error>> {
    let mut state = ACTOR_SYSTEM.lock().unwrap();
    *state = Some(sys);

    let mut state = RPC_ACTOR.lock().unwrap();
    *state = Some(actor);

    Server::bind(addr)
        .serve(make_service_fn(|_| async move {
            Ok::<_, Error>(service_fn(move |req| {
                async move {
                    router(req).await
                }
            }))
        }))
}

fn get_sys() -> ActorSystem {
    (*ACTOR_SYSTEM.lock().expect("Unable to communicate with actor system")).clone()
        .expect("Actor system not set properly")
}

fn get_actor() -> RpcServerRef {
    (*RPC_ACTOR.lock().expect("Unable to contact rpc actor")).clone()
        .expect("RPC actor not set properly")
}

fn timestamp() -> TimeStamp {
    TimeStamp::Integral(Utc::now().timestamp())
}

fn not_found() -> ServiceResult {
    Ok(Response::builder()
        .status(StatusCode::from_u16(404)?)
        .body(Body::from("not found"))?)
}

fn empty() -> ServiceResult {
    Ok(Response::builder()
        .status(StatusCode::from_u16(204)?)
        .body(Body::empty())?)
}

async fn active_chains() -> ServiceResult {
    Ok(Response::new(Body::from("10101010")))
}

async fn bootstrapped() -> ServiceResult {
    use crate::server::control_msg::GetCurrentHead;
    use crate::encoding::monitor::BootstrapInfo;

    let sys = get_sys();
    let rpc_actor = get_actor();
    let current_head = ask(&sys, &rpc_actor, GetCurrentHead::Request).await;
    if let GetCurrentHead::Response(current_head) = current_head {
        let resp = serde_json::to_string(&if let Some(current_head) = current_head {
            let hash = HashEncoding::new(HashType::BlockHash).bytes_to_string(&current_head.hash());
            BootstrapInfo::new(hash.into(), timestamp())
        } else {
            BootstrapInfo::new("".into(), timestamp())
        })?;
        Ok(Response::new(Body::from(resp)))
    } else {
        empty()
    }
}


async fn router(req: Request<Body>) -> ServiceResult {
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/monitor/active_chains") => active_chains().await,
        (&Method::GET, "/monitor/bootstrapped") => bootstrapped().await,
        _ => not_found()
    }
}