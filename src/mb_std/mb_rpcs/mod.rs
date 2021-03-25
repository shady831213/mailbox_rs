mod c_print;
mod exit;
mod print;
pub use c_print::*;
pub use exit::*;
pub use print::*;

use crate::mb_channel::*;
use crate::mb_std::mb_server::*;
use async_std::prelude::*;
use async_std::task::Context;
use async_std::task::Poll;
use std::pin::Pin;
pub trait MBAsyncRPC<RA: MBPtrReader, R: MBPtrResolver<READER = RA>> {
    fn poll_cmd(
        &self,
        server_name: &str,
        r: &R,
        req: &MBReqEntry,
        cx: &mut Context,
    ) -> Poll<Option<MBRespEntry>>;
    fn do_cmd<'a>(
        &'a self,
        server_name: &'a str,
        r: &'a R,
        req: &'a MBReqEntry,
    ) -> MBAsyncRPCFuture<'a, RA, R, Self>
    where
        Self: Sized,
    {
        MBAsyncRPCFuture {
            rpc: self,
            server_name,
            r,
            req,
        }
    }
}

pub trait CustomAsycRPC<RA: MBPtrReader, R: MBPtrResolver<READER = RA>>:
    MBAsyncRPC<RA, R> + Send
{
    fn is_me(&self, action: u32) -> bool;
}

pub struct MBAsyncRPCFuture<
    'a,
    RA: MBPtrReader,
    R: MBPtrResolver<READER = RA>,
    RPC: MBAsyncRPC<RA, R>,
> {
    rpc: &'a RPC,
    server_name: &'a str,
    r: &'a R,
    req: &'a MBReqEntry,
}

impl<'a, RA: MBPtrReader, R: MBPtrResolver<READER = RA>, RPC: MBAsyncRPC<RA, R>> Future
    for MBAsyncRPCFuture<'a, RA, R, RPC>
{
    type Output = Option<MBRespEntry>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        self.rpc.poll_cmd(self.server_name, self.r, self.req, cx)
    }
}
