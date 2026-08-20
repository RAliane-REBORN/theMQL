//! # themql-desktop
//!
//! Desktop binary entry point. Primary development, analysis, training,
//! telemetry, and operational environment. Per `specs/desktop.toml`,
//! this binary composes all desktop-capable crates and exposes a CLI,
//! TUI, and Dioxus UI.
//!
//! This implementation wires a tokio `#[tokio::main]` entry point, clap
//! CLI dispatch with placeholder prints, and a ratatui/crossterm TUI
//! dashboard with three labelled panes (telemetry stream, state
//! estimate, controller state). The actual server, training, analysis,
//! and telemetry work is a future task; the dispatch here prints the
//! chosen command so the wiring is observable end-to-end.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]

use std::io;
use std::time::Duration;

use clap::{Parser, Subcommand};
use ratatui::crossterm::event::{self, Event, KeyCode};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Style, Stylize};
use ratatui::widgets::{Block, Borders, Paragraph};

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
            println!("themql-desktop: launching TUI");
            if let Err(e) = tui_main() {
                eprintln!("themql-desktop: tui error: {e}");
            }
        }
    }
    Ok(())
}

/// Ratatui/crossterm TUI dashboard. Three labelled panes per
/// `specs/desktop.toml [api.TuiLayout]`: telemetry stream (top), state
/// estimate + covariance (middle), controller state + actuator commands
/// (bottom). Loops reading key events; quits on `q` or `Esc`.
///
/// # Errors
/// Returns the underlying `io::Error` if terminal init, draw, or event
/// read fails.
fn tui_main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    loop {
        terminal.draw(draw_dashboard)?;
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
                    break;
                }
            }
        }
    }
    ratatui::restore();
    Ok(())
}

/// Draw the three-pane dashboard layout. Each pane is a labelled,
/// bordered block; contents are placeholders pending real telemetry
/// wiring.
fn draw_dashboard(frame: &mut ratatui::Frame<'_>) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Min(1),
        ])
        .split(frame.area());

    let telemetry = Block::default()
        .borders(Borders::ALL)
        .title("Telemetry stream")
        .bold();
    frame.render_widget(telemetry, chunks[0]);

    let state_estimate = Block::default()
        .borders(Borders::ALL)
        .title("State estimate + covariance")
        .bold();
    frame.render_widget(state_estimate, chunks[1]);

    let controller_state = Block::default()
        .borders(Borders::ALL)
        .title("Controller state + actuator commands")
        .bold();
    frame.render_widget(controller_state, chunks[2]);

    let hint = Paragraph::new("press q to quit")
        .right_aligned()
        .style(Style::new().dim().underlined());
    frame.render_widget(hint, chunks[0]);
}

/// Desktop entry point. Parses the CLI and dispatches to the chosen
/// subcommand. Per `specs/desktop.toml [entry]`, the canonical form
/// uses a tokio `DesktopRuntime`; here we run under a tokio
/// multi-thread runtime so async commands compose naturally.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    dispatch(&cli.command)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

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
    fn cli_parses_analyze_subcommand() {
        let cli = Cli::parse_from(["themql-desktop", "analyze", "--input", "x.csv"]);
        match cli.command {
            Command::Analyze(args) => assert_eq!(args.input, "x.csv"),
            _ => panic!("must parse Analyze subcommand"),
        }
    }

    #[test]
    fn cli_parses_validate_subcommand() {
        let cli = Cli::parse_from(["themql-desktop", "validate", "--artifact", "m.tar"]);
        match cli.command {
            Command::Validate(args) => assert_eq!(args.artifact, "m.tar"),
            _ => panic!("must parse Validate subcommand"),
        }
    }

    #[test]
    fn cli_parses_telemetry_subcommand() {
        let cli = Cli::parse_from(["themql-desktop", "telemetry"]);
        match cli.command {
            Command::Telemetry(args) => assert_eq!(args.subject, "vehicle.#"),
            _ => panic!("must parse Telemetry subcommand"),
        }
    }

    #[test]
    fn cli_help_contains_expected_strings() {
        let help = Cli::command().render_help().to_string();
        assert!(help.contains("themql-desktop"));
        assert!(help.contains("serve"));
        assert!(help.contains("analyze"));
        assert!(help.contains("train"));
        assert!(help.contains("validate"));
        assert!(help.contains("telemetry"));
        assert!(help.contains("tui"));
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
    fn dispatch_analyze_succeeds() {
        dispatch(&Command::Analyze(AnalyzeArgs {
            input: "in.csv".to_owned(),
        }))
        .unwrap();
    }

    #[test]
    fn dispatch_train_succeeds() {
        dispatch(&Command::Train(TrainArgs {
            dataset: "d.parquet".to_owned(),
            kind: "dense".to_owned(),
            epochs: 1,
            output: "o.tar".to_owned(),
        }))
        .unwrap();
    }

    #[test]
    fn dispatch_validate_succeeds() {
        dispatch(&Command::Validate(ValidateArgs {
            artifact: "a.tar".to_owned(),
        }))
        .unwrap();
    }

    #[test]
    fn dispatch_telemetry_succeeds() {
        dispatch(&Command::Telemetry(TelemetryArgs {
            subject: "vehicle.state_estimate".to_owned(),
        }))
        .unwrap();
    }

    #[test]
    fn draw_dashboard_runs_with_test_backend() {
        let backend = ratatui::backend::TestBackend::new(40, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal.draw(draw_dashboard).unwrap();
    }
}
