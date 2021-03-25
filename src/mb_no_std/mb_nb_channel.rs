use crate::mb_channel::*;
extern crate alloc;
extern crate nb;
extern crate spin;
use nb::block;
use spin::Mutex;
extern "C" {
    fn __mb_save_flag() -> MBPtrT;
    fn __mb_restore_flag(flag: MBPtrT);
}
pub struct MBNbRefSender<CH: 'static + MBChannelIf>(Mutex<&'static mut CH>);

impl<CH: 'static + MBChannelIf> MBNbRefSender<CH> {
    pub fn new(ch: &'static mut CH) -> MBNbRefSender<CH> {
        MBNbRefSender(Mutex::new(ch))
    }
    fn try_send<REQ: Copy, RPC: MBRpc<REQ = REQ>>(
        &self,
        rpc: &RPC,
        req: REQ,
        ch: &mut CH,
    ) -> nb::Result<(), ()> {
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
    ) -> nb::Result<RESP, ()> {
        if !ch.resp_can_get() {
            return Err(nb::Error::WouldBlock);
        }
        let ret = ch.get_resp(rpc);
        Ok(ret)
    }

    pub fn send_nb<REQ: Copy, RPC: MBRpc<REQ = REQ>>(&self, rpc: &RPC, req: REQ) {
        let flag = unsafe { __mb_save_flag() };
        let mut ch = self.0.lock();
        block!(self.try_send(rpc, req, &mut ch)).unwrap();
        unsafe { __mb_restore_flag(flag) };
    }
    pub fn send<REQ: Copy, RESP, RPC: MBRpc<REQ = REQ, RESP = RESP>>(
        &self,
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
}
