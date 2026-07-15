use fs2::FileExt;
use sqlx::postgres::{PgPool, PgPoolOptions};
use std::{
    env,
    fs::{File, OpenOptions},
    sync::{Arc, Once, OnceLock},
};
use testcontainers::{ContainerAsync, ImageExt, core::IntoContainerPort, runners::AsyncRunner};
use testcontainers_modules::postgres::Postgres;
use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};

static INIT_LOGGER: Once = Once::new();
static TEST_DATABASE_LOCK: OnceLock<Arc<AsyncMutex<()>>> = OnceLock::new();

fn test_database_lock() -> Arc<AsyncMutex<()>> {
    TEST_DATABASE_LOCK
        .get_or_init(|| Arc::new(AsyncMutex::new(())))
        .clone()
}

/// Initialize test logging (call once per test run)
pub fn init_test_logging() {
    INIT_LOGGER.call_once(|| {
        let log_level = env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
        tracing_subscriber::fmt()
            .with_env_filter(log_level)
            .with_test_writer()
            .try_init()
            .ok();
    });
}

pub struct TestDatabase {
    pub pool: PgPool,
    _container: Option<ContainerAsync<Postgres>>,
    _process_guard: OwnedMutexGuard<()>,
    _file_lock: File,
}

impl TestDatabase {
    pub async fn new() -> Self {
        let process_guard = test_database_lock().lock_owned().await;
        let file_lock = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open("/tmp/hermes-apmv2-integration-postgres.lock")
            .expect("integration database lock file must open");
        FileExt::lock_exclusive(&file_lock)
            .expect("integration database cross-process lock must be acquired");

        // Default to using testcontainers for test isolation
        // Set USE_EXISTING_DB=true to use existing database instead
        let use_existing_db = env::var("USE_EXISTING_DB")
            .unwrap_or_else(|_| "false".to_string())
            .parse::<bool>()
            .unwrap_or(false);

        if use_existing_db {
            Self::with_existing_db(process_guard, file_lock).await
        } else {
            Self::with_testcontainer(process_guard, file_lock).await
        }
    }

    async fn with_testcontainer(process_guard: OwnedMutexGuard<()>, file_lock: File) -> Self {
        tracing::info!("Starting PostgreSQL testcontainer...");

        let container = Postgres::default()
            .with_tag("14-alpine")
            .with_mapped_port(55433, 5432.tcp())
            .start()
            .await
            .expect("Failed to start PostgreSQL container");

        let host = container
            .get_host()
            .await
            .expect("Testcontainers host must be available");
        let database_url = format!("postgresql://postgres:postgres@{host}:55433/postgres");
        tracing::info!(host = %host, port = 55433, "Testcontainer PostgreSQL available");

        // Retry connection with exponential backoff (max 10 seconds)
        let pool = Self::connect_with_retry(&database_url, 5).await;

        // Run migrations
        run_migrations(&pool).await;
        run_seeds(&pool).await;
        tracing::info!("Database setup complete");

        Self {
            pool,
            _container: Some(container),
            _process_guard: process_guard,
            _file_lock: file_lock,
        }
    }

    async fn connect_with_retry(database_url: &str, max_retries: u32) -> PgPool {
        let mut retry_count = 0;
        let mut wait_ms = 500;

        loop {
            match PgPoolOptions::new()
                .max_connections(10)
                .acquire_timeout(std::time::Duration::from_secs(30))
                .connect(database_url)
                .await
            {
                Ok(pool) => {
                    tracing::info!("Successfully connected to testcontainer database");
                    return pool;
                }
                Err(e) => {
                    retry_count += 1;
                    if retry_count > max_retries {
                        panic!(
                            "Failed to connect to testcontainer database after {} retries: {}",
                            max_retries, e
                        );
                    }
                    tracing::warn!(
                        "Connection attempt {} failed, retrying in {}ms: {}",
                        retry_count,
                        wait_ms,
                        e
                    );
                    tokio::time::sleep(tokio::time::Duration::from_millis(wait_ms)).await;
                    wait_ms *= 2; // Exponential backoff
                }
            }
        }
    }

    async fn with_existing_db(process_guard: OwnedMutexGuard<()>, file_lock: File) -> Self {
        let database_url = env::var("TEST_DATABASE_URL").unwrap_or_else(|_| {
            "postgresql://biz_apm_admin:password@localhost:5432/biz_apm".to_string()
        });

        let pool = PgPoolOptions::new()
            .max_connections(10)
            .acquire_timeout(std::time::Duration::from_secs(30))
            .connect(&database_url)
            .await
            .expect("Failed to connect to test database");

        // Always run seeds for existing DB to ensure test data is present
        run_migrations(&pool).await;
        run_seeds(&pool).await;

        Self {
            pool,
            _container: None,
            _process_guard: process_guard,
            _file_lock: file_lock,
        }
    }
}

