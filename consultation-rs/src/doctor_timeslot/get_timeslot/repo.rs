use sqlx::PgPool;
use uuid::Uuid;

use crate::consultation_config::model::ScheduleAvailableConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorTimeslotIdentity {
    pub doctor_id: Uuid,
    pub doctor_account_id: i64,
    pub doctor_profile_id: i64,
    pub is_active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReservedWindow {
    pub booking_id: String,
    pub start_epoch: i64,
    pub end_epoch: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorTimeslotConfigSnapshot {
    pub schedule_available: bool,
    pub schedule_config: ScheduleAvailableConfig,
    pub service_config: Option<DoctorServiceConfigSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorServiceConfigSnapshot {
    pub channels: Vec<String>,
    pub duration_minutes: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorScheduleCandidate {
    pub identity: DoctorTimeslotIdentity,
    pub schedule_available: bool,
    pub schedule_config: ScheduleAvailableConfig,
    pub service_config: Option<DoctorServiceConfigSnapshot>,
}

#[async_trait::async_trait]
pub trait GetDoctorTimeslotRepo: Send + Sync {
    async fn resolve_doctor_identity(
        &self,
        doctor_account_id: i64,
        doctor_profile_id: i64,
    ) -> Result<Option<DoctorTimeslotIdentity>, anyhow::Error>;

    async fn resolve_doctor_identity_by_doctor_id(
        &self,
        doctor_id: Uuid,
    ) -> Result<Option<DoctorTimeslotIdentity>, anyhow::Error>;

    async fn list_schedule_available_doctors(
        &self,
    ) -> Result<Vec<DoctorScheduleCandidate>, anyhow::Error>;

    async fn get_config_snapshot(
        &self,
        doctor_id: Uuid,
    ) -> Result<DoctorTimeslotConfigSnapshot, anyhow::Error>;

    async fn list_reserved_windows(
        &self,
        doctor_profile_id: i64,
        from_epoch: i64,
        to_epoch: i64,
    ) -> Result<Vec<ReservedWindow>, anyhow::Error>;
}

pub struct GetDoctorTimeslotRepoPsql {
    pool: PgPool,
}

impl GetDoctorTimeslotRepoPsql {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl GetDoctorTimeslotRepo for GetDoctorTimeslotRepoPsql {
    async fn resolve_doctor_identity(
        &self,
        doctor_account_id: i64,
        doctor_profile_id: i64,
    ) -> Result<Option<DoctorTimeslotIdentity>, anyhow::Error> {
        let row = sqlx::query_as::<_, (Uuid, i64, i64, bool)>(
            r#"
            SELECT doctor_id, doctor_account_id, doctor_profile_id, is_active
            FROM v2.doctor_info_projection
            WHERE doctor_account_id = $1
              AND doctor_profile_id = $2
            "#,
        )
        .bind(doctor_account_id)
        .bind(doctor_profile_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to resolve doctor timeslot identity: {e}"))?;

        Ok(row.map(
            |(doctor_id, doctor_account_id, doctor_profile_id, is_active)| DoctorTimeslotIdentity {
                doctor_id,
                doctor_account_id,
                doctor_profile_id,
                is_active,
            },
        ))
    }

    async fn resolve_doctor_identity_by_doctor_id(
        &self,
        doctor_id: Uuid,
    ) -> Result<Option<DoctorTimeslotIdentity>, anyhow::Error> {
        let row = sqlx::query_as::<_, (Uuid, i64, i64, bool)>(
            r#"
            SELECT doctor_id, doctor_account_id, doctor_profile_id, is_active
            FROM v2.doctor_info_projection
            WHERE doctor_id = $1
            "#,
        )
        .bind(doctor_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            anyhow::anyhow!("Failed to resolve doctor timeslot identity by doctor_id: {e}")
        })?;

        Ok(row.map(
            |(doctor_id, doctor_account_id, doctor_profile_id, is_active)| DoctorTimeslotIdentity {
                doctor_id,
                doctor_account_id,
                doctor_profile_id,
                is_active,
            },
        ))
    }

    async fn list_schedule_available_doctors(
        &self,
    ) -> Result<Vec<DoctorScheduleCandidate>, anyhow::Error> {
        let rows = sqlx::query_as::<
            _,
            (
                Uuid,
                i64,
                i64,
                bool,
                bool,
                Option<serde_json::Value>,
                Option<Vec<String>>,
                Option<i32>,
            ),
        >(
            r#"
            SELECT
                dip.doctor_id,
                dip.doctor_account_id,
                dip.doctor_profile_id,
                dip.is_active,
                dcc.schedule_available,
                dcc.schedule_config,
                dscp.channels,
                dscp.duration_minutes
            FROM v2.doctor_info_projection dip
            JOIN v2.doctor_consultation_config dcc ON dcc.doctor_id = dip.doctor_id
            LEFT JOIN v2.doctor_service_config_projection dscp ON dscp.doctor_id = dip.doctor_id
            WHERE dip.is_active = true
              AND dcc.schedule_available = true
            ORDER BY dip.doctor_profile_id
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to list schedule available doctors: {e}"))?;

        rows.into_iter()
            .map(
                |(
                    doctor_id,
                    doctor_account_id,
                    doctor_profile_id,
                    is_active,
                    schedule_available,
                    schedule_config,
                    channels,
                    duration_minutes,
                )| {
                    Ok(DoctorScheduleCandidate {
                        identity: DoctorTimeslotIdentity {
                            doctor_id,
                            doctor_account_id,
                            doctor_profile_id,
                            is_active,
                        },
                        schedule_available,
                        schedule_config: match schedule_config {
                            Some(value) => serde_json::from_value(value)?,
                            None => ScheduleAvailableConfig::default(),
                        },
                        service_config: match (channels, duration_minutes) {
                            (Some(channels), Some(duration_minutes)) => {
                                Some(DoctorServiceConfigSnapshot {
                                    channels,
                                    duration_minutes,
                                })
                            }
                            _ => None,
                        },
                    })
                },
            )
            .collect()
    }

    async fn get_config_snapshot(
        &self,
        doctor_id: Uuid,
    ) -> Result<DoctorTimeslotConfigSnapshot, anyhow::Error> {
        let row = sqlx::query_as::<
            _,
            (
                bool,
                Option<serde_json::Value>,
                Option<Vec<String>>,
                Option<i32>,
            ),
        >(
            r#"
            SELECT dcc.schedule_available, dcc.schedule_config, dscp.channels, dscp.duration_minutes
            FROM v2.doctor_consultation_config dcc
            LEFT JOIN v2.doctor_service_config_projection dscp ON dscp.doctor_id = dcc.doctor_id
            WHERE dcc.doctor_id = $1
            "#,
        )
        .bind(doctor_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to fetch doctor timeslot config: {e}"))?;

        match row {
            Some((schedule_available, Some(schedule_config), channels, duration_minutes)) => {
                Ok(DoctorTimeslotConfigSnapshot {
                    schedule_available,
                    schedule_config: serde_json::from_value(schedule_config)?,
                    service_config: service_config(channels, duration_minutes),
                })
            }
            Some((schedule_available, None, channels, duration_minutes)) => {
                Ok(DoctorTimeslotConfigSnapshot {
                    schedule_available,
                    schedule_config: ScheduleAvailableConfig::default(),
                    service_config: service_config(channels, duration_minutes),
                })
            }
            None => Ok(DoctorTimeslotConfigSnapshot {
                schedule_available: false,
                schedule_config: ScheduleAvailableConfig::default(),
                service_config: None,
            }),
        }
    }

    async fn list_reserved_windows(
        &self,
        doctor_profile_id: i64,
        from_epoch: i64,
        to_epoch: i64,
    ) -> Result<Vec<ReservedWindow>, anyhow::Error> {
        let rows = sqlx::query_as::<_, (String, i64, i64)>(
            r#"
            SELECT COALESCE(h.booking_id, a.booking_id),
                EXTRACT(EPOCH FROM o.starts_at)::bigint AS start_epoch,
                EXTRACT(EPOCH FROM o.ends_at)::bigint AS end_epoch
            FROM v2.doctor_occupancy o
            LEFT JOIN v2.appointment_hold h ON h.hold_id = o.hold_id
            LEFT JOIN v2.appointment a ON a.appointment_id = o.appointment_id
            WHERE o.doctor_profile_id = $1
              AND o.occupancy_status = 'ACTIVE'::v2.doctor_occupancy_status_enum
              AND o.starts_at < to_timestamp($3)
              AND o.ends_at > to_timestamp($2)
            ORDER BY o.starts_at
            "#,
        )
        .bind(doctor_profile_id)
        .bind(from_epoch)
        .bind(to_epoch)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to fetch reserved windows: {e}"))?;

        Ok(rows
            .into_iter()
            .map(|(booking_id, start_epoch, end_epoch)| ReservedWindow {
                booking_id,
                start_epoch,
                end_epoch,
            })
            .collect())
    }
}

fn service_config(
    channels: Option<Vec<String>>,
    duration_minutes: Option<i32>,
) -> Option<DoctorServiceConfigSnapshot> {
    Some(DoctorServiceConfigSnapshot {
        channels: channels?,
        duration_minutes: duration_minutes?,
    })
}
