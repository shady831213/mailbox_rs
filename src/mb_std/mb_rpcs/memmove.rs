use super::MBAsyncRPC;
use crate::mb_channel::*;
use crate::mb_rpcs::*;
use crate::mb_std::mb_async_channel::*;
use crate::mb_std::mb_ptr_resolver::*;
use async_std::prelude::*;
use async_std::task::Context;
use async_std::task::Poll;

impl<'a, RA: MBPtrReader, WA: MBPtrWriter, R: MBPtrResolver<READER = RA, WRITER = WA>>
    MBAsyncRPC<RA, WA, R> for MBMemMove<'a>
{
    fn poll_cmd(
        &self,
        _server_name: &str,
        r: &R,
        req: &MBReqEntry,
        _cx: &mut Context,
    ) -> Poll<Option<MBRespEntry>> {
        let args = MBMemMoveArgs {
            dest: req.args[0],
            src: req.args[1],
            len: req.args[2],
        };
        let mut buf = vec![0u8; args.len as usize];
        r.read_slice(args.src as *const u8, &mut buf);
        r.write_slice(args.dest as *mut u8, &buf);
        Poll::Ready(Some(MBRespEntry::default()))
    }
}

pub fn mb_memmove<'a, CH: MBChannelIf>(
    sender: &'a MBAsyncSender<CH>,
    dest: MBPtrT,
    src: MBPtrT,
    len: MBPtrT,
) -> impl Future<Output = MBPtrT> + 'a {
    let memmove_rpc = MBMemMove::new();
    async move {
        let args = MBMemMoveArgs { dest, src, len };
        sender.send_req(&memmove_rpc, &args).await;
        sender.recv_resp(&memmove_rpc).await;
        dest
    }
}
