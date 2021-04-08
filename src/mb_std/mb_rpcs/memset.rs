use super::MBAsyncRPC;
use crate::mb_channel::*;
use crate::mb_rpcs::*;
use crate::mb_std::mb_async_channel::*;
use crate::mb_std::mb_server::*;
use async_std::prelude::*;
use async_std::task::Context;
use async_std::task::Poll;

impl<'a, RA: MBPtrReader, WA: MBPtrWriter, R: MBPtrResolver<READER = RA, WRITER = WA>>
    MBAsyncRPC<RA, WA, R> for MBMemSet<'a>
{
    fn poll_cmd(
        &self,
        _server_name: &str,
        r: &R,
        req: &MBReqEntry,
        _cx: &mut Context,
    ) -> Poll<Option<MBRespEntry>> {
        let args = MBMemSetArgs {
            dest: req.args[0],
            data: req.args[1],
            len: req.args[2],
        };
        let data = args.data as u8;
        for i in 0..args.len as MBPtrT {
            r.write_sized((args.dest + i) as *mut u8, &data);
        }
        Poll::Ready(Some(MBRespEntry::default()))
    }
}

pub fn mb_memset<'a, CH: MBChannelIf>(
    sender: &'a MBAsyncSender<CH>,
    dest: MBPtrT,
    data: MBPtrT,
    len: MBPtrT,
) -> impl Future<Output = MBPtrT> + 'a {
    let memset_rpc = MBMemSet::new();
    async move {
        let args = MBMemSetArgs { dest, data, len };
        sender.send_req(&memset_rpc, &args).await;
        sender.recv_resp(&memset_rpc).await;
        dest
    }
}
