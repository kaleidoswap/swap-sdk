//! # rln-client
//!
//! Rust types (and, later, a thin HTTP client) for the
//! [RGB Lightning Node](https://github.com/kaleidoswap/rgb-lightning-node) API.
//!
//! This mirrors the kaleido-sdk approach: the [`types`] module is **generated**
//! from `specs/rgb-lightning-node.yaml` (OpenAPI 3.1) by
//! [typify](https://github.com/oxidecomputer/typify) and must not be edited by
//! hand — regenerate it with `make generate-rln-types`. A hand-written client
//! (in the style of `kaleidoswap_sdk::boltz::BoltzApiClientV2`) will be added in a
//! `client` module on top of these types.

/// Generated RLN API types. Do not edit — see `make generate-rln-types`.
pub mod types;

/// Hand-written async HTTP client over [`types`]. Enabled by the `client` feature.
#[cfg(feature = "client")]
pub mod client;

#[cfg(feature = "client")]
pub use client::{RlnClient, RlnError};
