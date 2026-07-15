use anyhow::Result;
use clap::{Parser, Subcommand};
use std::fs;
use std::path::PathBuf;
use utoipa::OpenApi;

#[derive(Parser)]
#[command(name = "openapi")]
#[command(about = "Generate OpenAPI specifications for services")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Generate {
        #[arg(short, long)]
        module: String,
        #[arg(short, long, default_value = "./specs/provides")]
        output_dir: PathBuf,
    },
    GenerateAll {
        #[arg(short, long, default_value = "./specs/provides")]
        output_dir: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Generate { module, output_dir } => {
            generate_openapi(&module, &output_dir)?;
        }
        Commands::GenerateAll { output_dir } => {
            let modules = vec!["consultation-rs"];
            for module in modules {
                if let Err(e) = generate_openapi(module, &output_dir) {
                    eprintln!("Error generating OpenAPI for {}: {}", module, e);
                }
            }
        }
    }

    Ok(())
}

fn generate_openapi(module: &str, output_dir: &PathBuf) -> Result<()> {
    println!("Generating OpenAPI specification for {}...", module);

    fs::create_dir_all(output_dir)?;

    let openapi = match module {
        "consultation-rs" => {
            use consultation_rs::openapi::ApiDoc;
            serde_json::to_value(&ApiDoc::openapi())
                .map_err(|e| anyhow::anyhow!("Failed to serialize OpenAPI spec: {}", e))?
        }
        _ => return Err(anyhow::anyhow!("Unsupported module: {}", module)),
    };

    // Write JSON
    let json_file = output_dir.join(format!("{}.json", module));
    let json_formatted = serde_json::to_string_pretty(&openapi)
        .map_err(|e| anyhow::anyhow!("Failed to format OpenAPI JSON: {}", e))?;
    fs::write(&json_file, &json_formatted)
        .map_err(|e| anyhow::anyhow!("Failed to write OpenAPI JSON: {}", e))?;
    println!("  JSON: {}", json_file.display());

    // Write YAML
    let yaml_file = output_dir.join(format!("{}.yaml", module));
    let yaml_formatted = serde_yaml::to_string(&openapi)
        .map_err(|e| anyhow::anyhow!("Failed to format OpenAPI YAML: {}", e))?;
    fs::write(&yaml_file, &yaml_formatted)
        .map_err(|e| anyhow::anyhow!("Failed to write OpenAPI YAML: {}", e))?;
    println!("  YAML: {}", yaml_file.display());

    Ok(())
}
