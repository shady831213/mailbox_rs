use crate::mb_channel::*;
use crate::mb_no_std::mb_nb_channel::*;
use crate::mb_rpcs::*;
pub fn mb_print<CH: MBChannelIf>(sender: &MBNbRefSender<CH>, msg: &str) {
    let print_rpc = MBPrint::new();
    let str_args = MBStringArgs {
        len: msg.len() as u32,
        ptr: msg.as_ptr() as MBPtrT,
    };
    sender.send(&print_rpc, &str_args);
}
