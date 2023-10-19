mod mb_nb_channel;
mod mb_rpcs;
use crate::mb_rpcs::MBPtrT;
pub use mb_nb_channel::*;
pub use mb_rpcs::*;

#[linkage = "weak"]
#[no_mangle]
extern "C" fn __mb_rfence(_start: MBPtrT, _size: usize) {}

#[linkage = "weak"]
#[no_mangle]
extern "C" fn __mb_wfence(_start: MBPtrT, _size: usize) {}
