/// Command module for the `omtsf` CLI.
///
/// Each submodule implements one subcommand. The `run` function in each
/// module takes the parsed arguments and returns `Ok(())` on success or
/// a [`crate::error::CliError`] on failure.
pub mod convert;
pub mod diff;
pub mod import;
pub mod init;
pub mod inspect;
pub mod merge;
pub mod path;
pub mod query;
pub mod reach;
pub mod redact;
pub mod selectors;
pub mod subgraph;
pub mod validate;
