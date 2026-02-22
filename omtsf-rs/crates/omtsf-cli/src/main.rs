pub mod cli;
pub mod cmd;
pub mod error;
pub mod format;
pub mod io;

pub use cli::{
    Cli, Command, Direction, DisclosureScope, MergeStrategy, OutputFormat, PathOrStdin,
    TargetEncoding,
};

use clap::Parser;

fn main() {
    #[cfg(unix)]
    install_sigpipe_default();

    let cli = Cli::parse();

    let result = dispatch(&cli);

    if let Err(e) = result {
        eprintln!("{}", e.message());
        std::process::exit(e.exit_code());
    }
}

/// Dispatches the parsed CLI arguments to the appropriate command handler.
///
/// Returns `Ok(())` on success or a [`error::CliError`] on failure. The
/// caller is responsible for printing the error message and exiting with the
/// appropriate exit code.
fn dispatch(cli: &Cli) -> Result<(), error::CliError> {
    match &cli.command {
        Command::Validate { file, level } => {
            let (omts_file, _encoding) = io::read_and_parse(file, cli.max_file_size, cli.verbose)?;
            cmd::validate::run(
                &omts_file,
                *level,
                &cli.format,
                cli.quiet,
                cli.verbose,
                cli.no_color,
            )
        }

        Command::Inspect { file } => {
            let (omts_file, _encoding) = io::read_and_parse(file, cli.max_file_size, cli.verbose)?;
            cmd::inspect::run(&omts_file, &cli.format)
        }

        Command::Convert {
            file,
            to,
            pretty,
            compact,
            compress,
        } => {
            let (omts_file, _encoding) = io::read_and_parse(file, cli.max_file_size, cli.verbose)?;
            cmd::convert::run(&omts_file, to, *pretty, *compact, *compress)
        }

        Command::Init { example } => cmd::init::run(*example),

        Command::Reach {
            file,
            node_id,
            depth,
            direction,
        } => {
            let (omts_file, _encoding) = io::read_and_parse(file, cli.max_file_size, cli.verbose)?;
            cmd::reach::run(&omts_file, node_id, *depth, direction, &cli.format)
        }

        Command::Path {
            file,
            from,
            to,
            max_paths,
            max_depth,
        } => {
            let (omts_file, _encoding) = io::read_and_parse(file, cli.max_file_size, cli.verbose)?;
            cmd::path::run(&omts_file, from, to, *max_paths, *max_depth, &cli.format)
        }

        Command::Subgraph {
            file,
            node_ids,
            node_type,
            edge_type,
            label,
            identifier,
            jurisdiction,
            name,
            expand,
            to,
            compress,
        } => {
            let (omts_file, _encoding) = io::read_and_parse(file, cli.max_file_size, cli.verbose)?;
            cmd::subgraph::run(
                &omts_file,
                node_ids,
                node_type,
                edge_type,
                label,
                identifier,
                jurisdiction,
                name,
                *expand,
                to,
                *compress,
            )
        }

        Command::Merge {
            files,
            strategy,
            to,
            compress,
        } => cmd::merge::run(
            files,
            strategy,
            to,
            *compress,
            cli.max_file_size,
            cli.verbose,
        ),

        Command::Redact {
            file,
            scope,
            to,
            compress,
        } => {
            let (omts_file, _encoding) = io::read_and_parse(file, cli.max_file_size, cli.verbose)?;
            cmd::redact::run(&omts_file, scope, to, *compress)
        }

        Command::Diff {
            a,
            b,
            ids_only,
            summary_only,
            node_type,
            edge_type,
            ignore_field,
        } => {
            let (file_a, _enc_a) = io::read_and_parse(a, cli.max_file_size, cli.verbose)?;
            let (file_b, _enc_b) = io::read_and_parse(b, cli.max_file_size, cli.verbose)?;
            cmd::diff::run(
                &file_a,
                &file_b,
                *ids_only,
                *summary_only,
                node_type,
                edge_type,
                ignore_field,
                &cli.format,
                cli.verbose,
                cli.no_color,
            )
        }

        Command::Query {
            file,
            node_type,
            edge_type,
            label,
            identifier,
            jurisdiction,
            name,
            count,
        } => {
            let (omts_file, _encoding) = io::read_and_parse(file, cli.max_file_size, cli.verbose)?;
            cmd::query::run(
                &omts_file,
                node_type,
                edge_type,
                label,
                identifier,
                jurisdiction,
                name,
                *count,
                &cli.format,
            )
        }
    }
}

/// Resets `SIGPIPE` to its default disposition (`SIG_DFL`).
///
/// Rust's runtime ignores `SIGPIPE` by default, which causes programs that
/// write to a closed pipe (e.g. `omtsf validate file.omts | head`) to receive
/// an `Err(BrokenPipe)` from a write call rather than being terminated silently.
/// By restoring the default disposition, the kernel will terminate the process
/// with exit code 0 (consistent with standard Unix behavior) when a write to a
/// closed pipe occurs.
///
/// This function uses `libc::signal` which requires the `libc` crate. It is
/// only compiled on Unix targets via `#[cfg(unix)]` at the call site.
#[cfg(unix)]
fn install_sigpipe_default() {
    // SAFETY: signal() is safe to call during single-threaded program
    // initialization before any other threads are spawned. SIG_DFL is a valid
    // handler for SIGPIPE. The return value (previous handler) is discarded.
    //
    // The workspace denies `unsafe_code` globally, but this is the minimal
    // unavoidable use of libc required for SIGPIPE handling on Unix. There is
    // no safe Rust equivalent in the standard library.
    //
    // We use an inline allow rather than a workspace-level exception so the
    // scope of the unsafe block is as narrow as possible.
    #[allow(unsafe_code)]
    {
        unsafe {
            libc::signal(libc::SIGPIPE, libc::SIG_DFL);
        }
    }
}
