use crate::mb_channel::*;
use crate::mb_no_std::mb_nb_channel::*;
use crate::mb_rpcs::*;
use core::marker::PhantomData;
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
    sender.send_nb(&cprint_rpc, &c_str_args);
}

pub struct MBCPrint<'a> {
    _marker: PhantomData<&'a u8>,
}
impl<'a> MBCPrint<'a> {
    pub fn new() -> MBCPrint<'a> {
        MBCPrint {
            _marker: PhantomData,
        }
    }
}

impl<'a> MBRpc for MBCPrint<'a> {
    type REQ = &'a MBCStringArgs;
    type RESP = ();
    fn put_req(&self, req: Self::REQ, entry: &mut MBReqEntry) {
        entry.set_words(req.len);
        entry.set_action(MBAction::CPRINT);
        entry.set_args(0, req.fmt_str);
        entry.set_args(1, req.file);
        entry.set_args(2, req.pos);
        for (i, d) in req.args[..req.args_len()].iter().enumerate() {
            entry.set_args(3 + i, *d);
        }
        // entry.action = MBAction::CPRINT;
        // entry.words = req.len;
        // entry.args[0] = req.fmt_str;
        // entry.args[1] = req.file;
        // entry.args[2] = req.pos;
        // for (i, d) in req.args[..req.arg_len()].iter().enumerate() {
        //     entry.args[3 + i] = *d
        // }
    }
    fn get_resp(&self, _: &MBRespEntry) -> Self::RESP {}
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
