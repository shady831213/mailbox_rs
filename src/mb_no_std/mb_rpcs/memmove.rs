use crate::mb_no_std::mb_nb_channel::*;
use crate::mb_rpcs::*;
pub fn mb_memmove<SENDER: MBNbSender>(
    sender: &mut SENDER,
    dest: MBPtrT,
    src: MBPtrT,
    len: usize,
) -> MBPtrT {
    let memmove_rpc = MBMemMove::new();
    let args = MBMemMoveArgs {
        dest,
        src,
        len: len as MBPtrT,
    };
    sender.send(&memmove_rpc, &args);
    dest
}
