//! Windows stand-in for `spotlight`: file facts Spotlight provides on macOS. Windows has
//! no "last opened" index (and often disables access times), so My Clutter falls back to
//! modification dates, as it does on the Mac when Spotlight knows nothing.

use std::path::Path;

#[derive(Debug, Default, Clone)]
pub struct Meta {
    pub last_used: Option<i64>,
    pub added: Option<i64>,
    /// Where a download came from, e.g. "github.com".
    pub source: Option<String>,
}

pub fn meta(_path: &Path) -> Meta {
    Meta::default()
}
