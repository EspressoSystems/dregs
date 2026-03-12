use anyhow::Result;
use clap::Parser;
use mutr::cli::{Cli, run};

fn main() -> Result<()> {
    run(Cli::parse())
}
