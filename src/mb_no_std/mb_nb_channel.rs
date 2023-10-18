use crate::mb_channel::*;
use crate::mb_rpcs::*;
extern crate nb;
extern crate spin;
use nb::block;
use spin::Mutex;
#[linkage = "weak"]
#[no_mangle]
extern "C" fn __mb_save_flag() -> MBPtrT {
    0
}

#[linkage = "weak"]
#[no_mangle]
extern "C" fn __mb_restore_flag(_flag: MBPtrT) {}

#[linkage = "weak"]
#[no_mangle]
extern "C" fn __mb_rfence(_start: MBPtrT, _size: usize) {}

#[linkage = "weak"]
#[no_mangle]
extern "C" fn __mb_wfence(_start: MBPtrT, _size: usize) {}

#[derive(Debug)]
pub enum MBNbSenderErr {
    NotReady,
}

pub trait MBNbSender {
    fn send_nb<REQ: Copy, RPC: MBRpc<REQ = REQ>>(&mut self, rpc: &RPC, req: REQ);
    fn send<REQ: Copy, RESP, RPC: MBRpc<REQ = REQ, RESP = RESP>>(
        &mut self,
        rpc: &RPC,
        req: REQ,
    ) -> RESP;
    fn reset(&mut self);
}

pub struct MBNbLockRefSender<CH: 'static + MBChannelIf>(Mutex<&'static mut CH>);

impl<CH: 'static + MBChannelIf> MBNbLockRefSender<CH> {
    pub const fn new(ch: &'static mut CH) -> MBNbLockRefSender<CH> {
        MBNbLockRefSender(Mutex::new(ch))
    }
    fn try_send<REQ: Copy, RPC: MBRpc<REQ = REQ>>(
        &self,
        rpc: &RPC,
        req: REQ,
        ch: &mut CH,
    ) -> nb::Result<(), ()> {
        __mb_rfence(ch as *mut _ as MBPtrT, core::mem::size_of::<CH>());
        if !ch.is_ready() {
            return Err(nb::Error::WouldBlock);
        }
        if !ch.req_can_put() {
            return Err(nb::Error::WouldBlock);
        }
        let entry = ch.put_req(rpc, req);
        __mb_wfence(entry, core::mem::size_of::<MBReqEntry>());
        let ptr_ptr = ch.commit_req();
        __mb_wfence(ptr_ptr, core::mem::size_of::<u32>());
        Ok(())
    }
    fn try_recv<RESP, RPC: MBRpc<RESP = RESP>>(
        &self,
        rpc: &RPC,
        ch: &mut CH,
    ) -> nb::Result<RESP, MBNbSenderErr> {
        __mb_rfence(ch as *mut _ as MBPtrT, core::mem::size_of::<CH>());
        if !ch.is_ready() {
            return Err(nb::Error::Other(MBNbSenderErr::NotReady));
        }
        if !ch.resp_can_get() {
            return Err(nb::Error::WouldBlock);
        }
        let ret = ch.get_resp(rpc);
        let ptr_ptr = ch.ack_resp();
        __mb_wfence(ptr_ptr, core::mem::size_of::<u32>());
        Ok(ret)
    }
}

impl<CH: 'static + MBChannelIf> MBNbSender for MBNbLockRefSender<CH> {
    fn send_nb<REQ: Copy, RPC: MBRpc<REQ = REQ>>(&mut self, rpc: &RPC, req: REQ) {
        let flag = __mb_save_flag();
        let mut ch = self.0.lock();
        block!(self.try_send(rpc, req, &mut ch)).unwrap();
        __mb_restore_flag(flag);
    }
    fn send<REQ: Copy, RESP, RPC: MBRpc<REQ = REQ, RESP = RESP>>(
        &mut self,
        rpc: &RPC,
        req: REQ,
    ) -> RESP {
        let flag = __mb_save_flag();
        let mut ch = self.0.lock();
        block!(self.try_send(rpc, req, &mut ch)).unwrap();
        let resp = block!(self.try_recv(rpc, &mut ch)).unwrap();
        __mb_restore_flag(flag);
        resp
    }
    fn reset(&mut self) {
        let mut ch = self.0.lock();
        ch.reset_req();
        __mb_wfence(&mut ch as *mut _ as MBPtrT, core::mem::size_of::<CH>());
    }
}

pub struct MBNbRefSender<CH: 'static + MBChannelIf>(&'static mut CH);

impl<CH: 'static + MBChannelIf> MBNbRefSender<CH> {
    pub const fn new(ch: &'static mut CH) -> MBNbRefSender<CH> {
        MBNbRefSender(ch)
    }
    fn try_send<REQ: Copy, RPC: MBRpc<REQ = REQ>>(
        &mut self,
        rpc: &RPC,
        req: REQ,
    ) -> nb::Result<(), ()> {
        __mb_rfence(self.0 as *const _ as MBPtrT, core::mem::size_of::<CH>());
        if !self.0.is_ready() {
            return Err(nb::Error::WouldBlock);
        }
        if !self.0.req_can_put() {
            return Err(nb::Error::WouldBlock);
        }
        let enrty = self.0.put_req(rpc, req);
        __mb_wfence(enrty, core::mem::size_of::<MBReqEntry>());
        let ptr_ptr = self.0.commit_req();
        __mb_wfence(ptr_ptr, core::mem::size_of::<u32>());
        Ok(())
    }
    fn try_recv<RESP, RPC: MBRpc<RESP = RESP>>(
        &mut self,
        rpc: &RPC,
    ) -> nb::Result<RESP, MBNbSenderErr> {
        __mb_rfence(self.0 as *const _ as MBPtrT, core::mem::size_of::<CH>());
        if !self.0.is_ready() {
            return Err(nb::Error::Other(MBNbSenderErr::NotReady));
        }
        if !self.0.resp_can_get() {
            return Err(nb::Error::WouldBlock);
        }
        let ret = self.0.get_resp(rpc);
        let ptr_ptr = self.0.ack_resp();
        __mb_wfence(ptr_ptr, core::mem::size_of::<u32>());
        Ok(ret)
    }
}

impl<CH: 'static + MBChannelIf> MBNbSender for MBNbRefSender<CH> {
    fn send_nb<REQ: Copy, RPC: MBRpc<REQ = REQ>>(&mut self, rpc: &RPC, req: REQ) {
        let flag = __mb_save_flag();
        block!(self.try_send(rpc, req)).unwrap();
        __mb_restore_flag(flag);
    }
    fn send<REQ: Copy, RESP, RPC: MBRpc<REQ = REQ, RESP = RESP>>(
        &mut self,
        rpc: &RPC,
        req: REQ,
    ) -> RESP {
        let flag = __mb_save_flag();
        block!(self.try_send(rpc, req)).unwrap();
        let resp = block!(self.try_recv(rpc)).unwrap();
        __mb_restore_flag(flag);
        resp
    }
    fn reset(&mut self) {
        self.0.reset_req();
        __mb_wfence(self.0 as *const _ as MBPtrT, core::mem::size_of::<CH>());
    }
}
