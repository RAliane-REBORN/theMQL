//! # themql-desktop
//!
//! Desktop binary entry point. Primary development, analysis, training,
//! telemetry, and operational environment. Per `specs/desktop.toml`,
//! this binary composes all desktop-capable crates and exposes a CLI,
//! TUI, and Dioxus UI.
//!
//! This implementation wires a tokio `#[tokio::main]` entry point, clap
//! CLI dispatch, and a ratatui/crossterm TUI dashboard. The `serve`,
//! `analyze`, `train`, `validate`, and `telemetry` subcommands now
//! compose real crate APIs (GraphQL + SSE server, polars analysis
//! pipeline, artifact validation, storage-backed telemetry query).

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]

use std::io;
use std::path::PathBuf;
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
    /// Start the desktop server (GraphQL + SSE).
    Serve(ServeArgs),
    /// Run a one-shot analysis pipeline on a JSON data file.
    Analyze(AnalyzeArgs),
    /// Train a model and emit an artifact (requires `tch-backend` feature).
    Train(TrainArgs),
    /// Validate a model artifact without activating it.
    Validate(ValidateArgs),
    /// Inspect telemetry stored in sled storage.
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
    /// MQTT broker host (used when `--enable-mqtt` is set).
    #[arg(long, default_value = "localhost")]
    mqtt_host: String,
    /// MQTT broker port (used when `--enable-mqtt` is set).
    #[arg(long, default_value_t = 1883)]
    mqtt_port: u16,
    /// MQTT client id (used when `--enable-mqtt` is set).
    #[arg(long, default_value = "themql-desktop")]
    mqtt_client_id: String,
    /// MQTT broker username (used when `--enable-mqtt` is set). Per
    /// `specs/auth.toml [authn.mqtt]`, credentials must come from env or
    /// CLI, not source.
    #[arg(long)]
    mqtt_username: Option<String>,
    /// MQTT broker password (used when `--enable-mqtt` is set).
    #[arg(long)]
    mqtt_password: Option<String>,
    /// Whether to enable GraphQL session auth via `better-auth`.
    #[arg(long, default_value_t = false)]
    enable_auth: bool,
    /// Auth secret for JWT session signing (>= 32 chars). Can also be
    /// set via `THEMQL_AUTH_SECRET` env var. Per `specs/auth.toml
    /// [authn.graphql]`, must not appear in source.
    #[arg(long)]
    auth_secret: Option<String>,
}

/// Arguments for the `analyze` subcommand.
#[derive(Debug, Clone, Parser)]
struct AnalyzeArgs {
    /// Path to the analysis input file (JSON array of rows).
    #[arg(long)]
    input: String,
    /// Column headers (comma-separated).
    #[arg(long)]
    headers: String,
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
    /// Path to the sled storage directory.
    #[arg(long, default_value = "themql-data")]
    storage: String,
}

/// Dispatch a parsed [`Command`] to its implementation.
///
/// # Errors
/// Returns [`themql_core::Error`] if the subcommand fails.
async fn dispatch(command: &Command) -> Result<(), themql_core::Error> {
    match command {
        Command::Serve(args) => serve(args).await,
        Command::Analyze(args) => analyze(args).await,
        Command::Train(args) => train(args).await,
        Command::Validate(args) => validate(args),
        Command::Telemetry(args) => telemetry(args).await,
        Command::Tui => {
            if let Err(e) = tui_main() {
                eprintln!("themql-desktop: tui error: {e}");
            }
            Ok(())
        }
    }
}

/// Stub resolver for the serve subcommand — returns the subject as JSON.
struct DesktopResolver;

#[allow(clippy::unused_async_trait_impl)]
impl themql_core::ResolverBoxed for DesktopResolver {
    async fn resolve(
        &self,
        query: &themql_core::Query,
        _ctx: &themql_core::Context,
    ) -> Result<themql_core::Response, themql_core::Error> {
        let subject = query.resource.subject.as_str();
        Ok(themql_core::Response::ok(
            themql_core::ResponseValue::Json(serde_json::json!({ "subject": subject })),
            themql_core::CorrelationId::new(),
        ))
    }
}

/// Stub message handler for the serve subcommand.
struct DesktopHandler;

