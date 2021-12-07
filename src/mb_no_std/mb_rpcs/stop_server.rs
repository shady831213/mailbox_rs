use crate::mb_no_std::mb_nb_channel::*;
use crate::mb_rpcs::*;
pub fn mb_stop_server<SENDER: MBNbSender>(sender: &mut SENDER) {
    sender.send_nb(&MBStopServer, ());
}
