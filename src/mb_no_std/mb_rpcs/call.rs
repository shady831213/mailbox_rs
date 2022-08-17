use crate::mb_channel::*;
use crate::mb_no_std::mb_nb_channel::*;
use crate::mb_rpcs::*;
pub fn mb_call<SENDER: MBNbSender>(
    sender: &mut SENDER,
    method: *const u8,
    args_len: MBPtrT,
    args: *const MBPtrT,
) -> MBPtrT {
    let call_rpc = MBCall::new();
    let mut call_args = MBCallArgs {
        len: args_len as u32,
        method: method as MBPtrT,
        args: [0; MB_MAX_ARGS - 1],
    };
    unsafe {
        for i in 0..args_len {
            call_args.args[i as usize] =
                *((args as MBPtrT + i * core::mem::size_of::<MBPtrT>() as MBPtrT) as *const MBPtrT)
        }
    }
    sender.send(&call_rpc, &call_args)
}