impl themql_core::MessageHandler for DesktopHandler {
    fn handle<'a>(
        &'a self,
        msg: &'a themql_core::Message,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<themql_core::Response, themql_core::Error>>
                + Send
                + 'a,
        >,
    > {
        let subject = msg.subject.as_str();
        let payload = msg.payload.clone();
        Box::pin(async move {
            Ok(themql_core::Response::ok(
                themql_core::ResponseValue::Json(serde_json::json!({
                    "subject": subject,
                    "echo": payload,
                })),
                themql_core::CorrelationId::new(),
            ))
        })
    }
}

/// MQTT-to-SSE bridge: re-publishes incoming MQTT messages to the SSE
/// publisher. Per `specs/transport.toml [bridges]`, the bridge routes
/// via `themql-core Message`, never adapter-to-adapter. This struct
/// implements `MessageHandler` so it can be registered with
/// `MqttSubscriber::subscribe`.
struct MqttToSseBridge {
    publisher: std::sync::Arc<themql_sse::TokioSsePublisher>,
}

impl MqttToSseBridge {
    /// Construct a bridge that forwards to the given SSE publisher.
    #[must_use]
    fn new(publisher: std::sync::Arc<themql_sse::TokioSsePublisher>) -> Self {
        Self { publisher }
    }
}

impl themql_core::MessageHandler for MqttToSseBridge {
    fn handle<'a>(
        &'a self,
        msg: &'a themql_core::Message,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<themql_core::Response, themql_core::Error>>
                + Send
                + 'a,
        >,
    > {
        use themql_sse::SsePublisher;
        let publisher = std::sync::Arc::clone(&self.publisher);
        let subject = msg.subject.clone();
        let msg_clone = msg.clone();
        Box::pin(async move {
            publisher
                .broadcast(&subject, &msg_clone)
                .await
                .map_err(|e| themql_core::Error::transport_error(e.to_string()))?;
            Ok(themql_core::Response::ok(
                themql_core::ResponseValue::Json(serde_json::json!({
                    "bridged": true,
                    "subject": subject.as_str(),
                })),
                themql_core::CorrelationId::new(),
            ))
        })
    }
}

