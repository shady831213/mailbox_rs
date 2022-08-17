use super::{MBAsyncRPC, MBAsyncRPCResult};
use crate::mb_channel::*;
use crate::mb_rpcs::*;
use crate::mb_std::mb_async_channel::*;
use crate::mb_std::mb_ptr_resolver::*;
use async_std::prelude::*;
use async_std::task::Context;
use async_std::task::Poll;

#[no_mangle]
#[linkage = "weak"]
pub unsafe extern "C" fn __mb_call(
    _ch_name: *const std::os::raw::c_char,
    _method: *const std::os::raw::c_char,
    _arg_len: u32,
    _args: *const MBPtrT,
    _status: &mut u32,
) -> MBPtrT {
    panic!("CALL is not implemented!")
}

impl<'a, RA: MBPtrReader, WA: MBPtrWriter, R: MBPtrResolver<READER = RA, WRITER = WA>>
    MBAsyncRPC<RA, WA, R> for MBCall<'a>
{
    fn poll_cmd(
        &self,
        server_name: &str,
        r: &R,
        req: &MBReqEntry,
        _cx: &mut Context,
    ) -> Poll<MBAsyncRPCResult> {
        let mut args = MBCallArgs {
            len: req.words - 1,
            method: req.args[0],
            args: [0; MB_MAX_ARGS - 1],
        };
        args.args[..args.len as usize].copy_from_slice(&req.args[1..req.words as usize]);
        let ch_name = std::ffi::CString::new(server_name).unwrap();
        let method_name = r.read_c_str(args.method as *const u8).unwrap();
        let method_name_c = std::ffi::CString::new(method_name).unwrap();
        let mut resp = MBRespEntry::default();
        resp.words = 1;
        let mut status: u32 = 0;
        unsafe {
            let ret = __mb_call(
                ch_name.as_ptr(),
                method_name_c.as_ptr(),
                args.len,
                args.args.as_ptr(),
                &mut status,
            );
            match status {
                x if x == MBCallStatus::Pending as u32 => Poll::Pending,
                x if x == MBCallStatus::Ready as u32 => {
                    resp.rets = ret;
                    Poll::Ready(Ok(resp))
                }
                _ => panic!("Unkown status {} for CALL!", status),
            }
        }
    }
}

pub fn mb_call<'a, CH: MBChannelIf>(
    sender: &'a MBAsyncSender<CH>,
    method: &'a str,
    args: &'a [MBPtrT],
) -> impl Future<Output = MBPtrT> + 'a {
    let call_rpc = MBCall::new();
    async move {
        let mut call_args = MBCallArgs {
            len: args.len() as u32,
            method: method.as_ptr() as MBPtrT,
            args: [0; MB_MAX_ARGS - 1],
        };
        call_args.args[..args.len()].copy_from_slice(&args[..args.len()]);
        sender.send_req(&call_rpc, &call_args).await;
        sender.recv_resp(&call_rpc).await
    }
}
