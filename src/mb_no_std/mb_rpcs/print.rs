use crate::mb_channel::*;
use crate::mb_no_std::mb_nb_channel::*;
use crate::mb_rpcs::*;
use core::marker::PhantomData;
pub fn mb_print<SENDER: MBNbSender>(sender: &mut SENDER, msg: &str) {
    let print_rpc = MBPrint::new();
    let str_args = MBStringArgs {
        len: msg.len() as u32,
        ptr: msg.as_ptr() as MBPtrT,
    };
    sender.send(&print_rpc, &str_args);
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
