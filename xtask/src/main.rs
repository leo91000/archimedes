#![allow(unused_parens)]

use clap::Parser;
use clap::Subcommand;
use release::release_command;
use release::ReleaseType;

mod release;

#[derive(Subcommand)]
enum Command {
    #[command(about = "Print an \"hello world\" message used to test the cli")]
    Hello {
        #[arg(short, long, default_value_t = ("World".to_string()))]
        message: String,
    },
    #[command(about = "Release a new version of the package")]
    Release {
        #[arg(short, long, default_value = "auto")]
        release_type: ReleaseType,
    },
}

#[derive(Parser)]
#[command(author, version, about, long_about = "Archimedes helper tasks cli")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Hello { message } => println!("Hello {} !", message),
        Command::Release { release_type } => release_command(release_type),
    }
}
