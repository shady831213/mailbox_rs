use crate::mb_channel::*;
use crate::mb_no_std::mb_nb_channel::*;
use crate::mb_rpcs::*;
pub fn mb_svcall<SENDER: MBNbSender>(
    sender: &mut SENDER,
    method: *const u8,
    args_len: MBPtrT,
    args: *const MBPtrT,
) -> MBPtrT {
    let svcall_rpc = MBSvCall::new();
    let mut svcall_args = MBSvCallArgs {
        len: args_len as u32,
        method: method as MBPtrT,
        args: [0; MB_MAX_ARGS - 1],
    };
    unsafe {
        for i in 0..args_len {
            svcall_args.args[i as usize] =
                *((args as MBPtrT + i * core::mem::size_of::<MBPtrT>() as MBPtrT) as *const MBPtrT)
        }
    }
    sender.send(&svcall_rpc, &svcall_args)
}