pub async fn setup_test_db() -> TestDatabase {
    init_test_logging();
    TestDatabase::new().await
}

async fn run_migrations(pool: &PgPool) {
    tracing::info!("Running database migrations...");
    sqlx::migrate!("../db/biz_apm/migrations")
        .run(pool)
        .await
        .expect("Failed to run migrations");

    // Execute each migration file directly
    // We can't use sqlx::migrate!() because our files use Flyway naming (V20260220...)
    // instead of sqlx naming (01_, 02_, etc.)

    // execute_migration_file(
    //     pool,
    //     "V20260220000001__create_schema",
    //     include_str!("../../../db/biz_apm/migrations/V20260220000001__create_schema.sql"),
    // )
    // .await;
    //
    // execute_migration_file(
    //     pool,
    //     "V20260220000002__init_types",
    //     include_str!("../../../db/biz_apm/migrations/V20260220000002__init_types.sql"),
    // )
    // .await;
    //
    // execute_migration_file(
    //     pool,
    //     "V20260220000003__new_biz_apm",
    //     include_str!("../../../db/biz_apm/migrations/V20260220000003__new_biz_apm.sql"),
    // )
    // .await;
    //
    // execute_migration_file(
    //     pool,
    //     "V20260220000004__funcs",
    //     include_str!("../../../db/biz_apm/migrations/V20260220000004__funcs.sql"),
    // )
    // .await;
    //
    // execute_migration_file(
    //     pool,
    //     "V20260221145531__dev_funcs",
    //     include_str!("../../../db/biz_apm/migrations/V20260221145531__dev_funcs.sql"),
    // )
    // .await;

    tracing::info!("All migrations completed successfully");
}

async fn run_seeds(_pool: &PgPool) {
    // Canonical integration tests own their fixtures explicitly. Starting from
    // an empty post-migration database avoids hidden cross-test coupling and,
    // critically, prevents retired Reservation rows from re-entering the
    // post-cutover runtime model through shared legacy seeds.
    tracing::info!("Canonical integration database starts without shared legacy seeds");
}

async fn execute_migration_file(pool: &PgPool, name: &str, sql: &str) {
    // Execute migration using raw_sql which handles multiple statements
    // Use a scoped block to ensure connection is released
    let result = {
        let mut conn = pool.acquire().await.expect("Failed to acquire connection");
        sqlx::raw_sql(sql).execute(&mut *conn).await
    }; // Connection is dropped and released here

    match result {
        Ok(_) => {
            tracing::info!("Migration {} completed", name);
        }
        Err(e) => {
            let err_msg = e.to_string();
            // Ignore errors for idempotency and known issues
            let is_ignorable = err_msg.contains("already exists") 
                || err_msg.contains("duplicate") 
                || err_msg.contains("DuplicateObject")
                // Ignore "does not exist" errors from FK constraints to non-existent tables
                || (err_msg.contains("does not exist") && err_msg.contains("reservation_cancel"))
                || (err_msg.contains("column") && err_msg.contains("does not exist") && err_msg.contains("booking_id"));

            if is_ignorable {
                tracing::warn!("Migration {} had warnings (ignored): {}", name, err_msg);
            } else {
                tracing::error!("Migration {} failed: {}", name, e);
                panic!("Failed to execute migration {}: {}", name, e);
            }
        }
    }
}

pub async fn seed_test_data(_pool: &PgPool) {
    // Seed data is now included in run_migrations
    // This function is kept for backwards compatibility
}

pub async fn cleanup_test_data(_pool: &PgPool) {
    // Don't cleanup shared test data
    // Tests should be read-only or clean up their own specific changes
}

/// Test-only KMS that echoes plaintext on encrypt and panics on decrypt.
/// Use this when a service or repo requires a `Kms` impl but the test
/// path under exercise must NOT call `decrypt`. If `decrypt` IS called,
/// the panic message tells the test author that the wrong code path
/// was hit.
#[allow(dead_code)]
pub struct MockKms;

#[async_trait::async_trait]
impl consultation_rs::sys::crypto::kms::Kms for MockKms {
    async fn encrypt(
        &self,
        plaintext: &[u8],
        _key_name: &str,
    ) -> consultation_rs::sys::crypto::kms::KmsResult<Vec<u8>> {
        Ok(plaintext.to_vec())
    }

    async fn decrypt(
        &self,
        _ciphertext: &[u8],
        _key_name: &str,
    ) -> consultation_rs::sys::crypto::kms::KmsResult<Vec<u8>> {
        unimplemented!(
            "MockKms decrypt is a test stub; if this panics, a test code path unexpectedly called decrypt — use GcpKmsService for tests that need real decryption"
        )
    }
}
