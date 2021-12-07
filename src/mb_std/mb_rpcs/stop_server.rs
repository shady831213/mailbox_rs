use super::{MBAsyncRPC, MBAsyncRPCError, MBAsyncRPCResult};
use crate::mb_channel::*;
use crate::mb_rpcs::*;
use crate::mb_std::mb_async_channel::*;
use crate::mb_std::mb_ptr_resolver::*;
use async_std::prelude::*;
use async_std::task::Context;
use async_std::task::Poll;

impl<RA: MBPtrReader, WA: MBPtrWriter, R: MBPtrResolver<READER = RA, WRITER = WA>>
    MBAsyncRPC<RA, WA, R> for MBStopServer
{
    fn poll_cmd(
        &self,
        _server_name: &str,
        _r: &R,
        _req: &MBReqEntry,
        _cx: &mut Context,
    ) -> Poll<MBAsyncRPCResult> {
        Poll::Ready(Err(MBAsyncRPCError::Stop))
    }
}

pub fn mb_stop_server<CH: MBChannelIf>(
    sender: &MBAsyncSender<CH>,
) -> impl Future<Output = ()> + '_ {
    async move {
        sender.send_req(&MBStopServer, ()).await;
    }
}
