use config::{Config, Environment, File};
use serde::de::DeserializeOwned;
use std::path::PathBuf;
use tracing::info;

/// Load config from multiple file paths, plus env vars with prefix `APP__`
///
/// If no paths provided, defaults to `config/default.toml` and `config/local.toml`
///
/// Each path's format is auto-detected by extension: .toml, .json, .yaml, .yml
/// Later files override earlier ones
pub fn load_conf_from_paths<T: DeserializeOwned>(paths: &[PathBuf]) -> anyhow::Result<T> {
    load_conf_from_paths_with_default_dirs(paths, &[PathBuf::from("./config")])
}

/// Load config from explicit paths, with default config directories tried first.
///
/// Each default directory contributes optional `default.toml` and `local.toml`.
/// Explicit paths are required and override default files.
pub fn load_conf_from_paths_with_default_dirs<T: DeserializeOwned>(
    paths: &[PathBuf],
    default_dirs: &[PathBuf],
) -> anyhow::Result<T> {
    let mut builder = Config::builder();

    for default_dir in default_dirs {
        builder = builder
            .add_source(File::from(default_dir.join("default.toml")).required(false))
            .add_source(File::from(default_dir.join("local.toml")).required(false));
    }

    if paths.is_empty() {
        let default_dirs = default_dirs
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        info!("No custom config paths provided, using default config dirs: {default_dirs}");
    } else {
        for path in paths {
            info!("Loading additional config from: {}", path.display());
            let path_str = path.to_str().unwrap();
            builder = builder.add_source(File::with_name(path_str).required(true));
        }
    }

    builder
        .add_source(Environment::with_prefix("APP").separator("__"))
        .build()?
        .try_deserialize()
        .map_err(|e| anyhow::anyhow!("Failed to load config: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use std::{
        fs,
        sync::{Mutex, OnceLock},
        time::{SystemTime, UNIX_EPOCH},
    };

    #[derive(Debug, Deserialize)]
    struct TestConfig {
        server: TestServerConfig,
    }

    #[derive(Debug, Deserialize)]
    struct TestServerConfig {
        host: String,
        port: u16,
    }

    fn cwd_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn loads_default_config_from_service_directory_when_root_config_is_missing() {
        let _guard = cwd_lock().lock().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("config-loader-test-{unique}"));
        let service_config_dir = root.join("consultation-rs").join("config");
        fs::create_dir_all(&service_config_dir).unwrap();
        fs::write(
            service_config_dir.join("default.toml"),
            r#"
                [server]
                host = "127.0.0.1"
                port = 8181
            "#,
        )
        .unwrap();

        std::env::set_current_dir(&root).unwrap();
        let result = load_conf_from_paths_with_default_dirs::<TestConfig>(
            &[],
            &[
                std::path::PathBuf::from("./config"),
                std::path::PathBuf::from("./consultation-rs/config"),
            ],
        );
        std::env::set_current_dir(original_dir).unwrap();

        let config = result.unwrap();
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 8181);
    }
}
