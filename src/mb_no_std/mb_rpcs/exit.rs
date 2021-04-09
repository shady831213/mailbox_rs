use crate::mb_no_std::mb_nb_channel::*;
use crate::mb_rpcs::*;
pub fn mb_exit<SENDER: MBNbSender>(sender: &mut SENDER, code: u32) {
    sender.send_nb(&MBExit, code);
}
