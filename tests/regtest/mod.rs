#![cfg(feature = "regtest")]

use std::time::Duration;

mod chain_swaps;
mod common;
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
mod kaleidoswap;
mod reverse;
mod submarine;

const WAIT_TIME: Duration = Duration::from_millis(5_000);

const BOLTZ_TIMEOUT: Duration = Duration::from_secs(30);
