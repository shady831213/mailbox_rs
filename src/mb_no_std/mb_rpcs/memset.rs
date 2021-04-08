use crate::mb_channel::*;
use crate::mb_no_std::mb_nb_channel::*;
use crate::mb_rpcs::*;
pub fn mb_memset<CH: MBChannelIf>(
    sender: &MBNbRefSender<CH>,
    dest: MBPtrT,
    data: MBPtrT,
    len: MBPtrT,
) -> MBPtrT {
    let memset_rpc = MBMemSet::new();
    let args = MBMemSetArgs { dest, data, len };
    sender.send(&memset_rpc, &args);
    dest
}
