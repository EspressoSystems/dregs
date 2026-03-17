use anyhow::Result;
use clap::Parser;
use dregs::Cli;
use env_logger::Env;

fn main() -> Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("gambit=warn"))
        .format_target(false)
        .format_timestamp(None)
        .init();
    dregs::run(Cli::parse())
}
