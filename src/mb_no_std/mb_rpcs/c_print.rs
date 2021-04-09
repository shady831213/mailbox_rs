use crate::mb_channel::*;
use crate::mb_no_std::mb_nb_channel::*;
use crate::mb_rpcs::*;
pub fn mb_cprint<SENDER: MBNbSender>(
    sender: &mut SENDER,
    fmt_str: *const u8,
    file: *const u8,
    pos: u32,
    args_len: MBPtrT,
    args: *const MBPtrT,
) {
    let cprint_rpc = MBCPrint::new();
    let c_str_args =
        cprint_rpc.to_cstr_args(file as MBPtrT, pos, fmt_str as MBPtrT, args_len, args);
    if c_str_args.rest_args_len() > 0 {
        sender.send(&cprint_rpc, &c_str_args);
    } else {
        sender.send_nb(&cprint_rpc, &c_str_args);
    }
}

impl<'a> MBCPrint<'a> {
    pub fn to_cstr_args(
        &self,
        file: MBPtrT,
        pos: u32,
        fmt_str: MBPtrT,
        args_len: MBPtrT,
        args: *const MBPtrT,
    ) -> MBCStringArgs {
        let mut c_str_args = MBCStringArgs::default();
        c_str_args.len = args_len as u32 + 3;
        c_str_args.file = file;
        c_str_args.pos = pos as MBPtrT;
        c_str_args.fmt_str = fmt_str;
        unsafe {
            for i in 0..args_len {
                c_str_args.args[i as usize] = *((args as MBPtrT
                    + i * core::mem::size_of::<MBPtrT>() as MBPtrT)
                    as *const MBPtrT)
            }
        }
        c_str_args
    }
}
