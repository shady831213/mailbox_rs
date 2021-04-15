use crate::mb_channel::*;
use core::marker::PhantomData;
#[cfg(feature = "ptr32")]
pub type MBPtrT = u32;
#[cfg(feature = "ptr64")]
pub type MBPtrT = u64;
#[cfg(feature = "ptrhost")]
pub type MBPtrT = usize;

#[derive(Debug, Copy, Clone)]
#[repr(u32)]
pub enum MBAction {
    IDLE = 0,
    EXIT = 1,
    PRINT = 2,
    CPRINT = 3,
    MEMMOVE = 4,
    MEMSET = 5,
    MEMCMP = 6,
    SVCALL = 7,
    FILEACCESS = 8,
    OTHER = 0x80000000,
}

impl Default for MBAction {
    fn default() -> Self {
        MBAction::IDLE
    }
}

pub struct MBExit;

impl MBRpc for MBExit {
    type REQ = u32;
    type RESP = ();
    fn put_req(&self, req: Self::REQ, entry: &mut MBReqEntry) {
        entry.words = 1;
        entry.action = MBAction::EXIT;
        entry.args[0] = req as MBPtrT;
    }
    fn get_resp(&self, _: &MBRespEntry) -> Self::RESP {}
}
#[derive(Default)]
#[repr(C)]
pub struct MBStringArgs {
    pub len: u32,
    pub ptr: MBPtrT,
}

pub struct MBPrint<'a> {
    _marker: PhantomData<&'a u8>,
}
impl<'a> MBPrint<'a> {
    pub fn new() -> MBPrint<'a> {
        MBPrint {
            _marker: PhantomData,
        }
    }
}
impl<'a> MBRpc for MBPrint<'a> {
    type REQ = &'a MBStringArgs;
    type RESP = ();
    fn put_req(&self, req: Self::REQ, entry: &mut MBReqEntry) {
        entry.action = MBAction::PRINT;
        entry.words = 2;
        entry.args[0] = req.len as MBPtrT;
        entry.args[1] = req.ptr;
    }
    fn get_resp(&self, _: &MBRespEntry) -> Self::RESP {}
}

pub const MB_CSTRING_MAX_ARGS: usize = 16;
#[derive(Default, Debug)]
#[repr(C)]
pub struct MBCStringArgs {
    pub len: u32,                            // -> MBReq.words
    pub fmt_str: MBPtrT,                     // -> MBReq.args[0]
    pub file: MBPtrT,                        // -> MBReq.args[1]
    pub pos: MBPtrT,                         // -> MBReq.args[2]
    pub args: [MBPtrT; MB_CSTRING_MAX_ARGS], // -> MBReq.args[3..]
}
impl MBCStringArgs {
    pub const fn dir_args_len(&self) -> usize {
        if self.len as usize > MB_MAX_ARGS {
            MB_MAX_ARGS - 1 - 3
        } else {
            self.len as usize - 3
        }
    }
    pub const fn rest_args_len(&self) -> usize {
        if self.len as usize > MB_MAX_ARGS {
            self.len as usize - MB_MAX_ARGS + 1
        } else {
            0
        }
    }
    pub const fn args_len(&self) -> usize {
        self.len as usize - 3
    }
}

pub struct MBCPrint<'a> {
    _marker: PhantomData<&'a u8>,
}
impl<'a> MBCPrint<'a> {
    pub fn new() -> MBCPrint<'a> {
        MBCPrint {
            _marker: PhantomData,
        }
    }
}

impl<'a> MBRpc for MBCPrint<'a> {
    type REQ = &'a MBCStringArgs;
    type RESP = ();
    fn put_req(&self, req: Self::REQ, entry: &mut MBReqEntry) {
        entry.action = MBAction::CPRINT;
        entry.words = req.len;
        entry.args[0] = req.fmt_str;
        entry.args[1] = req.file;
        entry.args[2] = req.pos;
        for (i, d) in req.args[..req.dir_args_len()].iter().enumerate() {
            entry.args[3 + i] = *d
        }
        if req.rest_args_len() > 0 {
            entry.args[MB_MAX_ARGS - 1] = req.args[req.dir_args_len()..].as_ptr() as MBPtrT;
        }
    }
    fn get_resp(&self, _: &MBRespEntry) -> Self::RESP {}
}

