use super::MBAsyncRPC;
use crate::mb_channel::*;
use crate::mb_rpcs::*;
use crate::mb_std::mb_async_channel::*;
use crate::mb_std::mb_server::*;
use async_std::prelude::*;
use async_std::task::Context;
use async_std::task::Poll;

impl<'a, RA: MBPtrReader, WA: MBPtrWriter, R: MBPtrResolver<READER = RA, WRITER = WA>>
    MBAsyncRPC<RA, WA, R> for MBMemCmp<'a>
{
    fn poll_cmd(
        &self,
        _server_name: &str,
        r: &R,
        req: &MBReqEntry,
        _cx: &mut Context,
    ) -> Poll<Option<MBRespEntry>> {
        let args = MBMemCmpArgs {
            s1: req.args[0],
            s2: req.args[1],
            len: req.args[2],
        };
        let mut resp = MBRespEntry::default();
        resp.words = 1;
        for i in 0..args.len as MBPtrT {
            let mut s1: u8 = 0;
            let mut s2: u8 = 0;
            r.read_sized((args.s1 + i) as *const u8, &mut s1);
            r.read_sized((args.s2 + i) as *const u8, &mut s2);
            if s1 != s2 {
                resp.rets = (s1 as i32 - s2 as i32) as MBPtrT;
                return Poll::Ready(Some(resp));
            }
        }
        Poll::Ready(Some(resp))
    }
}

pub fn mb_memcmp<'a, CH: MBChannelIf>(
    sender: &'a MBAsyncSender<CH>,
    s1: MBPtrT,
    s2: MBPtrT,
    len: MBPtrT,
) -> impl Future<Output = i32> + 'a {
    let memcmp_rpc = MBMemCmp::new();
    async move {
        let args = MBMemCmpArgs { s1, s2, len };
        sender.send_req(&memcmp_rpc, &args).await;
        sender.recv_resp(&memcmp_rpc).await
    }
}
