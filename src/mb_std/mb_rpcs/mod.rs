mod c_print;
mod exit;
mod memcmp;
mod memmove;
mod memset;
mod print;
mod svcall;
pub use c_print::*;
pub use exit::*;
pub use memcmp::*;
pub use memmove::*;
pub use memset::*;
pub use print::*;
pub use svcall::*;

use crate::mb_channel::*;
use crate::mb_std::mb_ptr_resolver::*;
use async_std::prelude::*;
use async_std::task::Context;
use async_std::task::Poll;
use std::pin::Pin;
pub trait MBAsyncRPC<RA: MBPtrReader, WA: MBPtrWriter, R: MBPtrResolver<READER = RA, WRITER = WA>> {
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
    ) -> MBAsyncRPCFuture<'a, RA, WA, R, Self>
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

pub trait CustomAsycRPC<
    RA: MBPtrReader,
    WA: MBPtrWriter,
    R: MBPtrResolver<READER = RA, WRITER = WA>,
>: MBAsyncRPC<RA, WA, R> + Send
{
    fn is_me(&self, action: u32) -> bool;
}

pub struct MBAsyncRPCFuture<
    'a,
    RA: MBPtrReader,
    WA: MBPtrWriter,
    R: MBPtrResolver<READER = RA, WRITER = WA>,
    RPC: MBAsyncRPC<RA, WA, R>,
> {
    rpc: &'a RPC,
    server_name: &'a str,
    r: &'a R,
    req: &'a MBReqEntry,
}

impl<
        'a,
        RA: MBPtrReader,
        WA: MBPtrWriter,
        R: MBPtrResolver<READER = RA, WRITER = WA>,
        RPC: MBAsyncRPC<RA, WA, R>,
    > Future for MBAsyncRPCFuture<'a, RA, WA, R, RPC>
{
    type Output = Option<MBRespEntry>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        self.rpc.poll_cmd(self.server_name, self.r, self.req, cx)
    }
}
