use crate::mb_no_std::mb_nb_channel::*;
use crate::mb_rpcs::*;
pub fn mb_memcmp<SENDER: MBNbSender>(
    sender: &mut SENDER,
    s1: MBPtrT,
    s2: MBPtrT,
    len: MBPtrT,
) -> i32 {
    let memcmp_rpc = MBMemCmp::new();
    let args = MBMemCmpArgs { s1, s2, len };
    sender.send(&memcmp_rpc, &args)
}
