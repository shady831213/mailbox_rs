use super::{MBAsyncRPC, MBAsyncRPCError, MBAsyncRPCResult};
use crate::mb_channel::*;
use crate::mb_rpcs::*;
use crate::mb_std::mb_async_channel::*;
use crate::mb_std::mb_fs::*;
use crate::mb_std::mb_ptr_resolver::*;
use async_std::prelude::*;
use async_std::task::Context;
use async_std::task::Poll;
impl MBFs {
    fn poll_open<RA: MBPtrReader, WA: MBPtrWriter, R: MBPtrResolver<READER = RA, WRITER = WA>>(
        &self,
        r: &R,
        args: &MBFOpenArgs,
    ) -> Poll<MBAsyncRPCResult> {
        let mut resp = MBRespEntry::default();
        resp.words = 1;
        let path = r.read_c_str(args.path as *const u8).unwrap();
        resp.rets = self.open(path.as_str(), args.flags).unwrap() as MBPtrT;
        Poll::Ready(Ok(resp))
    }
    fn poll_close(&self, fd: u32) -> Poll<MBAsyncRPCResult> {
        self.close(fd).unwrap();
        Poll::Ready(Err(MBAsyncRPCError::NoResp))
    }
    fn poll_read<RA: MBPtrReader, WA: MBPtrWriter, R: MBPtrResolver<READER = RA, WRITER = WA>>(
        &self,
        r: &R,
        args: &MBFReadArgs,
    ) -> Poll<MBAsyncRPCResult> {
        match self.read(r, args.fd, args.ptr as *mut u8, args.len as usize) {
            Ok(len) => {
                let mut resp = MBRespEntry::default();
                resp.words = 1;
                resp.rets = len as MBPtrT;
                Poll::Ready(Ok(resp))
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Poll::Pending,
            Err(e) => panic!("{:?}", e),
        }
    }
    fn poll_write<RA: MBPtrReader, WA: MBPtrWriter, R: MBPtrResolver<READER = RA, WRITER = WA>>(
        &self,
        r: &R,
        args: &MBFWriteArgs,
    ) -> Poll<MBAsyncRPCResult> {
        match self.write(r, args.fd, args.ptr as *const u8, args.len as usize) {
            Ok(len) => {
                let mut resp = MBRespEntry::default();
                resp.words = 1;
                resp.rets = len as MBPtrT;
                Poll::Ready(Ok(resp))
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Poll::Pending,
            Err(e) => panic!("{:?}", e),
        }
    }
    fn poll_seek(&self, args: &MBFSeekArgs) -> Poll<MBAsyncRPCResult> {
        let mut resp = MBRespEntry::default();
        resp.words = 1;
        resp.rets = self.seek(args.fd, args.pos as u64).unwrap() as MBPtrT;
        Poll::Ready(Ok(resp))
    }
}

impl<RA: MBPtrReader, WA: MBPtrWriter, R: MBPtrResolver<READER = RA, WRITER = WA>>
    MBAsyncRPC<RA, WA, R> for MBFs
{
    fn poll_cmd(
        &self,
        _server_name: &str,
        r: &R,
        req: &MBReqEntry,
        _cx: &mut Context,
    ) -> Poll<MBAsyncRPCResult> {
        let file_action = req.args[0] as u32;
        let mut resp = MBRespEntry::default();
        resp.words = 1;
        match file_action {
            a if a == MBFileAction::OPEN as u32 => {
                let args = MBFOpenArgs {
                    path: req.args[1],
                    flags: req.args[2] as u32,
                };
                self.poll_open(r, &args)
            }
            a if a == MBFileAction::CLOSE as u32 => self.poll_close(req.args[1] as u32),
            a if a == MBFileAction::READ as u32 => {
                let args = MBFReadArgs {
                    fd: req.args[1] as u32,
                    ptr: req.args[2],
                    len: req.args[3],
                };
                self.poll_read(r, &args)
            }
            a if a == MBFileAction::WRITE as u32 => {
                let args = MBFWriteArgs {
                    fd: req.args[1] as u32,
                    ptr: req.args[2],
                    len: req.args[3],
                };
                self.poll_write(r, &args)
            }
            a if a == MBFileAction::SEEK as u32 => {
                let args = MBFSeekArgs {
                    fd: req.args[1] as u32,
                    pos: req.args[2],
                };
                self.poll_seek(&args)
            }
            _ => panic!("Unkown MBFileAction {:#x}!", file_action),
        }
    }
}

pub fn mb_fopen<'a, CH: MBChannelIf>(
    sender: &'a MBAsyncSender<CH>,
    path: &'a str,
    flags: u32,
) -> impl Future<Output = u32> + 'a {
    let fopen_rpc = MBFOpen::new();
    async move {
        let args = MBFOpenArgs {
            path: path.as_ptr() as MBPtrT,
            flags,
        };
        sender.send_req(&fopen_rpc, &args).await;
        sender.recv_resp(&fopen_rpc).await
    }
}

pub fn mb_fclose<CH: MBChannelIf>(
    sender: &MBAsyncSender<CH>,
    fd: u32,
) -> impl Future<Output = ()> + '_ {
    let fclose_rpc = MBFClose;
    async move {
        sender.send_req(&fclose_rpc, fd).await;
    }
}

pub fn mb_fread<'a, CH: MBChannelIf>(
    sender: &'a MBAsyncSender<CH>,
    fd: u32,
    ptr: MBPtrT,
    len: MBPtrT,
) -> impl Future<Output = usize> + 'a {
    let fread_rpc = MBFRead::new();
    async move {
        let args = MBFReadArgs { fd, ptr, len };
        sender.send_req(&fread_rpc, &args).await;
        sender.recv_resp(&fread_rpc).await
    }
}

pub fn mb_fwrite<'a, CH: MBChannelIf>(
    sender: &'a MBAsyncSender<CH>,
    fd: u32,
    ptr: MBPtrT,
    len: MBPtrT,
) -> impl Future<Output = usize> + 'a {
    let fwrite_rpc = MBFWrite::new();
    async move {
        let args = MBFWriteArgs { fd, ptr, len };
        sender.send_req(&fwrite_rpc, &args).await;
        sender.recv_resp(&fwrite_rpc).await
    }
}

pub fn mb_fseek<'a, CH: MBChannelIf>(
    sender: &'a MBAsyncSender<CH>,
    fd: u32,
    pos: MBPtrT,
) -> impl Future<Output = MBPtrT> + 'a {
    let fseek_rpc = MBFSeek::new();
    async move {
        let args = MBFSeekArgs { fd, pos };
        sender.send_req(&fseek_rpc, &args).await;
        sender.recv_resp(&fseek_rpc).await
    }
}
