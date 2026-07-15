#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HttpServerConfig {
    pub host: String,
    pub port: u16,
}
