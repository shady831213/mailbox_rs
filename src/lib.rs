#![cfg_attr(feature = "no_std", feature(const_fn, const_mut_refs))]
#![cfg_attr(feature = "no_std", no_std)]
pub mod mb_channel;
pub mod mb_rpcs;
#[cfg(feature = "std")]
pub mod mb_std;

#[cfg(feature = "no_std")]
pub mod mb_no_std;
