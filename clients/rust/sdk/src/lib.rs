#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod location;
#[cfg(all(feature = "native-ota", unix))]
pub mod native_ota;
#[cfg(any(feature = "std", feature = "alloc"))]
pub mod ota;
pub mod sensor;
#[cfg(feature = "std")]
pub mod time;
