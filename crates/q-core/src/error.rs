use thiserror::Error;

pub type Result<T> = std::result::Result<T, QError>;

#[derive(Debug, Error)]
pub enum QError {
    #[error("requisição inválida: {0}")]
    InvalidRequest(String),
    #[error("provedor '{provider}' falhou: {message}")]
    Provider { provider: String, message: String },
    #[error("todos os provedores elegíveis falharam")]
    AllProvidersFailed,
    #[error("nenhum provedor está configurado para o modelo '{0}'")]
    NoProvider(String),
    #[error("operação não suportada: {0}")]
    UnsupportedOperation(String),
    #[error("ferramenta '{0}' não encontrada")]
    ToolNotFound(String),
    #[error("o modelo excedeu o limite de {0} rodadas de ferramentas")]
    MaxToolRounds(usize),
    #[error("skill '{0}' não encontrada")]
    SkillNotFound(String),
    #[error("configuração inválida: {0}")]
    Configuration(String),
}
