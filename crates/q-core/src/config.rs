#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}
impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 8080,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub name: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub default_model: String,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub mode: String,
    pub default_provider: String,
    pub server: ServerConfig,
    pub providers: Vec<ProviderConfig>,
}
impl Default for AppConfig {
    fn default() -> Self {
        Self {
            mode: "cli".into(),
            default_provider: "demo".into(),
            server: ServerConfig::default(),
            providers: vec![],
        }
    }
}
impl AppConfig {
    pub fn validate(&self) -> crate::Result<()> {
        if !matches!(self.mode.as_str(), "cli" | "server") {
            return Err(crate::QError::Configuration(
                "Q_RS_MODE deve ser cli ou server".into(),
            ));
        }
        if self.server.port == 0 {
            return Err(crate::QError::Configuration(
                "porta do servidor inválida".into(),
            ));
        }
        for p in &self.providers {
            if p.enabled && p.api_key.as_deref().unwrap_or_default().is_empty() {
                return Err(crate::QError::Configuration(format!(
                    "provedor '{}' habilitado sem API key",
                    p.name
                )));
            }
        }
        Ok(())
    }
}
