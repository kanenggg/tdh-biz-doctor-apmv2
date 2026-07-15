use anyhow::Result;
use clap::{Parser, Subcommand};
use common_rs::twilio::TwilioAccessTokenBuilder;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use toml;

#[derive(Deserialize, Default)]
struct TwilioConfig {
    account_sid: Option<String>,
    api_key_sid: Option<String>,
    api_key_secret: Option<String>,
}

#[derive(Deserialize, Default)]
struct VideoConfig {
    room_name: Option<String>,
    identity: Option<String>,
    #[serde(flatten)]
    twilio: TwilioConfig,
}

#[derive(Deserialize, Default)]
struct ChatConfig {
    service_sid: Option<String>,
    identity: Option<String>,
    #[serde(flatten)]
    twilio: TwilioConfig,
}

#[derive(Deserialize, Default)]
struct VideoChatConfig {
    room_name: Option<String>,
    service_sid: Option<String>,
    identity: Option<String>,
    #[serde(flatten)]
    twilio: TwilioConfig,
}

fn load_video_config(path: &PathBuf) -> Result<VideoConfig> {
    let content = fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("Failed to read config file: {}", e))?;
    toml::from_str(&content).map_err(|e| anyhow::anyhow!("Failed to parse TOML: {}", e))
}

fn load_chat_config(path: &PathBuf) -> Result<ChatConfig> {
    let content = fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("Failed to read config file: {}", e))?;
    toml::from_str(&content).map_err(|e| anyhow::anyhow!("Failed to parse TOML: {}", e))
}

fn load_video_chat_config(path: &PathBuf) -> Result<VideoChatConfig> {
    let content = fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("Failed to read config file: {}", e))?;
    toml::from_str(&content).map_err(|e| anyhow::anyhow!("Failed to parse TOML: {}", e))
}

#[derive(Parser)]
#[command(name = "twilio")]
#[command(about = "Generate Twilio JWT tokens for video, chat, or combined access")]
struct Cli {
    #[arg(short = 'c', long, value_name = "FILE", global = true)]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Video {
        #[arg(short = 'A', long)]
        account_sid: Option<String>,
        #[arg(short = 'k', long)]
        api_key_sid: Option<String>,
        #[arg(short = 's', long)]
        api_key_secret: Option<String>,
        #[arg(short = 'r', long)]
        room_name: Option<String>,
        #[arg(short = 'i', long)]
        identity: Option<String>,
    },
    Chat {
        #[arg(short = 'A', long)]
        account_sid: Option<String>,
        #[arg(short = 'k', long)]
        api_key_sid: Option<String>,
        #[arg(short = 's', long)]
        api_key_secret: Option<String>,
        #[arg(short = 'S', long)]
        service_sid: Option<String>,
        #[arg(short = 'i', long)]
        identity: Option<String>,
    },
    VideoChat {
        #[arg(short = 'A', long)]
        account_sid: Option<String>,
        #[arg(short = 'k', long)]
        api_key_sid: Option<String>,
        #[arg(short = 's', long)]
        api_key_secret: Option<String>,
        #[arg(short = 'r', long)]
        room_name: Option<String>,
        #[arg(short = 'S', long)]
        service_sid: Option<String>,
        #[arg(short = 'i', long)]
        identity: Option<String>,
    },
}

fn resolve_value<T>(config_val: Option<T>, cli_val: Option<T>) -> Result<T>
where
    T: std::fmt::Debug,
{
    match (config_val, cli_val) {
        (Some(v), _) => Ok(v),
        (None, Some(v)) => Ok(v),
        (None, None) => Err(anyhow::anyhow!("Missing required value")),
    }
}

