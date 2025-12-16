use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "mutr")]
#[command(about = "Mutation testing for Solidity projects", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Run {
        #[arg(help = "Solidity files to mutate (default: src/**/*.sol)")]
        files: Vec<PathBuf>,

        #[arg(short, long, default_value = ".", help = "Project root")]
        project: PathBuf,

        #[arg(short, long, help = "Output report path (JSON)")]
        output: Option<PathBuf>,

        #[arg(
            long,
            help = "Fail if mutation score below threshold (0.0-1.0)",
            value_name = "SCORE"
        )]
        fail_under: Option<f64>,

        #[arg(long, help = "Path to solc binary")]
        solc: Option<PathBuf>,

        #[arg(
            long,
            help = "Comma-separated mutation operators (default: all)",
            value_delimiter = ','
        )]
        mutations: Vec<String>,

        #[arg(
            long,
            default_value = "60",
            help = "Test timeout per mutant in seconds"
        )]
        timeout: u64,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run {
            files,
            project,
            output,
            fail_under,
            solc,
            mutations,
            timeout,
        } => {
            println!("Running mutation testing...");
            println!("Project: {}", project.display());
            println!("Files: {:?}", files);
            println!("Output: {:?}", output);
            println!("Fail under: {:?}", fail_under);
            println!("Solc: {:?}", solc);
            println!("Mutations: {:?}", mutations);
            println!("Timeout: {}s", timeout);
            todo!("Implement run command")
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert!(true);
    }
}
