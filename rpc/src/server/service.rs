use hyper::{Body, Response, Error, Server, Request, StatusCode, Method};
use hyper::service::{service_fn, make_service_fn};
use std::net::SocketAddr;
use lazy_static::lazy_static;
use std::sync::Mutex;
use futures::Future;
use crate::rpc_actor::RpcServerRef;

lazy_static! {
    static ref ACTOR_SYSTEM: Mutex<Option<RpcServerRef>> = Mutex::new(None);
}

type ServiceResult = Result<Response<Body>, Box<dyn std::error::Error + Sync + Send>>;

pub fn spawn_server(addr: &SocketAddr, sys: RpcServerRef) -> impl Future<Output=Result<(), Error>> {
    let mut state = ACTOR_SYSTEM.lock().unwrap();
    *state = Some(sys);

    Server::bind(addr)
        .serve(make_service_fn(|_| async move {
            Ok::<_, Error>(service_fn(move |req| {
                async move {
                    router(req)
                }
            }))
        }))
}

fn not_found() -> ServiceResult {
    Ok(Response::builder()
        .status(StatusCode::from_u16(404)?)
        .body(Body::from("not found"))?)
}

fn active_chains() -> ServiceResult {
    Ok(Response::new(Body::from("10101010")))
}

fn router(req: Request<Body>) -> ServiceResult {
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/monitor/active_chains") => active_chains(),
        _ => not_found()
    }
}