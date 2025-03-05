use std::{env, str::FromStr, sync::Once};

use bitcoin::amount;
use elements::{encode::Decodable, hex::ToHex};
use lightning_invoice::{Bolt11Invoice, RouteHintHop};

use crate::error::Error;

pub mod ec;
pub mod fees;
#[cfg(feature = "lnurl")]
pub mod lnurl;
pub mod secrets;

static INIT: Once = Once::new();

/// Setup function that will only run once, even if called multiple times.
pub fn setup_logger() {
    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    INIT.call_once(|| {
        env_logger::Builder::from_env(
            env_logger::Env::default()
                .default_filter_or("debug")
                .default_write_style_or("always"),
        )
        .filter_module("serial_test", log::LevelFilter::Error)
        // .is_test(true)
        .init();
    });
}
