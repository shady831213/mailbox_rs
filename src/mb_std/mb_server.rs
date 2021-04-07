use super::mb_rpcs::*;
use super::mb_share_mem::*;
use crate::mb_channel::*;
use crate::mb_rpcs::*;
use async_std::prelude::*;
use async_std::task::Context;
use async_std::task::Poll;
use std::io::{BufRead, BufReader, Read};
use std::sync::Arc;
use std::sync::Mutex;

pub trait MBPtrReader: Read {
    fn read_slice<T: Sized + Copy>(&mut self, data: &mut [T]);
    fn read_sized<T: Sized>(&mut self, data: &mut T);
}

pub trait MBPtrResolver {
    type READER: MBPtrReader;
    fn reader<T: Sized>(&self, ptr: *const T) -> Self::READER;
    fn read_slice<T: Sized + Copy>(&self, ptr: *const T, data: &mut [T]) {
        self.reader(ptr).read_slice(data)
    }
    fn read_sized<T: Sized>(&self, ptr: *const T, data: &mut T) {
        self.reader(ptr).read_sized(data)
    }
    fn read_str(&self, str_args: &MBStringArgs) -> Result<String, String> {
        let str_len = str_args.len as usize;
        let raw_ptr = str_args.ptr as *const usize;
        let mut buf_reader = BufReader::new(self.reader(raw_ptr as *const u8));
        let mut buf = vec![0u8; str_len];
        buf_reader.read_exact(&mut buf).map_err(|e| e.to_string())?;
        Ok(String::from_utf8(buf).map_err(|e| e.to_string())?)
    }
    fn read_c_str(&self, ptr: *const u8) -> Result<String, String> {
        let mut buf_reader = BufReader::new(self.reader(ptr).take(4096));
        let mut buf = vec![];
        buf_reader
            .read_until(b'\0', &mut buf)
            .map_err(|e| e.to_string())?;
        if buf[buf.len() - 1] == 0 {
            buf = buf[..buf.len() - 1].to_vec();
        }
        let string = String::from_utf8(buf).map_err(|e| e.to_string())?;
        Ok(string)
    }
}

struct MBServerInner<RA: MBPtrReader, R: MBPtrResolver<READER = RA>> {
    exit: MBExit,
    print: MBPrint<'static>,
    cprint: MBCPrint<'static>,
    other_cmds: Mutex<Vec<Box<dyn CustomAsycRPC<RA, R>>>>,
}
impl<RA: MBPtrReader, R: MBPtrResolver<READER = RA>> MBServerInner<RA, R> {
    fn new() -> MBServerInner<RA, R> {
        MBServerInner {
            exit: MBExit,
            print: MBPrint::new(),
            cprint: MBCPrint::new(),
            other_cmds: Mutex::new(vec![]),
        }
    }
    fn add_cmd<C: CustomAsycRPC<RA, R> + 'static>(&self, cmd: C) {
        self.other_cmds.lock().unwrap().push(Box::new(cmd));
    }
}

impl<RA: MBPtrReader, R: MBPtrResolver<READER = RA>> MBAsyncRPC<RA, R> for MBServerInner<RA, R> {
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

pub struct MBLocalPtrReader {
    ptr: *const u8,
}
impl MBLocalPtrReader {
    fn new(ptr: *const u8) -> MBLocalPtrReader {
        MBLocalPtrReader { ptr }
    }
}
impl MBPtrReader for MBLocalPtrReader {
    fn read_slice<T: Sized + Copy>(&mut self, data: &mut [T]) {
        unsafe {
            std::slice::from_raw_parts_mut(data.as_mut_ptr() as *mut T, data.len())
                .copy_from_slice(std::slice::from_raw_parts(self.ptr as *const T, data.len()))
        }
    }
    fn read_sized<T: Sized>(&mut self, data: &mut T) {
        unsafe {
            std::slice::from_raw_parts_mut(data as *mut T as *mut u8, std::mem::size_of::<T>())
                .copy_from_slice(std::slice::from_raw_parts(
                    self.ptr as *const T as *const u8,
                    std::mem::size_of::<T>(),
                ))
        }
    }
}

impl Read for MBLocalPtrReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let len = buf.len();
        unsafe {
            std::slice::from_raw_parts_mut(buf.as_mut_ptr(), len)
                .copy_from_slice(std::slice::from_raw_parts(self.ptr, len))
        }
        Ok(len)
    }
}

