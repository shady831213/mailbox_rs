use crate::mb_channel::*;
use core::marker::PhantomData;
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
