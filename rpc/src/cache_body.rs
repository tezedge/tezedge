use hyper::body::{Buf, HttpBody};
use std::{
    pin::Pin,
    task::{Context, Poll},
};

pub enum CacheBody<B> {
    Ready(Option<String>),
    Inner(B),
}

pub enum CacheBodyData<D>
where
    D: Buf,
{
    Ready(Vec<u8>),
    Inner(D),
}

impl<D> Buf for CacheBodyData<D>
where
    D: Buf,
{
    fn remaining(&self) -> usize {
        match self {
            &CacheBodyData::Ready(ref v) => v.len(),
            &CacheBodyData::Inner(ref d) => d.remaining(),
        }
    }

    fn chunk(&self) -> &[u8] {
        match self {
            &CacheBodyData::Ready(ref v) => v.as_ref(),
            &CacheBodyData::Inner(ref d) => d.chunk(),
        }
    }

    fn advance(&mut self, cnt: usize) {
        match self {
            &mut CacheBodyData::Ready(ref mut v) => {
                *v = v[cnt..].to_vec();
            }
            &mut CacheBodyData::Inner(ref mut d) => d.advance(cnt),
        }
    }
}

impl<B> HttpBody for CacheBody<B>
where
    B: HttpBody + Unpin,
{
    type Data = CacheBodyData<B::Data>;
    type Error = B::Error;

    fn poll_data(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        match self.get_mut() {
            &mut CacheBody::Ready(ref mut s) => {
                if let Some(s) = s.take() {
                    Poll::Ready(Some(Ok(CacheBodyData::Ready(s.into_bytes()))))
                } else {
                    Poll::Ready(None)
                }
            }
            &mut CacheBody::Inner(ref mut b) => {
                let d = HttpBody::poll_data(Pin::new(b), cx);
                d.map(|d| d.map(|d| d.map(CacheBodyData::Inner)))
            }
        }
    }

    fn poll_trailers(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Option<hyper::HeaderMap>, Self::Error>> {
        match self.get_mut() {
            &mut CacheBody::Ready(_) => Poll::Ready(Ok(None)),
            &mut CacheBody::Inner(ref mut b) => HttpBody::poll_trailers(Pin::new(b), cx),
        }
    }
}
