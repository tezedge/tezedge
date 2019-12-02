#[derive(Clone, PartialEq, ::prost::Message, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorCommitHashRequest {}
#[derive(Clone, PartialEq, ::prost::Message, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorCommitHashReply {
    #[prost(string, tag = "1")]
    pub commit_hash: std::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetBlockRequest {
    #[prost(string, tag = "1")]
    pub chain_id: std::string::String,
    #[prost(string, tag = "2")]
    pub block_id: std::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetBlockReply {
    /// A block identifier (Base58Check-encoded)
    #[prost(string, tag = "1")]
    pub block_hash: std::string::String,
    #[prost(string, tag = "2")]
    pub chain_id: std::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HelloRequest {
    #[prost(string, tag = "1")]
    pub name: std::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HelloReply {
    #[prost(string, tag = "1")]
    pub message: std::string::String,
}
#[doc = r" Generated client implementations."]
pub mod client {
    #![allow(unused_variables, dead_code, missing_docs)]
    use tonic::codegen::*;
    pub struct TezedgeClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl TezedgeClient<tonic::transport::Channel> {
        #[doc = r" Attempt to create a new client by connecting to a given endpoint."]
        pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: std::convert::TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
        {
            let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
            Ok(Self::new(conn))
        }
    }
    impl<T> TezedgeClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody>,
        T::ResponseBody: Body + HttpBody + Send + 'static,
        T::Error: Into<StdError>,
        <T::ResponseBody as HttpBody>::Error: Into<StdError> + Send,
        <T::ResponseBody as HttpBody>::Data: Into<bytes::Bytes> + Send,
    {
        pub fn new(inner: T) -> Self {
            let inner = tonic::client::Grpc::new(inner);
            Self { inner }
        }
        #[doc = r" Check if the service is ready."]
        pub async fn ready(&mut self) -> Result<(), tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })
        }
        pub async fn get_block(
            &mut self,
            request: impl tonic::IntoRequest<super::GetBlockRequest>,
        ) -> Result<tonic::Response<super::GetBlockReply>, tonic::Status> {
            self.ready().await?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/tezedge.Tezedge/GetBlock");
            self.inner.unary(request.into_request(), path, codec).await
        }
        pub async fn monitor_commit_hash(
            &mut self,
            request: impl tonic::IntoRequest<super::MonitorCommitHashRequest>,
        ) -> Result<tonic::Response<super::MonitorCommitHashReply>, tonic::Status> {
            self.ready().await?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/tezedge.Tezedge/MonitorCommitHash");
            self.inner.unary(request.into_request(), path, codec).await
        }
        #[doc = " Our SayHello rpc accepts HelloRequests and returns HelloReplies"]
        pub async fn say_hello(
            &mut self,
            request: impl tonic::IntoRequest<super::HelloRequest>,
        ) -> Result<tonic::Response<super::HelloReply>, tonic::Status> {
            self.ready().await?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/tezedge.Tezedge/SayHello");
            self.inner.unary(request.into_request(), path, codec).await
        }
    }
    impl<T: Clone> Clone for TezedgeClient<T> {
        fn clone(&self) -> Self {
            Self {
                inner: self.inner.clone(),
            }
        }
    }
}
#[doc = r" Generated server implementations."]
pub mod server {
    #![allow(unused_variables, dead_code, missing_docs)]
    use tonic::codegen::*;
    #[doc = "Generated trait containing gRPC methods that should be implemented for use with TezedgeServer."]
    #[async_trait]
    pub trait Tezedge: Send + Sync + 'static {
        async fn get_block(
            &self,
            request: tonic::Request<super::GetBlockRequest>,
        ) -> Result<tonic::Response<super::GetBlockReply>, tonic::Status> {
            Err(tonic::Status::unimplemented("Not yet implemented"))
        }
        async fn monitor_commit_hash(
            &self,
            request: tonic::Request<super::MonitorCommitHashRequest>,
        ) -> Result<tonic::Response<super::MonitorCommitHashReply>, tonic::Status> {
            Err(tonic::Status::unimplemented("Not yet implemented"))
        }
        #[doc = " Our SayHello rpc accepts HelloRequests and returns HelloReplies"]
        async fn say_hello(
            &self,
            request: tonic::Request<super::HelloRequest>,
        ) -> Result<tonic::Response<super::HelloReply>, tonic::Status> {
            Err(tonic::Status::unimplemented("Not yet implemented"))
        }
    }
    #[derive(Debug)]
    #[doc(hidden)]
    pub struct TezedgeServer<T: Tezedge> {
        inner: Arc<T>,
    }
    impl<T: Tezedge> TezedgeServer<T> {
        pub fn new(inner: T) -> Self {
            let inner = Arc::new(inner);
            Self { inner }
        }
    }
    impl<T: Tezedge> Service<http::Request<HyperBody>> for TezedgeServer<T> {
        type Response = http::Response<tonic::body::BoxBody>;
        type Error = Never;
        type Future = BoxFuture<Self::Response, Self::Error>;
        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, req: http::Request<HyperBody>) -> Self::Future {
            let inner = self.inner.clone();
            match req.uri().path() {
                "/tezedge.Tezedge/GetBlock" => {
                    struct GetBlockSvc<T: Tezedge>(pub Arc<T>);
                    impl<T: Tezedge> tonic::server::UnaryService<super::GetBlockRequest> for GetBlockSvc<T> {
                        type Response = super::GetBlockReply;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::GetBlockRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { inner.get_block(request).await };
                            Box::pin(fut)
                        }
                    }
                    let inner = self.inner.clone();
                    let fut = async move {
                        let method = GetBlockSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec);
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/tezedge.Tezedge/MonitorCommitHash" => {
                    struct MonitorCommitHashSvc<T: Tezedge>(pub Arc<T>);
                    impl<T: Tezedge> tonic::server::UnaryService<super::MonitorCommitHashRequest>
                        for MonitorCommitHashSvc<T>
                    {
                        type Response = super::MonitorCommitHashReply;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::MonitorCommitHashRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { inner.monitor_commit_hash(request).await };
                            Box::pin(fut)
                        }
                    }
                    let inner = self.inner.clone();
                    let fut = async move {
                        let method = MonitorCommitHashSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec);
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/tezedge.Tezedge/SayHello" => {
                    struct SayHelloSvc<T: Tezedge>(pub Arc<T>);
                    impl<T: Tezedge> tonic::server::UnaryService<super::HelloRequest> for SayHelloSvc<T> {
                        type Response = super::HelloReply;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::HelloRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { inner.say_hello(request).await };
                            Box::pin(fut)
                        }
                    }
                    let inner = self.inner.clone();
                    let fut = async move {
                        let method = SayHelloSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec);
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                _ => Box::pin(async move {
                    Ok(http::Response::builder()
                        .status(200)
                        .header("grpc-status", "12")
                        .body(tonic::body::BoxBody::empty())
                        .unwrap())
                }),
            }
        }
    }
    impl<T: Tezedge> Clone for TezedgeServer<T> {
        fn clone(&self) -> Self {
            let inner = self.inner.clone();
            Self { inner }
        }
    }
    impl<T: Tezedge> tonic::transport::ServiceName for TezedgeServer<T> {
        const NAME: &'static str = "tezedge.Tezedge";
    }
}
