use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "omtsf", about = "Open Multi-Tier Supply-Chain Framework CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print the omtsf-core library version
    Version,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Version => {
            println!("{}", omtsf_core::version());
        }
    }
}
