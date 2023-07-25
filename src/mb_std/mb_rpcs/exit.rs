use super::{MBAsyncRPC, MBAsyncRPCError, MBAsyncRPCResult};
use crate::mb_channel::*;
use crate::mb_rpcs::*;
use crate::mb_std::mb_async_channel::*;
use crate::mb_std::mb_ptr_resolver::*;
use async_std::prelude::*;
use async_std::task::Context;
use async_std::task::Poll;

impl<RA: MBPtrReader, WA: MBPtrWriter, R: MBPtrResolver<READER = RA, WRITER = WA>>
    MBAsyncRPC<RA, WA, R> for MBExit
{
    fn poll_cmd(
        &self,
        server_name: &str,
        _r: &R,
        req: &MBReqEntry,
        _cx: &mut Context,
    ) -> Poll<MBAsyncRPCResult> {
        Poll::Ready(Err(MBAsyncRPCError::Stop(
            server_name.to_string(),
            req.args[0] as u32,
        )))
    }
}

pub fn mb_exit<CH: MBChannelIf>(
    sender: &MBAsyncSender<CH>,
    code: u32,
) -> impl Future<Output = ()> + '_ {
    async move { sender.send_req(&MBExit, code).await }
}