fn resolve_optional_value<T>(config_val: Option<T>, cli_val: Option<T>) -> Option<T>
where
    T: std::fmt::Debug,
{
    match (config_val, cli_val) {
        (Some(v), _) => Some(v),
        (None, v) => v,
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Video {
            account_sid,
            api_key_sid,
            api_key_secret,
            room_name,
            identity,
        } => {
            let config = if let Some(ref path) = cli.config {
                Some(load_video_config(path)?)
            } else {
                None
            };

            let account_sid = resolve_value(
                config.as_ref().and_then(|c| c.twilio.account_sid.clone()),
                account_sid,
            )?;
            let api_key_sid = resolve_value(
                config.as_ref().and_then(|c| c.twilio.api_key_sid.clone()),
                api_key_sid,
            )?;
            let api_key_secret = resolve_value(
                config
                    .as_ref()
                    .and_then(|c| c.twilio.api_key_secret.clone()),
                api_key_secret,
            )?;
            let room_name =
                resolve_value(config.as_ref().and_then(|c| c.room_name.clone()), room_name)?;
            let identity =
                resolve_value(config.as_ref().and_then(|c| c.identity.clone()), identity)?;

            let builder = TwilioAccessTokenBuilder::new(account_sid, api_key_sid, api_key_secret);
            let token = builder
                .build_video_token(&room_name, &identity)
                .map_err(|e| anyhow::anyhow!("Failed to build video token: {}", e))?;
            println!("{}", token);
        }
        Commands::Chat {
            account_sid,
            api_key_sid,
            api_key_secret,
            service_sid,
            identity,
        } => {
            let config = if let Some(ref path) = cli.config {
                Some(load_chat_config(path)?)
            } else {
                None
            };

            let account_sid = resolve_value(
                config.as_ref().and_then(|c| c.twilio.account_sid.clone()),
                account_sid,
            )?;
            let api_key_sid = resolve_value(
                config.as_ref().and_then(|c| c.twilio.api_key_sid.clone()),
                api_key_sid,
            )?;
            let api_key_secret = resolve_value(
                config
                    .as_ref()
                    .and_then(|c| c.twilio.api_key_secret.clone()),
                api_key_secret,
            )?;
            let service_sid = resolve_value(
                config.as_ref().and_then(|c| c.service_sid.clone()),
                service_sid,
            )?;
            let identity =
                resolve_value(config.as_ref().and_then(|c| c.identity.clone()), identity)?;

            let builder = TwilioAccessTokenBuilder::new(account_sid, api_key_sid, api_key_secret);
            let token = builder
                .build_chat_token(&service_sid, &identity)
                .map_err(|e| anyhow::anyhow!("Failed to build chat token: {}", e))?;
            println!("{}", token);
        }
        Commands::VideoChat {
            account_sid,
            api_key_sid,
            api_key_secret,
            room_name,
            service_sid,
            identity,
        } => {
            let config = if let Some(ref path) = cli.config {
                Some(load_video_chat_config(path)?)
            } else {
                None
            };

            let account_sid = resolve_value(
                config.as_ref().and_then(|c| c.twilio.account_sid.clone()),
                account_sid,
            )?;
            let api_key_sid = resolve_value(
                config.as_ref().and_then(|c| c.twilio.api_key_sid.clone()),
                api_key_sid,
            )?;
            let api_key_secret = resolve_value(
                config
                    .as_ref()
                    .and_then(|c| c.twilio.api_key_secret.clone()),
                api_key_secret,
            )?;
            let room_name =
                resolve_value(config.as_ref().and_then(|c| c.room_name.clone()), room_name)?;
            let service_sid = resolve_optional_value(
                config.as_ref().and_then(|c| c.service_sid.clone()),
                service_sid,
            );
            let identity =
                resolve_value(config.as_ref().and_then(|c| c.identity.clone()), identity)?;

            let builder = TwilioAccessTokenBuilder::new(account_sid, api_key_sid, api_key_secret);
            let token = builder
                .build_video_chat_token(&room_name, service_sid.as_deref(), &identity)
                .map_err(|e| anyhow::anyhow!("Failed to build video-chat token: {}", e))?;
            println!("{}", token);
        }
    }

    Ok(())
}