/// Start the desktop server: GraphQL (POST /graphql + WS /graphql) and
/// SSE (GET /events) on the same axum router. When `--enable-mqtt` is
/// set, an MQTT-to-SSE bridge forwards incoming MQTT messages on
/// `vehicle.events` to the SSE publisher. When `--enable-auth` is set,
/// `better-auth` session auth routes are mounted at `/api/auth/*` and
/// GraphQL field guards are activated with a default `Admin` role.
/// MQTT ACLs are applied based on the `--mqtt-username` role mapping.
#[allow(clippy::too_many_lines)]
async fn serve(args: &ServeArgs) -> Result<(), themql_core::Error> {
    use better_auth::handlers::axum::AxumIntegration;
    use std::sync::Arc;
    use themql_graphql::{
        AuthRole, DispatchBridgeImpl, GraphqlResolverBridgeImpl, GraphqlSchema, GraphqlSchemaImpl,
        MutationRoot, QueryRoot, SubscriptionRoot,
    };
    use themql_sse::TokioSsePublisher;

    let resolver: Arc<dyn themql_core::Resolver> = Arc::new(DesktopResolver);
    let bridge = Arc::new(GraphqlResolverBridgeImpl::new(resolver));
    let handler: Arc<dyn themql_core::MessageHandler> = Arc::new(DesktopHandler);
    let dispatch = Arc::new(DispatchBridgeImpl::new(handler));

    let publisher = Arc::new(TokioSsePublisher::new());
    let source: Arc<dyn themql_graphql::GraphqlSubscriptionSource> =
        Arc::clone(&publisher) as Arc<dyn themql_graphql::GraphqlSubscriptionSource>;

    let schema = GraphqlSchemaImpl::with_role(
        QueryRoot::new(bridge),
        MutationRoot::new(dispatch),
        SubscriptionRoot::with_source(source),
        AuthRole::Admin,
    );

    let sse_subject = themql_core::Subject::from_str("vehicle.events")
        .map_err(|e| themql_core::Error::validation_error(e.to_string()))?;
    let sse_router = themql_sse::serve_sse_with_publisher(Arc::clone(&publisher), sse_subject);
    let graphql_router = themql_graphql::serve_graphql(schema.schema().clone());
    let mut app = graphql_router.merge(sse_router);

    if args.enable_auth {
        let secret = if let Some(s) = &args.auth_secret {
            s.clone()
        } else if let Ok(s) = std::env::var("THEMQL_AUTH_SECRET") {
            s
        } else {
            return Err(themql_core::Error::validation_error(
                "auth enabled but no secret provided (--auth-secret or THEMQL_AUTH_SECRET)",
            ));
        };
        if secret.len() < 32 {
            return Err(themql_core::Error::validation_error(
                "auth secret must be at least 32 characters",
            ));
        }

        let auth_config = better_auth::AuthConfig::new(secret)
            .base_url(format!("http://{}:{}", args.bind, args.port));
        let auth = better_auth::AuthBuilder::new(auth_config)
            .database(better_auth::MemoryDatabaseAdapter::new())
            .plugin(better_auth::plugins::EmailPasswordPlugin::new())
            .build()
            .await
            .map_err(|e| themql_core::Error::internal_error(format!("auth init: {e}")))?;

        let auth_arc = Arc::new(auth);
        let auth_router = Arc::clone(&auth_arc).axum_router().with_state(auth_arc);
        app = app.merge(auth_router);

        println!("themql-desktop: auth enabled (better-auth, email/password)");
    }

    let addr: std::net::SocketAddr = format!("{}:{}", args.bind, args.port)
        .parse()
        .map_err(|e| themql_core::Error::internal_error(format!("invalid bind address: {e}")))?;
    println!("themql-desktop: serving GraphQL + SSE on http://{addr}");

    if args.enable_mqtt {
        use themql_mqtt::MqttSubscriber;
        let mut mqtt_config = themql_mqtt::RumqttcConfig::new(
            args.mqtt_host.as_str(),
            args.mqtt_port,
            args.mqtt_client_id.as_str(),
        );
        if let (Some(u), Some(p)) = (&args.mqtt_username, &args.mqtt_password) {
            mqtt_config = mqtt_config.with_credentials(u, p);
            let acl = match u.as_str() {
                "operator" => themql_mqtt::operator_acl(),
                "observer" => themql_mqtt::observer_acl(),
                _ => themql_mqtt::admin_acl(),
            };
            mqtt_config = mqtt_config.with_acl(acl);
        }
        let transport = Arc::new(themql_mqtt::RumqttcTransport::new(&mqtt_config));
        let bridge_publisher = Arc::clone(&publisher);
        let mqtt_bridge = MqttToSseBridge::new(bridge_publisher);

        let subscribe_subject = themql_core::Subject::from_str("vehicle.events")
            .map_err(|e| themql_core::Error::validation_error(e.to_string()))?;
        transport
            .subscribe(&subscribe_subject, mqtt_bridge)
            .await
            .map_err(|e| themql_core::Error::transport_error(e.to_string()))?;

        let poll_transport = Arc::clone(&transport);
        tokio::spawn(async move {
            loop {
                match poll_transport.poll().await {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("themql-desktop: MQTT poll error: {e}");
                        break;
                    }
                }
            }
        });

        let cred_msg = if args.mqtt_username.is_some() {
            " (authenticated)"
        } else {
            ""
        };
        println!(
            "themql-desktop: MQTT bridge connected to {}:{}{cred_msg} (subscribing to vehicle.events)",
            args.mqtt_host, args.mqtt_port
        );
    }

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| themql_core::Error::internal_error(format!("bind failed: {e}")))?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|e| themql_core::Error::internal_error(format!("server error: {e}")))?;
    Ok(())
}

/// Wait for Ctrl-C to shut down the server.
async fn shutdown_signal() {
    tokio::signal::ctrl_c().await.unwrap_or_else(|_| {
        eprintln!("themql-desktop: shutdown signal failed");
    });
}

