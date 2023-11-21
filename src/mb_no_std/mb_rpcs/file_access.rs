use crate::mb_no_std::mb_nb_channel::*;
use crate::mb_rpcs::*;
pub fn mb_fopen<SENDER: MBNbSender>(sender: &mut SENDER, path: MBPtrT, flags: u32) -> u32 {
    let fopen_rpc = MBFOpen::new();
    let args = MBFOpenArgs { path, flags };
    sender.send(&fopen_rpc, &args)
}

pub fn mb_fclose<SENDER: MBNbSender>(sender: &mut SENDER, fd: u32) {
    let fclose_rpc = MBFClose;
    sender.send_nb(&fclose_rpc, fd)
}

pub fn mb_fread<SENDER: MBNbSender>(
    sender: &mut SENDER,
    fd: u32,
    ptr: MBPtrT,
    len: usize,
) -> usize {
    let fread_rpc = MBFRead::new();
    let args = MBFReadArgs {
        fd,
        ptr,
        len: len as MBPtrT,
    };
    sender.send(&fread_rpc, &args)
}

pub fn mb_fwrite<SENDER: MBNbSender>(
    sender: &mut SENDER,
    fd: u32,
    ptr: MBPtrT,
    len: usize,
) -> usize {
    let fwrite_rpc = MBFWrite::new();
    let args = MBFWriteArgs {
        fd,
        ptr,
        len: len as MBPtrT,
    };
    sender.send(&fwrite_rpc, &args)
}

pub fn mb_fseek<SENDER: MBNbSender>(sender: &mut SENDER, fd: u32, pos: MBPtrT) -> MBPtrT {
    let fseek_rpc = MBFSeek::new();
    let args = MBFSeekArgs { fd, pos };
    sender.send(&fseek_rpc, &args)
}
