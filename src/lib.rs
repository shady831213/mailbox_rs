#![cfg_attr(feature = "no_std", feature(const_mut_refs))]
#![feature(linkage)]
#![cfg_attr(feature = "no_std", no_std)]
#![cfg_attr(feature = "std", feature(int_roundings))]
pub mod mb_channel;
pub mod mb_rpcs;
#[cfg(feature = "std")]
pub mod mb_std;

#[cfg(feature = "no_std")]
pub mod mb_no_std;
