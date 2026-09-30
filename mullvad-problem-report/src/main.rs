use clap::Parser;
use mullvad_problem_report::{ClockOffset, Error, ProblemReportCollector, WriteSource};
use std::{io, path::PathBuf, process, time::Duration};
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
                clock_offset: daemon_clock_offset(),
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

/// How long the report waits for the daemon's clock reading. A daemon that is
/// down or wedged is often why a report is filed, so it must not hold the
/// report back.
const DAEMON_TIMEOUT: Duration = Duration::from_secs(3);

/// This machine's clock against the Warren servers', as the running daemon
/// measured it: the daemon holds the one clock every signed request is
/// stamped with.
fn daemon_clock_offset() -> ClockOffset {
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return ClockOffset::Unknown;
    };
    let diagnostics = runtime.block_on(async {
        tokio::time::timeout(DAEMON_TIMEOUT, async {
            let mut client = mullvad_management_interface::MullvadProxyClient::new()
                .await
                .ok()?;
            client.get_warren_diagnostics().await.ok()
        })
        .await
        .ok()
        .flatten()
    });
    match diagnostics {
        Some(diagnostics) => diagnostics
            .server_clock_offset_secs
            .map_or(ClockOffset::NotMeasured, ClockOffset::Measured),
        None => ClockOffset::Unknown,
    }
}
