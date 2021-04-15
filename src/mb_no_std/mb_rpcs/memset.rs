use crate::mb_no_std::mb_nb_channel::*;
use crate::mb_rpcs::*;
pub fn mb_memset<SENDER: MBNbSender>(
    sender: &mut SENDER,
    dest: MBPtrT,
    data: MBPtrT,
    len: MBPtrT,
) -> MBPtrT {
    let memset_rpc = MBMemSet::new();
    let args = MBMemSetArgs { dest, data, len };
    sender.send(&memset_rpc, &args);
    dest
}
