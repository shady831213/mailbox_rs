use crate::mb_channel::*;
use crate::mb_rpcs::*;
extern crate nb;
extern crate spin;
use nb::block;
use spin::Mutex;
extern "C" {
    fn __mb_save_flag() -> MBPtrT;
    fn __mb_restore_flag(flag: MBPtrT);
}

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
        if !ch.is_ready() {
            return Err(nb::Error::WouldBlock);
        }
        if !ch.req_can_put() {
            return Err(nb::Error::WouldBlock);
        }
        ch.put_req(rpc, req);
        Ok(())
    }
    fn try_recv<RESP, RPC: MBRpc<RESP = RESP>>(
        &self,
        rpc: &RPC,
        ch: &mut CH,
    ) -> nb::Result<RESP, MBNbSenderErr> {
        if !ch.is_ready() {
            return Err(nb::Error::Other(MBNbSenderErr::NotReady));
        }
        if !ch.resp_can_get() {
            return Err(nb::Error::WouldBlock);
        }
        let ret = ch.get_resp(rpc);
        Ok(ret)
    }
}

impl<CH: 'static + MBChannelIf> MBNbSender for MBNbLockRefSender<CH> {
    fn send_nb<REQ: Copy, RPC: MBRpc<REQ = REQ>>(&mut self, rpc: &RPC, req: REQ) {
        let flag = unsafe { __mb_save_flag() };
        let mut ch = self.0.lock();
        block!(self.try_send(rpc, req, &mut ch)).unwrap();
        unsafe { __mb_restore_flag(flag) };
    }
    fn send<REQ: Copy, RESP, RPC: MBRpc<REQ = REQ, RESP = RESP>>(
        &mut self,
        rpc: &RPC,
        req: REQ,
    ) -> RESP {
        let flag = unsafe { __mb_save_flag() };
        let mut ch = self.0.lock();
        block!(self.try_send(rpc, req, &mut ch)).unwrap();
        let resp = block!(self.try_recv(rpc, &mut ch)).unwrap();
        unsafe { __mb_restore_flag(flag) };
        resp
    }
    fn reset(&mut self) {
        self.0.lock().reset_req();
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
        if !self.0.is_ready() {
            return Err(nb::Error::WouldBlock);
        }
        if !self.0.req_can_put() {
            return Err(nb::Error::WouldBlock);
        }
        self.0.put_req(rpc, req);
        Ok(())
    }
    fn try_recv<RESP, RPC: MBRpc<RESP = RESP>>(
        &mut self,
        rpc: &RPC,
    ) -> nb::Result<RESP, MBNbSenderErr> {
        if !self.0.is_ready() {
            return Err(nb::Error::Other(MBNbSenderErr::NotReady));
        }
        if !self.0.resp_can_get() {
            return Err(nb::Error::WouldBlock);
        }
        let ret = self.0.get_resp(rpc);
        Ok(ret)
    }
}

impl<CH: 'static + MBChannelIf> MBNbSender for MBNbRefSender<CH> {
    fn send_nb<REQ: Copy, RPC: MBRpc<REQ = REQ>>(&mut self, rpc: &RPC, req: REQ) {
        let flag = unsafe { __mb_save_flag() };
        block!(self.try_send(rpc, req)).unwrap();
        unsafe { __mb_restore_flag(flag) };
    }
    fn send<REQ: Copy, RESP, RPC: MBRpc<REQ = REQ, RESP = RESP>>(
        &mut self,
        rpc: &RPC,
        req: REQ,
    ) -> RESP {
        let flag = unsafe { __mb_save_flag() };
        block!(self.try_send(rpc, req)).unwrap();
        let resp = block!(self.try_recv(rpc)).unwrap();
        unsafe { __mb_restore_flag(flag) };
        resp
    }
    fn reset(&mut self) {
        self.0.reset_req();
    }
}
