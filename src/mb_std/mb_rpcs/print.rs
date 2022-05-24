use super::{MBAsyncRPC, MBAsyncRPCResult};
use crate::mb_channel::*;
use crate::mb_rpcs::*;
use crate::mb_std::mb_async_channel::*;
use crate::mb_std::mb_ptr_resolver::*;
use async_std::prelude::*;
use async_std::task::Context;
use async_std::task::Poll;

impl<'a, RA: MBPtrReader, WA: MBPtrWriter, R: MBPtrResolver<READER = RA, WRITER = WA>>
    MBAsyncRPC<RA, WA, R> for MBPrint<'a>
{
    fn poll_cmd(
        &self,
        server_name: &str,
        r: &R,
        req: &MBReqEntry,
        _cx: &mut Context,
    ) -> Poll<MBAsyncRPCResult> {
        let str_args = MBStringArgs {
            len: req.args[0] as u32,
            ptr: req.args[1],
        };
        print!("[{}] {}", server_name, r.read_str(&str_args).unwrap());
        Poll::Ready(Ok(MBRespEntry::default()))
    }
}

pub fn mb_print<'a, CH: MBChannelIf>(
    sender: &'a MBAsyncSender<CH>,
    msg: &'a str,
) -> impl Future<Output = ()> + 'a {
    let print_rpc = MBPrint::new();
    async move {
        let str_args = MBStringArgs {
            len: msg.len() as u32,
            ptr: msg.as_ptr() as MBPtrT,
        };
        sender.send_req(&print_rpc, &str_args).await;
        sender.recv_resp(&print_rpc).await
    }
}