#[derive(Default)]
pub struct MBLocalPtrResolver;
impl MBPtrResolver for MBLocalPtrResolver {
    type READER = MBLocalPtrReader;
    fn reader<T: Sized>(&self, ptr: *const T) -> Self::READER {
        MBLocalPtrReader::new(ptr as *const u8)
    }
}

pub struct MBLocalServer {
    name: String,
    resolver: MBLocalPtrResolver,
    inner: MBServerInner<MBLocalPtrReader, MBLocalPtrResolver>,
}

impl MBLocalServer {
    pub fn new(name: &str) -> MBLocalServer {
        MBLocalServer {
            name: name.to_string(),
            resolver: MBLocalPtrResolver::default(),
            inner: MBServerInner::new(),
        }
    }
    pub fn do_cmd<'a>(
        &'a self,
        req: &'a MBReqEntry,
    ) -> impl Future<Output = Option<MBRespEntry>> + 'a {
        self.inner.do_cmd(self.name.as_str(), &self.resolver, req)
    }
    pub fn add_cmd<C: CustomAsycRPC<MBLocalPtrReader, MBLocalPtrResolver> + 'static>(
        &self,
        cmd: C,
    ) {
        self.inner.add_cmd(cmd);
    }
}

pub struct MBSMPtrReader<SM: MBShareMem> {
    ptr: MBPtrT,
    sm: Arc<Mutex<SM>>,
}
impl<SM: MBShareMem> MBSMPtrReader<SM> {
    fn new(ptr: MBPtrT, sm: &Arc<Mutex<SM>>) -> MBSMPtrReader<SM> {
        MBSMPtrReader {
            ptr,
            sm: sm.clone(),
        }
    }
}
impl<SM: MBShareMem> MBPtrReader for MBSMPtrReader<SM> {
    fn read_slice<T: Sized + Copy>(&mut self, data: &mut [T]) {
        self.sm.lock().unwrap().read_slice(self.ptr, data);
    }
    fn read_sized<T: Sized>(&mut self, data: &mut T) {
        self.sm.lock().unwrap().read_sized(self.ptr, data);
    }
}

impl<SM: MBShareMem> Read for MBSMPtrReader<SM> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        Ok(self.sm.lock().unwrap().read(self.ptr, buf))
    }
}

pub struct MBSMPtrResolver<SM: MBShareMem> {
    sm: Arc<Mutex<SM>>,
}
impl<SM: MBShareMem> MBSMPtrResolver<SM> {
    pub fn new(sm: &Arc<Mutex<SM>>) -> MBSMPtrResolver<SM> {
        MBSMPtrResolver { sm: sm.clone() }
    }
}
impl<SM: MBShareMem> MBPtrResolver for MBSMPtrResolver<SM> {
    type READER = MBSMPtrReader<SM>;
    fn reader<T: Sized>(&self, ptr: *const T) -> Self::READER {
        MBSMPtrReader::new(ptr as MBPtrT, &self.sm)
    }
}

pub struct MBSMServer<SM: MBShareMem> {
    name: String,
    resolver: MBSMPtrResolver<SM>,
    inner: MBServerInner<MBSMPtrReader<SM>, MBSMPtrResolver<SM>>,
}

impl<SM: MBShareMem> MBSMServer<SM> {
    pub fn new(name: &str, sm: &Arc<Mutex<SM>>) -> MBSMServer<SM> {
        MBSMServer {
            name: name.to_string(),
            resolver: MBSMPtrResolver::new(sm),
            inner: MBServerInner::new(),
        }
    }
    pub fn do_cmd<'a>(
        &'a self,
        req: &'a MBReqEntry,
    ) -> impl Future<Output = Option<MBRespEntry>> + 'a {
        self.inner.do_cmd(self.name.as_str(), &self.resolver, req)
    }
    pub fn add_cmd<C: CustomAsycRPC<MBSMPtrReader<SM>, MBSMPtrResolver<SM>> + 'static>(
        &self,
        cmd: C,
    ) {
        self.inner.add_cmd(cmd);
    }
}
