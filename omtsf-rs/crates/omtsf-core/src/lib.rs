#![deny(clippy::print_stdout, clippy::print_stderr)]

pub mod newtypes;

pub use newtypes::{CalendarDate, CountryCode, EdgeId, FileSalt, NewtypeError, NodeId, SemVer};

/// Returns the current version of the omtsf-core library.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn version_is_semver() {
        let v = version();
        let parts: Vec<&str> = v.split('.').collect();
        assert_eq!(parts.len(), 3, "version should have 3 parts: {v}");
        for part in parts {
            part.parse::<u32>().expect("each part should be a number");
        }
    }
}
