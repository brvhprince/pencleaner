//! PenCleaner engine: everything that touches the filesystem lives here, free of any
//! Tauri dependency so it can be unit-tested and driven from a CLI.

pub mod applications;
pub mod apps;
pub mod cleanup;
pub mod clutter;
pub mod error;
pub mod fsutil;
pub mod imaging;
pub mod permissions;
pub mod privileged;
pub mod progress;
pub mod remove;
pub mod safety;
pub mod space_lens;
pub mod spotlight;
pub mod volumes;

pub use error::{Error, Result};
