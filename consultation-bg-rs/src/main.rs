use std::sync::Arc;

use axum::{Router, routing::post};
use clap::Parser;

mod appointment_hold;
mod common;
mod doctor_identity;
mod event;
mod event_outbox;
mod payment_confirm;
mod sys;

use payment_confirm::{
    handler as payment_handler, payement_token::PaymentVerifierWithPaseto,
    repo::PaymentConfirmPsql, service::PaymentConfirmService,
};
use sys::config::AppConfig;

#[derive(Parser)]
#[command(author, version, about = "Consultation Background Service")]
struct Args {
    #[arg(short, long, value_name = "FILE")]
    config_path: Vec<std::path::PathBuf>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args = Args::try_parse()?;
    let config = AppConfig::from(&args.config_path)?;

    tracing::info!(
        host = %config.server.host,
        port = config.server.port,
        "Starting consultation-bg-rs service"
    );

    // Database pool
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.database.connection_url())
        .await?;
    tracing::info!("Database connected");

    // Build service
    let payment_state = if config.payment_confirm.enabled {
        let verifier = PaymentVerifierWithPaseto::new(&config.payment.public_key)?;
        let repo = PaymentConfirmPsql::new(pool.clone());
        Some(Arc::new(payment_handler::AppState {
            service: Arc::new(PaymentConfirmService::new(
                Arc::new(verifier),
                Arc::new(repo),
                Some(config.consultation_event.pubsub_topic.clone()),
            )),
        }))
    } else {
        tracing::warn!("payment confirmation route disabled; projection-only safe mode");
        None
    };

    let doctor_identity_repo = doctor_identity::repo::DoctorIdentityPsql::new(pool.clone());
    let doctor_identity_service = Arc::new(doctor_identity::service::DoctorIdentityService::new(
        Arc::new(doctor_identity_repo),
        config.doctor_projection.allowed_schema_versions.clone(),
    ));
    let doctor_projection_sync_auth = Arc::new(
        doctor_identity::auth::DoctorProjectionSyncAuth::new(config.doctor_projection_sync.clone()),
    );
    let doctor_identity_state = Arc::new(doctor_identity::handler::AppState {
        service: doctor_identity_service,
        sync_auth: doctor_projection_sync_auth,
    });

    let outbox_publisher = Arc::new(
        event::PubSubConsultationEventPublisher::new(
            &config.google_cloud.project_id,
            config.google_cloud.pubsub_emulator_host.as_deref(),
        )
        .await?,
    );
    let outbox_dispatch_service = Arc::new(event_outbox::service::EventOutboxDispatchService::new(
        Arc::new(event_outbox::repo::EventOutboxPsql::new(pool.clone())),
        outbox_publisher,
        config.event_outbox.lock_seconds,
    ));
    event_outbox::worker::spawn_event_outbox_worker(
        outbox_dispatch_service,
        event_outbox::worker::EventOutboxWorkerConfig {
            enabled: config.event_outbox.enabled,
            poll_interval_seconds: config.event_outbox.poll_interval_seconds,
            batch_size: config.event_outbox.batch_size,
        },
    );

    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let expiry_service = Arc::new(
        appointment_hold::service::AppointmentHoldExpiryService::new(
            Arc::new(appointment_hold::repo::AppointmentHoldExpiryPsql::new(
                pool.clone(),
            )),
            config.consultation_event.pubsub_topic.clone(),
        ),
    );
    let expiry_worker = appointment_hold::worker::spawn_appointment_hold_expiry_worker(
        expiry_service,
        appointment_hold::worker::AppointmentHoldExpiryWorkerConfig {
            enabled: config.appointment_hold_expiry.enabled,
            poll_interval_seconds: config.appointment_hold_expiry.poll_interval_seconds,
            batch_size: config.appointment_hold_expiry.batch_size,
        },
        shutdown_rx,
    );

    // Router
    let payment_router = payment_state.map_or_else(Router::new, |state| {
        Router::new()
            .route(
                "/pubsub/payment-confirm",
                post(payment_handler::handle_pubsub_push),
            )
            .with_state(state)
    });
    let doctor_identity_router = doctor_identity::handler::routes(doctor_identity_state);
    let app = Router::new()
        .merge(payment_router)
        .merge(doctor_identity_router);

    // Start server
    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Server listening on {}", addr);
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let _ = tokio::signal::ctrl_c().await;
            let _ = shutdown_tx.send(true);
        })
        .await?;
    let _ = tokio::time::timeout(std::time::Duration::from_secs(10), expiry_worker).await;

    Ok(())
}
