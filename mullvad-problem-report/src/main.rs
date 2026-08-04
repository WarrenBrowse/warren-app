use clap::Parser;
use mullvad_problem_report::{Error, ProblemReportCollector, WriteSource};
use std::{io, path::PathBuf, process};
use talpid_types::ErrorExt;

fn main() {
    process::exit(match run() {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("{}", error.display_chain());
            1
        }
    })
}

#[derive(Debug, Parser)]
#[command(author, version = mullvad_version::VERSION, about, long_about = None)]
#[command(
    arg_required_else_help = true,
    disable_help_subcommand = true,
    disable_version_flag = true
)]
enum Cli {
    /// Collect problem report to a single file
    Collect {
        /// The destination path for saving the collected report
        #[arg(required = true, long, short = 'o')]
        output: String,
        /// Paths to additional log files to be included
        extra_logs: Vec<PathBuf>,
        /// List of strings to remove from the report
        #[arg(long)]
        redact: Vec<String>,
    },
}

fn run() -> Result<(), Error> {
    tracing_subscriber::fmt::init();

    match Cli::parse() {
        Cli::Collect {
            output,
            extra_logs,
            redact,
        } => {
            let collector = ProblemReportCollector {
                extra_logs,
                redact_custom_strings: redact,
            };
            if output != "-" {
                collector.write_to_path(&output)?;

                println!("Problem report written to {output}");
                println!();
                println!("Attach the report to a support thread on the community forum.");
            } else {
                // Write logs to stdout
                collector.write(WriteSource::from((io::stdout(), "stdout".to_owned())))?;
            }
        }
    }

    Ok(())
}