/// Run a one-shot analysis pipeline on a JSON data file.
async fn analyze(args: &AnalyzeArgs) -> Result<(), themql_core::Error> {
    use themql_analysis::{
        AnalysisInput, AnalysisPipeline, PolarsDatasetBuilder, RayonAnalysisPipeline,
    };

    let input = std::fs::read_to_string(&args.input)
        .map_err(|e| themql_core::Error::internal_error(format!("read input: {e}")))?;
    let json_rows: Vec<serde_json::Value> = serde_json::from_str(&input)
        .map_err(|e| themql_core::Error::internal_error(format!("parse JSON: {e}")))?;
    let headers: Vec<String> = args.headers.split(',').map(String::from).collect();

    let mut rows: Vec<Vec<f64>> = Vec::with_capacity(json_rows.len());
    for json_row in &json_rows {
        let row: Vec<f64> = match json_row {
            serde_json::Value::Array(arr) => {
                arr.iter().map(|v| v.as_f64().unwrap_or(0.0)).collect()
            }
            _ => {
                return Err(themql_core::Error::validation_error(
                    "each row must be a JSON array of numbers",
                ));
            }
        };
        rows.push(row);
    }

    let builder = PolarsDatasetBuilder::new(headers.clone());
    let frame = builder
        .build_from_rows(&headers, &rows)
        .map_err(|e| themql_core::Error::internal_error(e.to_string()))?;

    let pipeline = RayonAnalysisPipeline::new();
    let ctx = themql_core::Context::new();
    let analysis = pipeline
        .run(&AnalysisInput::DataFrame(frame), &ctx)
        .await
        .map_err(|e| themql_core::Error::internal_error(e.to_string()))?;

    println!("themql-desktop: analysis complete");
    println!("  rows: {}", analysis.stats.rows);
    println!("  columns: {}", analysis.stats.columns);
    println!("  null cells: {}", analysis.stats.null_count);
    Ok(())
}

/// Train a model and emit an artifact. Requires the `tch-backend` feature.
#[allow(clippy::unused_async)]
async fn train(args: &TrainArgs) -> Result<(), themql_core::Error> {
    #[cfg(not(feature = "tch-backend"))]
    {
        let _ = args;
        eprintln!("themql-desktop: train requires the `tch-backend` feature");
        eprintln!("  rebuild with: cargo run --features tch-backend -- train ...");
        Err(themql_core::Error::internal_error(
            "tch-backend feature not enabled",
        ))
    }

    #[cfg(feature = "tch-backend")]
    {
        use themql_artifact::{ArtifactMetadata, ArtifactWriter, BincodeArtifactWriter};
        use themql_schema::{FeatureDType, FeatureSchema, TrainedModel, ValidationMetrics};
        use themql_training::{Dataset, TchTrainer, Trainer, TrainerKind, TrainingConfig};

        let dataset_bytes = std::fs::read(&args.dataset)
            .map_err(|e| themql_core::Error::internal_error(format!("read dataset: {e}")))?;
        let dataset: Dataset = bincode::deserialize(&dataset_bytes)
            .map_err(|e| themql_core::Error::internal_error(format!("deserialize dataset: {e}")))?;

        let feature_schema = FeatureSchema {
            features: vec![],
            input_dim: 1,
            output_dim: 1,
            dtype: FeatureDType::F32,
            normalization: themql_schema::NormalizationSpec::None,
        };

        let kind = match args.kind.as_str() {
            "dense" => TrainerKind::Dense,
            "pinn" => TrainerKind::Pinn,
            "gradient_boosting" => TrainerKind::GradientBoosting,
            "fine_tuning" => TrainerKind::FineTuning,
            other => {
                return Err(themql_core::Error::validation_error(format!(
                    "unknown trainer kind: {other}"
                )))
            }
        };

        let config = TrainingConfig {
            kind,
            epochs: args.epochs,
            batch_size: 32,
            learning_rate: 1.0e-3,
            weight_decay: None,
            early_stopping: None,
            pruning: None,
            sparsification: None,
        };

        let trainer = TchTrainer::new(kind);
        let model: TrainedModel = trainer
            .train(&dataset, &config)
            .await
            .map_err(|e| themql_core::Error::internal_error(e.to_string()))?;

        let metadata = ArtifactMetadata {
            model_id: format!(
                "themql-{}-{}",
                args.kind,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0)
            ),
            training_version: "0.1".to_owned(),
            dataset_version: "0.1".to_owned(),
            feature_schema: feature_schema.clone(),
            normalization: themql_schema::NormalizationSpec::None,
            validation_metrics: ValidationMetrics {
                loss: 0.0,
                accuracy: None,
                custom: std::collections::BTreeMap::new(),
            },
            pruning_metadata: None,
        };

        let writer = BincodeArtifactWriter::new("0.1", &args.kind);
        let artifact = writer
            .write(&model, &metadata)
            .map_err(|e| themql_core::Error::internal_error(e.to_string()))?;
        writer
            .write_to_file(&artifact, &std::path::Path::new(&args.output))
            .map_err(|e| themql_core::Error::internal_error(e.to_string()))?;

        println!(
            "themql-desktop: model trained and written to {}",
            args.output
        );
        Ok(())
    }
}

