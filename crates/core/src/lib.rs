//! PenCleaner engine: everything that touches the filesystem lives here, free of any
//! Tauri dependency so it can be unit-tested and driven from a CLI.

pub mod error;
pub mod fsutil;
pub mod permissions;
pub mod progress;
pub mod remove;
pub mod safety;
pub mod space_lens;
pub mod volumes;

pub use error::{Error, Result};