#[derive(Default, Debug)]
#[repr(C)]
pub struct MBMemMoveArgs {
    pub dest: MBPtrT,
    pub src: MBPtrT,
    pub len: MBPtrT,
}
pub struct MBMemMove<'a> {
    _marker: PhantomData<&'a u8>,
}
impl<'a> MBMemMove<'a> {
    pub fn new() -> MBMemMove<'a> {
        MBMemMove {
            _marker: PhantomData,
        }
    }
}
impl<'a> MBRpc for MBMemMove<'a> {
    type REQ = &'a MBMemMoveArgs;
    type RESP = ();
    fn put_req(&self, req: Self::REQ, entry: &mut MBReqEntry) {
        entry.words = 3;
        entry.action = MBAction::MEMMOVE;
        entry.args[0] = req.dest;
        entry.args[1] = req.src;
        entry.args[2] = req.len;
    }
    fn get_resp(&self, _: &MBRespEntry) -> Self::RESP {}
}

#[derive(Default, Debug)]
#[repr(C)]
pub struct MBMemSetArgs {
    pub dest: MBPtrT,
    pub data: MBPtrT,
    pub len: MBPtrT,
}
pub struct MBMemSet<'a> {
    _marker: PhantomData<&'a u8>,
}
impl<'a> MBMemSet<'a> {
    pub fn new() -> MBMemSet<'a> {
        MBMemSet {
            _marker: PhantomData,
        }
    }
}
impl<'a> MBRpc for MBMemSet<'a> {
    type REQ = &'a MBMemSetArgs;
    type RESP = ();
    fn put_req(&self, req: Self::REQ, entry: &mut MBReqEntry) {
        entry.words = 3;
        entry.action = MBAction::MEMSET;
        entry.args[0] = req.dest;
        entry.args[1] = req.data;
        entry.args[2] = req.len;
    }
    fn get_resp(&self, _: &MBRespEntry) -> Self::RESP {}
}

#[derive(Default, Debug)]
#[repr(C)]
pub struct MBMemCmpArgs {
    pub s1: MBPtrT,
    pub s2: MBPtrT,
    pub len: MBPtrT,
}
pub struct MBMemCmp<'a> {
    _marker: PhantomData<&'a u8>,
}
impl<'a> MBMemCmp<'a> {
    pub fn new() -> MBMemCmp<'a> {
        MBMemCmp {
            _marker: PhantomData,
        }
    }
}
impl<'a> MBRpc for MBMemCmp<'a> {
    type REQ = &'a MBMemCmpArgs;
    type RESP = i32;
    fn put_req(&self, req: Self::REQ, entry: &mut MBReqEntry) {
        entry.words = 3;
        entry.action = MBAction::MEMCMP;
        entry.args[0] = req.s1;
        entry.args[1] = req.s2;
        entry.args[2] = req.len;
    }
    fn get_resp(&self, resp: &MBRespEntry) -> Self::RESP {
        resp.rets as i32
    }
}

#[repr(u32)]
pub enum MBSvCallStatus {
    Ready = 0,
    Pending = 1,
}

#[derive(Default, Debug)]
#[repr(C)]
pub struct MBSvCallArgs {
    pub len: u32,                        // -> MBReq.words
    pub method: MBPtrT,                  // -> MBReq.args[0]
    pub args: [MBPtrT; MB_MAX_ARGS - 1], // -> MBReq.args[1..]
}

pub struct MBSvCall<'a> {
    _marker: PhantomData<&'a u8>,
}
impl<'a> MBSvCall<'a> {
    pub fn new() -> MBSvCall<'a> {
        MBSvCall {
            _marker: PhantomData,
        }
    }
}

impl<'a> MBRpc for MBSvCall<'a> {
    type REQ = &'a MBSvCallArgs;
    type RESP = MBPtrT;
    fn put_req(&self, req: Self::REQ, entry: &mut MBReqEntry) {
        entry.action = MBAction::SVCALL;
        entry.words = req.len + 1;
        entry.args[0] = req.method;
        //can not use memcpy!
        for (i, d) in req.args[0..req.len as usize].iter().enumerate() {
            entry.args[1 + i] = *d
        }
    }
    fn get_resp(&self, resp: &MBRespEntry) -> Self::RESP {
        resp.rets
    }
}

pub const MB_FILE_READ: u32 = 0x1;
pub const MB_FILE_WRITE: u32 = 0x2;
pub const MB_FILE_APPEND: u32 = 0x4;
pub const MB_FILE_TRUNC: u32 = 0x8;

#[repr(u32)]
pub enum MBFileAction {
    OPEN = 0,
    CLOSE = 1,
    READ = 2,
    WRITE = 3,
    SEEK = 4,
}

#[derive(Default, Debug)]
#[repr(C)]
pub struct MBFOpenArgs {
    pub path: MBPtrT, // -> MBReq.args[1]
    pub flags: u32,   // -> MBReq.args[2]
}

