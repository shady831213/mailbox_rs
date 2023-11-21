use crate::mb_channel::*;
use crate::mb_no_std::mb_nb_channel::*;
use crate::mb_rpcs::*;
pub fn mb_call<SENDER: MBNbSender>(
    sender: &mut SENDER,
    method: *const u8,
    args_len: usize,
    args: *const usize,
) -> MBPtrT {
    let call_rpc = MBCall::new();
    let mut call_args = MBCallArgs {
        len: args_len as u32,
        method: method as MBPtrT,
        args: [0; MB_MAX_ARGS - 1],
    };
    unsafe {
        for i in 0..args_len {
            call_args.args[i] =
                *((args as usize + i * core::mem::size_of::<usize>()) as *const usize) as MBPtrT
        }
    }
    sender.send(&call_rpc, &call_args)
}
