use crate::mb_no_std::mb_nb_channel::*;
use crate::mb_channel::*;
use crate::mb_rpcs::*;
pub fn mb_exit<CH: MBChannelIf>(sender: &MBNbRefSender<CH>, code: u32) {
    sender.send_nb(&MBExit, code);
}