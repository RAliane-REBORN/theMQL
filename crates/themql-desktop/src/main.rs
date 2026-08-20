//! # themql-desktop
//!
//! Desktop binary entry point. Primary development, analysis, training,
//! telemetry, and operational environment. Per `specs/desktop.toml`,
//! this binary composes all desktop-capable crates and exposes a CLI,
//! TUI, and Dioxus UI.
//!
//! This is a stub implementation: it parses the CLI and dispatches to
//! `println!` placeholders. The actual server, training, analysis,
//! telemetry, and TUI implementations are future tasks. Heavy
//! dependencies (dioxus, ratatui, tokio, polars, tch) are deferred.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]

use std::process::ExitCode;

use clap::{Parser, Subcommand};

/// Desktop CLI. Mirrors `specs/desktop.toml [api.Cli]`.
#[derive(Debug, Clone, Parser)]
#[command(name = "themql-desktop", version, about = "theMQL desktop binary")]
struct Cli {
    /// The subcommand to run.
    #[command(subcommand)]
    command: Command,
}

/// Desktop subcommands. Mirrors `specs/desktop.toml [api.Command]`.
#[derive(Debug, Clone, Subcommand)]
enum Command {
    /// Start the desktop server (GraphQL + SSE + MQTT bridge + UI).
    Serve(ServeArgs),
    /// Run a one-shot analysis pipeline.
    Analyze(AnalyzeArgs),
    /// Train a model and emit an artifact.
    Train(TrainArgs),
    /// Validate a model artifact without activating it.
    Validate(ValidateArgs),
    /// Inspect telemetry stored in helix-db.
    Telemetry(TelemetryArgs),
    /// Launch the TUI dashboard.
    Tui,
}

/// Arguments for the `serve` subcommand. Mirrors
/// `specs/desktop.toml [api.ServeArgs]`.
#[derive(Debug, Clone, Parser)]
struct ServeArgs {
    /// Bind address for the desktop server.
    #[arg(long, default_value = "0.0.0.0")]
    bind: String,
    /// Port for the desktop server.
    #[arg(long, default_value_t = 8080)]
    port: u16,
    /// Whether to enable the MQTT bridge.
    #[arg(long, default_value_t = false)]
    enable_mqtt: bool,
}

/// Arguments for the `analyze` subcommand.
#[derive(Debug, Clone, Parser)]
struct AnalyzeArgs {
    /// Path to the analysis input file.
    #[arg(long)]
    input: String,
}

/// Arguments for the `train` subcommand. Mirrors
/// `specs/desktop.toml [api.TrainArgs]`.
#[derive(Debug, Clone, Parser)]
struct TrainArgs {
    /// Path to the training dataset.
    #[arg(long)]
    dataset: String,
    /// Trainer kind (`dense`, `pinn`, `gradient_boosting`, `fine_tuning`).
    #[arg(long, default_value = "dense")]
    kind: String,
    /// Number of training epochs.
    #[arg(long, default_value_t = 100)]
    epochs: u32,
    /// Path to write the emitted artifact.
    #[arg(long)]
    output: String,
}

/// Arguments for the `validate` subcommand.
#[derive(Debug, Clone, Parser)]
struct ValidateArgs {
    /// Path to the model artifact to validate.
    #[arg(long)]
    artifact: String,
}

/// Arguments for the `telemetry` subcommand.
#[derive(Debug, Clone, Parser)]
struct TelemetryArgs {
    /// Subject pattern to inspect.
    #[arg(long, default_value = "vehicle.#")]
    subject: String,
}

/// Dispatch a parsed [`Command`] to its placeholder implementation.
///
/// This stub prints the chosen command. Real implementations will live
/// in future tasks and may return errors.
#[allow(clippy::unnecessary_wraps)]
fn dispatch(command: &Command) -> Result<(), themql_core::Error> {
    match command {
        Command::Serve(args) => {
            println!(
                "themql-desktop: serve on {}:{} (mqtt={})",
                args.bind, args.port, args.enable_mqtt
            );
        }
        Command::Analyze(args) => {
            println!("themql-desktop: analyze input={}", args.input);
        }
        Command::Train(args) => {
            println!(
                "themql-desktop: train dataset={} kind={} epochs={} output={}",
                args.dataset, args.kind, args.epochs, args.output
            );
        }
        Command::Validate(args) => {
            println!("themql-desktop: validate artifact={}", args.artifact);
        }
        Command::Telemetry(args) => {
            println!("themql-desktop: telemetry subject={}", args.subject);
        }
        Command::Tui => {
            println!("themql-desktop: tui (not implemented)");
        }
    }
    Ok(())
}

/// Desktop entry point. Parses the CLI and dispatches to the chosen
/// subcommand. Per `specs/desktop.toml [entry]`, the canonical form
/// uses a tokio `DesktopRuntime`; this stub uses synchronous dispatch
/// because tokio is deferred.
fn main() -> ExitCode {
    let cli = Cli::parse();
    match dispatch(&cli.command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_parses_serve_subcommand() {
        let cli = Cli::parse_from(["themql-desktop", "serve", "--port", "9090"]);
        match cli.command {
            Command::Serve(args) => assert_eq!(args.port, 9090),
            _ => panic!("must parse Serve subcommand"),
        }
    }

    #[test]
    fn cli_parses_train_subcommand() {
        let cli = Cli::parse_from([
            "themql-desktop",
            "train",
            "--dataset",
            "data.parquet",
            "--kind",
            "pinn",
            "--epochs",
            "50",
            "--output",
            "model.tar",
        ]);
        match cli.command {
            Command::Train(args) => {
                assert_eq!(args.dataset, "data.parquet");
                assert_eq!(args.kind, "pinn");
                assert_eq!(args.epochs, 50);
                assert_eq!(args.output, "model.tar");
            }
            _ => panic!("must parse Train subcommand"),
        }
    }

    #[test]
    fn cli_parses_tui_subcommand() {
        let cli = Cli::parse_from(["themql-desktop", "tui"]);
        assert!(matches!(cli.command, Command::Tui));
    }

    #[test]
    fn dispatch_serve_prints_port() {
        let args = ServeArgs {
            bind: "127.0.0.1".to_owned(),
            port: 1234,
            enable_mqtt: true,
        };
        dispatch(&Command::Serve(args)).unwrap();
    }

    #[test]
    fn dispatch_tui_succeeds() {
        dispatch(&Command::Tui).unwrap();
    }
}
