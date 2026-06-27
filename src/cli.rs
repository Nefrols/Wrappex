use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "wrappex",
    version,
    about = "Launch Codex with local model profiles"
)]
pub struct Cli {
    #[arg(long = "codex-bin", global = true)]
    pub codex_bin: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Run(RunArgs),
    Profile(ProfileArgs),
}

#[derive(Debug, Args)]
pub struct ProfileArgs {
    #[command(subcommand)]
    pub command: ProfileCommand,
}

#[derive(Debug, Args)]
pub struct RunArgs {
    pub profile: String,

    #[arg(last = true)]
    pub codex_args: Vec<String>,
}

#[derive(Debug, Subcommand)]
pub enum ProfileCommand {
    Create,
    List,
    Remove { profile: String },
}