pub struct MBFOpen<'a> {
    _marker: PhantomData<&'a u8>,
}
impl<'a> MBFOpen<'a> {
    pub fn new() -> MBFOpen<'a> {
        MBFOpen {
            _marker: PhantomData,
        }
    }
}

impl<'a> MBRpc for MBFOpen<'a> {
    type REQ = &'a MBFOpenArgs;
    type RESP = u32;
    fn put_req(&self, req: Self::REQ, entry: &mut MBReqEntry) {
        entry.action = MBAction::FILEACCESS;
        entry.words = 3;
        entry.args[0] = MBFileAction::OPEN as MBPtrT;
        entry.args[1] = req.path;
        entry.args[2] = req.flags as MBPtrT;
    }
    fn get_resp(&self, resp: &MBRespEntry) -> Self::RESP {
        resp.rets as u32
    }
}

pub struct MBFClose;

impl MBRpc for MBFClose {
    type REQ = u32;
    type RESP = ();
    fn put_req(&self, req: Self::REQ, entry: &mut MBReqEntry) {
        entry.action = MBAction::FILEACCESS;
        entry.words = 2;
        entry.args[0] = MBFileAction::CLOSE as MBPtrT;
        entry.args[1] = req as MBPtrT;
    }
    fn get_resp(&self, _resp: &MBRespEntry) -> Self::RESP {}
}

#[derive(Default, Debug)]
#[repr(C)]
pub struct MBFReadArgs {
    pub fd: u32,     // -> MBReq.args[1]
    pub ptr: MBPtrT, // -> MBReq.args[2]
    pub len: u32,    // -> MBReq.args[3]
}

pub struct MBFRead<'a> {
    _marker: PhantomData<&'a u8>,
}
impl<'a> MBFRead<'a> {
    pub fn new() -> MBFRead<'a> {
        MBFRead {
            _marker: PhantomData,
        }
    }
}

impl<'a> MBRpc for MBFRead<'a> {
    type REQ = &'a MBFReadArgs;
    type RESP = usize;
    fn put_req(&self, req: Self::REQ, entry: &mut MBReqEntry) {
        entry.action = MBAction::FILEACCESS;
        entry.words = 4;
        entry.args[0] = MBFileAction::READ as MBPtrT;
        entry.args[1] = req.fd as MBPtrT;
        entry.args[2] = req.ptr;
        entry.args[3] = req.len as MBPtrT;
    }
    fn get_resp(&self, resp: &MBRespEntry) -> Self::RESP {
        resp.rets as usize
    }
}

#[derive(Default, Debug)]
#[repr(C)]
pub struct MBFWriteArgs {
    pub fd: u32,     // -> MBReq.args[1]
    pub ptr: MBPtrT, // -> MBReq.args[2]
    pub len: u32,    // -> MBReq.args[3]
}

pub struct MBFWrite<'a> {
    _marker: PhantomData<&'a u8>,
}
impl<'a> MBFWrite<'a> {
    pub fn new() -> MBFWrite<'a> {
        MBFWrite {
            _marker: PhantomData,
        }
    }
}

impl<'a> MBRpc for MBFWrite<'a> {
    type REQ = &'a MBFWriteArgs;
    type RESP = usize;
    fn put_req(&self, req: Self::REQ, entry: &mut MBReqEntry) {
        entry.action = MBAction::FILEACCESS;
        entry.words = 4;
        entry.args[0] = MBFileAction::WRITE as MBPtrT;
        entry.args[1] = req.fd as MBPtrT;
        entry.args[2] = req.ptr;
        entry.args[3] = req.len as MBPtrT;
    }
    fn get_resp(&self, resp: &MBRespEntry) -> Self::RESP {
        resp.rets as usize
    }
}

#[derive(Default, Debug)]
#[repr(C)]
pub struct MBFSeekArgs {
    pub fd: u32,     // -> MBReq.args[1]
    pub pos: MBPtrT, // -> MBReq.args[2]
}

pub struct MBFSeek<'a> {
    _marker: PhantomData<&'a u8>,
}
impl<'a> MBFSeek<'a> {
    pub fn new() -> MBFSeek<'a> {
        MBFSeek {
            _marker: PhantomData,
        }
    }
}

impl<'a> MBRpc for MBFSeek<'a> {
    type REQ = &'a MBFSeekArgs;
    type RESP = MBPtrT;
    fn put_req(&self, req: Self::REQ, entry: &mut MBReqEntry) {
        entry.action = MBAction::FILEACCESS;
        entry.words = 3;
        entry.args[0] = MBFileAction::SEEK as MBPtrT;
        entry.args[1] = req.fd as MBPtrT;
        entry.args[2] = req.pos;
    }
    fn get_resp(&self, resp: &MBRespEntry) -> Self::RESP {
        resp.rets
    }
}