/// Validate a model artifact without activating it.
fn validate(args: &ValidateArgs) -> Result<(), themql_core::Error> {
    use themql_artifact::{ArtifactLoader, FileArtifactLoader};

    let loader = FileArtifactLoader::new();
    let path = PathBuf::from(&args.artifact);
    let artifact = loader
        .load(&path)
        .map_err(|e| themql_core::Error::internal_error(e.to_string()))?;

    println!("themql-desktop: artifact validation passed");
    println!("  format: {:?}", artifact.format);
    println!("  schema_version: {}", artifact.schema_version);
    println!("  model_bytes: {} bytes", artifact.model_bytes.len());
    println!("  hash (first 16): {}", hex_first_16(&artifact.hash));
    Ok(())
}

/// Format the first 16 bytes of a hash as hex.
fn hex_first_16(bytes: &[u8]) -> String {
    bytes.iter().take(16).fold(String::new(), |mut acc, b| {
        use std::fmt::Write;
        let _ = write!(acc, "{b:02x}");
        acc
    })
}

/// Inspect telemetry stored in sled storage.
async fn telemetry(args: &TelemetryArgs) -> Result<(), themql_core::Error> {
    use themql_storage::{SledStorage, Storage, StorageQuery};

    let storage = SledStorage::open(&args.storage)
        .map_err(|e| themql_core::Error::internal_error(e.to_string()))?;

    let pattern = themql_core::SubjectPattern::from_str(&args.subject)
        .map_err(|e| themql_core::Error::validation_error(e.to_string()))?;
    let query = StorageQuery::BySubjectPattern(pattern);
    let result_set = storage
        .query(&query)
        .await
        .map_err(|e| themql_core::Error::internal_error(e.to_string()))?;

    println!(
        "themql-desktop: telemetry query '{subject}' returned {n} entries",
        subject = args.subject,
        n = result_set.entries.len()
    );
    for (key, value) in &result_set.entries {
        println!(
            "  key={key} format={format} {len}B",
            key = key.as_str(),
            format = value.format,
            len = value.bytes.len()
        );
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
    dispatch(&cli.command).await?;
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
        let cli = Cli::parse_from([
            "themql-desktop",
            "analyze",
            "--input",
            "x.json",
            "--headers",
            "a,b,c",
        ]);
        match cli.command {
            Command::Analyze(args) => {
                assert_eq!(args.input, "x.json");
                assert_eq!(args.headers, "a,b,c");
            }
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

    #[cfg(not(feature = "tch-backend"))]
    #[tokio::test]
    async fn dispatch_train_without_feature_returns_error() {
        let result = train(&TrainArgs {
            dataset: "d.bin".to_owned(),
            kind: "dense".to_owned(),
            epochs: 1,
            output: "o.tar".to_owned(),
        })
        .await;
        assert!(result.is_err());
    }

    #[test]
    fn dispatch_validate_nonexistent_file_returns_error() {
        let result = validate(&ValidateArgs {
            artifact: "/nonexistent/path/model.bin".to_owned(),
        });
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn dispatch_telemetry_nonexistent_storage_returns_error() {
        let result = telemetry(&TelemetryArgs {
            subject: "vehicle.#".to_owned(),
            storage: "/nonexistent/path/sled".to_owned(),
        })
        .await;
        assert!(result.is_err());
    }

    #[test]
    fn draw_dashboard_runs_with_test_backend() {
        let backend = ratatui::backend::TestBackend::new(40, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal.draw(draw_dashboard).unwrap();
    }

    #[test]
    fn hex_first_16_formats_correctly() {
        let bytes = [0xab, 0xcd, 0xef];
        let hex = hex_first_16(&bytes);
        assert_eq!(hex, "abcdef");
    }

    #[test]
    fn mqtt_to_sse_bridge_constructs() {
        use std::sync::Arc;
        use themql_sse::TokioSsePublisher;
        let publisher = Arc::new(TokioSsePublisher::new());
        let bridge = MqttToSseBridge::new(publisher);
        let _ = &bridge;
    }

    #[test]
    fn mqtt_transport_constructs_without_io() {
        let config = themql_mqtt::RumqttcConfig::new("localhost", 1883, "themql-desktop-test");
        let _transport = themql_mqtt::RumqttcTransport::new(&config);
    }

    #[test]
    fn cli_parses_serve_with_mqtt_args() {
        let cli = Cli::parse_from([
            "themql-desktop",
            "serve",
            "--enable-mqtt",
            "--mqtt-host",
            "broker.local",
            "--mqtt-port",
            "8883",
            "--mqtt-client-id",
            "test-client",
            "--mqtt-username",
            "op",
            "--mqtt-password",
            "secret",
        ]);
        match cli.command {
            Command::Serve(args) => {
                assert!(args.enable_mqtt);
                assert_eq!(args.mqtt_host, "broker.local");
                assert_eq!(args.mqtt_port, 8883);
                assert_eq!(args.mqtt_client_id, "test-client");
                assert_eq!(args.mqtt_username.as_deref(), Some("op"));
                assert_eq!(args.mqtt_password.as_deref(), Some("secret"));
            }
            _ => panic!("must parse Serve subcommand"),
        }
    }

    #[test]
    fn cli_parses_serve_with_auth_args() {
        let cli = Cli::parse_from([
            "themql-desktop",
            "serve",
            "--enable-auth",
            "--auth-secret",
            "this-is-a-very-long-secret-key-for-testing-ok",
        ]);
        match cli.command {
            Command::Serve(args) => {
                assert!(args.enable_auth);
                assert_eq!(
                    args.auth_secret.as_deref(),
                    Some("this-is-a-very-long-secret-key-for-testing-ok")
                );
            }
            _ => panic!("must parse Serve subcommand"),
        }
    }

    #[tokio::test]
    async fn auth_builds_with_memory_adapter() {
        let config = better_auth::AuthConfig::new("test-secret-key-that-is-at-least-32-chars!!");
        let auth = better_auth::AuthBuilder::new(config)
            .database(better_auth::MemoryDatabaseAdapter::new())
            .plugin(better_auth::plugins::EmailPasswordPlugin::new())
            .build()
            .await;
        assert!(auth.is_ok());
    }

    #[test]
    fn mqtt_acl_maps_username_to_role() {
        let admin_acl = themql_mqtt::admin_acl();
        assert!(admin_acl.permits(themql_mqtt::AclAction::Publish, "system.config"));
        assert!(admin_acl.permits(themql_mqtt::AclAction::Subscribe, "anything"));

        let op_acl = themql_mqtt::operator_acl();
        assert!(op_acl.permits(themql_mqtt::AclAction::Publish, "vehicle.events"));
        assert!(!op_acl.permits(themql_mqtt::AclAction::Publish, "system.config"));

        let obs_acl = themql_mqtt::observer_acl();
        assert!(obs_acl.permits(themql_mqtt::AclAction::Subscribe, "vehicle.events"));
        assert!(!obs_acl.permits(themql_mqtt::AclAction::Publish, "vehicle.events"));
    }

    #[test]
    fn graphql_auth_role_hierarchy() {
        use themql_graphql::AuthRole;
        assert!(AuthRole::Admin.satisfies(AuthRole::Operator));
        assert!(AuthRole::Admin.satisfies(AuthRole::Observer));
        assert!(AuthRole::Operator.satisfies(AuthRole::Observer));
        assert!(!AuthRole::Observer.satisfies(AuthRole::Operator));
    }
}
