use anyhow::Result;
use clap::Parser;
use dregs::cli::{Cli, run};

fn main() -> Result<()> {
    run(Cli::parse())
}
