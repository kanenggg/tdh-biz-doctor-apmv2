use sqlx::postgres::PgPoolOptions;
use std::env;
use std::path::Path;
use std::fs;
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();

    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost:5432/biz_apm".to_string());

    println!("Connecting to database...");
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await?;

    println!("Connected successfully!");

    let seeds_dir = Path::new(file!()).parent().expect("Failed to get seeds directory");

    let mut entries: Vec<_> = fs::read_dir(seeds_dir)?
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.path().extension().map_or(false, |ext| ext == "sql")
                && entry.file_name() != "run_seeds.rs"
        })
        .collect();
    entries.sort_by_key(|entry| entry.path());

    if entries.is_empty() {
        println!("No seed files found in {}", seeds_dir.display());
        return Ok(());
    }

    let mut transaction = pool.begin().await?;
    for entry in entries {
        let path = entry.path();
        let file_name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        println!("Executing seed file: {}", file_name);

        let seed_sql = fs::read_to_string(&path)?;
        sqlx::raw_sql(&seed_sql).execute(&mut *transaction).await?;

        println!("✓ Completed: {}", file_name);
    }

    transaction.commit().await?;
    println!("\nSeeds executed successfully!");

    Ok(())
}
