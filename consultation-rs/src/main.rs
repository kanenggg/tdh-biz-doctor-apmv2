use anyhow::Result;
use axum::{Router, routing::get};
use clap::{self, Parser};
use std::sync::Arc;
use tracing::Level;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
mod appointment;
mod common;
mod consultation;
mod consultation_config;
mod doctor_timeslot;
mod infra;
mod internal;
mod openapi;
mod protocol;
mod provider_callback;
mod repo;
mod summarization;
mod sys;

use crate::openapi::ApiDoc;

async fn redoc_handler() -> axum::response::Html<&'static str> {
    axum::response::Html(include_str!("../openapi/redoc.html"))
}

#[derive(clap::Parser)]
#[command(author, version, about = "Consultation Service API")]
pub(crate) struct CommandArgs {
    #[arg(short, long, value_name = "FILE")]
    config_path: Vec<std::path::PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    let cli = CommandArgs::try_parse()?;

    tracing::info!("consultation-rs service starting...");
    dotenv::dotenv().ok();
    init_logging();

    let config = sys::config::AppConfig::from(&cli.config_path).map_err(|e| {
        tracing::error!("Failed to load configuration: {}", e);
        e
    })?;

    tracing::info!("Configuration loaded successfully");

    let infra = common::infrastructure::Infrastructure::new(config.clone()).await?;

    tracing::info!("Initializing PubSub publisher...");
    let pubsub_publisher = Arc::new(
        infra::event::PubSubEventPublisher::new(
            &infra.config.google_cloud.project_id,
            &infra.config.booking.pubsub_topic,
            infra.config.google_cloud.pubsub_emulator_host.as_deref(),
        )
        .await?,
    );
    let event_publisher: Arc<dyn infra::event::EventPublisher> =
        Arc::new(infra::event::OutboxEventPublisher::new(
            infra.db_pool.clone(),
            infra.config.booking.pubsub_topic.clone(),
            pubsub_publisher,
        ));

    let infra = infra.with_event_publisher(event_publisher);

    let consultation_app_state = consultation::bootstrap(&infra);
    let consultation_config_app_state = consultation_config::bootstrap(&infra);
    let appointment_app_state = appointment::bootstrap(&infra).await?;
    let appointment_hold_app_state = appointment::hold::bootstrap(&infra);
    let doctor_timeslot_app_state = doctor_timeslot::bootstrap(&infra);
    let internal_app_state = internal::bootstrap(&infra);
    let summarization_app_state = summarization::bootstrap(&infra).await?;
    let provider_callback_app_state = provider_callback::bootstrap(&infra);

    let router = Router::new()
        .merge(consultation::router().with_state(consultation_app_state))
        .merge(consultation_config::router().with_state(consultation_config_app_state))
        .merge(appointment::router().with_state(appointment_app_state))
        .merge(appointment::hold::router().with_state(appointment_hold_app_state))
        .merge(doctor_timeslot::router().with_state(doctor_timeslot_app_state))
        .merge(internal::router().with_state(internal_app_state))
        .merge(summarization::router().with_state(summarization_app_state))
        .merge(provider_callback::router().with_state(provider_callback_app_state))
        .merge(SwaggerUi::new("/docs/swagger").url("/docs/openapi.json", ApiDoc::openapi()))
        .route("/docs/health", get(health_check))
        .route("/docs/redoc", get(redoc_handler));

    let addr = format!("{}:{}", infra.config.server.host, infra.config.server.port);
    tracing::info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    let local_addr = listener.local_addr()?;
    tracing::info!("Server listening on {}", local_addr);

    axum::serve(listener, router).await?;

    Ok(())
}

async fn health_check() -> &'static str {
    "OK: consultation-rs service is running"
}

fn init_logging() {
    use tracing_subscriber::prelude::*;

    let rust_log = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into());

    let use_json_logging = std::env::var("K_SERVICE").is_ok()
        || std::env::var("KUBERNETES_SERVICE_HOST").is_ok()
        || std::env::var("LOG_FORMAT")
            .map(|v| v == "json")
            .unwrap_or(false);

    if use_json_logging {
        tracing_subscriber::registry()
            .with(tracing_subscriber::EnvFilter::new(&rust_log))
            .with(
                tracing_subscriber::fmt::layer()
                    .json()
                    .with_target(true)
                    .with_current_span(false)
                    .with_file(false)
                    .with_line_number(false)
                    .map_event_format(|_| CloudRunFormatter),
            )
            .init();
    } else {
        tracing_subscriber::fmt().with_env_filter(rust_log).init();
    }
}

struct CloudRunFormatter;

impl<S, N> tracing_subscriber::fmt::FormatEvent<S, N> for CloudRunFormatter
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
    N: for<'a> tracing_subscriber::fmt::FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        _ctx: &tracing_subscriber::fmt::FmtContext<'_, S, N>,
        mut writer: tracing_subscriber::fmt::format::Writer<'_>,
        event: &tracing::Event<'_>,
    ) -> std::fmt::Result {
        let severity = match *event.metadata().level() {
            Level::ERROR => "ERROR",
            Level::WARN => "WARNING",
            Level::INFO => "INFO",
            Level::DEBUG => "DEBUG",
            Level::TRACE => "DEBUG",
        };

        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);

        write!(
            writer,
            r#"{{"severity":"{}","message":"{}","target":"{}","timestamp":"{}"}}"#,
            severity,
            visitor.message.replace('\\', "\\\\").replace('"', "\\\""),
            event.metadata().target(),
            jiff::Timestamp::now(),
        )?;
        writeln!(writer)
    }
}

#[derive(Default)]
struct MessageVisitor {
    message: String,
}

impl tracing::field::Visit for MessageVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{:?}", value);
            if self.message.starts_with('"') && self.message.ends_with('"') {
                self.message = self.message[1..self.message.len() - 1].to_string();
            }
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        }
    }
}
