use crate::appointment_hold::service::AppointmentHoldExpiryService;
use std::{sync::Arc, time::Duration};
use tokio::sync::watch;

#[derive(Debug, Clone)]
pub(crate) struct AppointmentHoldExpiryWorkerConfig {
    pub enabled: bool,
    pub poll_interval_seconds: u64,
    pub batch_size: i32,
}

pub(crate) fn spawn_appointment_hold_expiry_worker(
    service: Arc<AppointmentHoldExpiryService>,
    config: AppointmentHoldExpiryWorkerConfig,
    mut shutdown: watch::Receiver<bool>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        if !config.enabled {
            tracing::info!("Appointment Hold expiry worker disabled");
            return;
        }
        let interval = Duration::from_secs(config.poll_interval_seconds.max(1));
        tracing::info!(
            poll_interval_seconds = interval.as_secs(),
            batch_size = config.batch_size,
            "Appointment Hold expiry worker started"
        );
        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    if *shutdown.borrow() { tracing::info!("Appointment Hold expiry worker stopped"); return; }
                }
                result = service.expire_due_holds(config.batch_size.max(1)) => match result {
                    Ok(count) if count > 0 => tracing::info!(expired_count = count, "Appointment Holds expired and occupancy released"),
                    Ok(_) => {},
                    Err(error) => tracing::error!(%error, "Appointment Hold expiry worker failed"),
                },
            }
            tokio::select! {
                _ = tokio::time::sleep(interval) => {},
                _ = shutdown.changed() => if *shutdown.borrow() { tracing::info!("Appointment Hold expiry worker stopped"); return; },
            }
        }
    })
}
