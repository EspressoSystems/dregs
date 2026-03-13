use anyhow::Result;
use clap::Parser;
use dregs::Cli;

fn main() -> Result<()> {
    dregs::run(Cli::parse())
}
