use super::mb_fs::*;
use super::mb_ptr_resolver::*;
use super::mb_rpcs::*;
use super::mb_share_mem::*;
use crate::mb_channel::*;
use crate::mb_rpcs::*;
use async_std::prelude::*;
use async_std::task::Context;
use async_std::task::Poll;
use std::sync::Arc;
use std::sync::Mutex;

struct MBServerInner<RA: MBPtrReader, WA: MBPtrWriter, R: MBPtrResolver<READER = RA, WRITER = WA>> {
    exit: MBExit,
    print: MBPrint<'static>,
    cprint: MBCPrint<'static>,
    memmove: MBMemMove<'static>,
    memset: MBMemSet<'static>,
    memcmp: MBMemCmp<'static>,
    svcall: MBSvCall<'static>,
    fs: Arc<Option<MBFs>>,
    other_cmds: Mutex<Vec<Box<dyn CustomAsycRPC<RA, WA, R>>>>,
}
impl<RA: MBPtrReader, WA: MBPtrWriter, R: MBPtrResolver<READER = RA, WRITER = WA>>
    MBServerInner<RA, WA, R>
{
    fn new(fs: &Arc<Option<MBFs>>) -> MBServerInner<RA, WA, R> {
        MBServerInner {
            exit: MBExit,
            print: MBPrint::new(),
            cprint: MBCPrint::new(),
            memmove: MBMemMove::new(),
            memset: MBMemSet::new(),
            memcmp: MBMemCmp::new(),
            svcall: MBSvCall::new(),
            fs: fs.clone(),
            other_cmds: Mutex::new(vec![]),
        }
    }
    fn add_cmd<C: CustomAsycRPC<RA, WA, R> + 'static>(&self, cmd: C) {
        self.other_cmds.lock().unwrap().push(Box::new(cmd));
    }
}

impl<RA: MBPtrReader, WA: MBPtrWriter, R: MBPtrResolver<READER = RA, WRITER = WA>>
    MBAsyncRPC<RA, WA, R> for MBServerInner<RA, WA, R>
{
    fn poll_cmd(
        &self,
        server_name: &str,
        r: &R,
        req: &MBReqEntry,
        cx: &mut Context,
    ) -> Poll<Option<MBRespEntry>> {
        match req.action {
            MBAction::EXIT => self.exit.poll_cmd(server_name, r, &req, cx),
            MBAction::PRINT => self.print.poll_cmd(server_name, r, &req, cx),
            MBAction::CPRINT => self.cprint.poll_cmd(server_name, r, &req, cx),
            MBAction::MEMMOVE => self.memmove.poll_cmd(server_name, r, &req, cx),
            MBAction::MEMSET => self.memset.poll_cmd(server_name, r, &req, cx),
            MBAction::MEMCMP => self.memcmp.poll_cmd(server_name, r, &req, cx),
            MBAction::SVCALL => self.svcall.poll_cmd(server_name, r, &req, cx),
            MBAction::FILEACCESS => {
                if let Some(fs) = &*self.fs {
                    fs.poll_cmd(server_name, r, &req, cx)
                } else {
                    panic!("No mb_fs in {}!", server_name)
                }
            }
            MBAction::OTHER => {
                let other_cmds = self.other_cmds.lock().unwrap();
                for cmd in other_cmds.iter() {
                    if cmd.is_me(req.args[0] as u32) {
                        return cmd.poll_cmd(server_name, r, &req, cx);
                    }
                }
                panic!("OTHER action {:#x} is not support!", req.args[0])
            }
            _ => Poll::Ready(None),
        }
    }
}

pub struct MBLocalServer {
    name: String,
    resolver: MBLocalPtrResolver,
    inner: MBServerInner<MBLocalPtrReader, MBLocalPtrWriter, MBLocalPtrResolver>,
}

impl MBLocalServer {
    pub fn new(name: &str, fs: &Arc<Option<MBFs>>) -> MBLocalServer {
        MBLocalServer {
            name: name.to_string(),
            resolver: MBLocalPtrResolver::default(),
            inner: MBServerInner::new(fs),
        }
    }
    pub fn do_cmd<'a>(
        &'a self,
        req: &'a MBReqEntry,
    ) -> impl Future<Output = Option<MBRespEntry>> + 'a {
        self.inner.do_cmd(self.name.as_str(), &self.resolver, req)
    }
    pub fn add_cmd<
        C: CustomAsycRPC<MBLocalPtrReader, MBLocalPtrWriter, MBLocalPtrResolver> + 'static,
    >(
        &self,
        cmd: C,
    ) {
        self.inner.add_cmd(cmd);
    }
}

pub struct MBSMServer<SM: MBShareMem> {
    name: String,
    resolver: MBSMPtrResolver<SM>,
    inner: MBServerInner<MBSMPtrReaderWrtier<SM>, MBSMPtrReaderWrtier<SM>, MBSMPtrResolver<SM>>,
}

impl<SM: MBShareMem> MBSMServer<SM> {
    pub fn new(name: &str, fs: &Arc<Option<MBFs>>, sm: &Arc<Mutex<SM>>) -> MBSMServer<SM> {
        MBSMServer {
            name: name.to_string(),
            resolver: MBSMPtrResolver::new(sm),
            inner: MBServerInner::new(fs),
        }
    }
    pub fn do_cmd<'a>(
        &'a self,
        req: &'a MBReqEntry,
    ) -> impl Future<Output = Option<MBRespEntry>> + 'a {
        self.inner.do_cmd(self.name.as_str(), &self.resolver, req)
    }
    pub fn add_cmd<
        C: CustomAsycRPC<MBSMPtrReaderWrtier<SM>, MBSMPtrReaderWrtier<SM>, MBSMPtrResolver<SM>>
            + 'static,
    >(
        &self,
        cmd: C,
    ) {
        self.inner.add_cmd(cmd);
    }
}
