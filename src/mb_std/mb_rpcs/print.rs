use super::{MBAsyncRPC, MBAsyncRPCResult};
use crate::mb_channel::*;
use crate::mb_rpcs::*;
use crate::mb_std::mb_async_channel::*;
use crate::mb_std::mb_ptr_resolver::*;
use async_std::prelude::*;
use async_std::task::Context;
use async_std::task::Poll;
use std::marker::PhantomData;
use std::sync::Mutex;

pub struct MBPrint<'a> {
    buf: Mutex<String>,
    _marker: PhantomData<&'a u8>,
}
impl<'a> MBPrint<'a> {
    pub fn new() -> MBPrint<'a> {
        MBPrint {
            buf: Mutex::new(String::new()),
            _marker: PhantomData,
        }
    }
}
impl<'a> MBRpc for MBPrint<'a> {
    type REQ = &'a MBStringArgs;
    type RESP = ();
    fn put_req(&self, req: Self::REQ, entry: &mut MBReqEntry) {
        entry.set_words(2);
        entry.set_action(MBAction::PRINT);
        entry.set_args(0, req.len as MBPtrT);
        entry.set_args(1, req.ptr);
        // entry.action = MBAction::PRINT;
        // entry.words = 2;
        // entry.args[0] = req.len as MBPtrT;
        // entry.args[1] = req.ptr;
    }
    fn get_resp(&self, _: &MBRespEntry) -> Self::RESP {}
}

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
        let s = r.read_str(&str_args).unwrap();
        {
            let mut buf = self.buf.lock().unwrap();
            *buf += &s;
            if buf.ends_with('\n') {
                print!("[{}] {}", server_name, buf);
                buf.clear();
            }
        }
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
