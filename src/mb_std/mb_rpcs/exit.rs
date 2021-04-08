use super::MBAsyncRPC;
use crate::mb_channel::*;
use crate::mb_rpcs::*;
use crate::mb_std::mb_async_channel::*;
use crate::mb_std::mb_server::*;
use async_std::prelude::*;
use async_std::task::Context;
use async_std::task::Poll;

impl<RA: MBPtrReader, WA: MBPtrWriter, R: MBPtrResolver<READER = RA, WRITER = WA>>
    MBAsyncRPC<RA, WA, R> for MBExit
{
    fn poll_cmd(
        &self,
        _server_name: &str,
        _r: &R,
        req: &MBReqEntry,
        _cx: &mut Context,
    ) -> Poll<Option<MBRespEntry>> {
        extern "C" {
            fn __mb_exit(code: u32);
        }
        unsafe {
            __mb_exit(req.args[0] as u32);
        }
        Poll::Ready(None)
    }
}

pub fn mb_exit<CH: MBChannelIf>(
    sender: &MBAsyncSender<CH>,
    code: u32,
) -> impl Future<Output = ()> + '_ {
    async move {
        sender.send_req(&MBExit, code).await;
    }
}
