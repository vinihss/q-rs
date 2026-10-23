use crate::{ChatRequest, ChatResponse, QError, StreamEvent};
use async_trait::async_trait;
use futures_core::Stream;
use std::{pin::Pin, sync::Arc};

pub type ProviderStream = Pin<Box<dyn Stream<Item = Result<StreamEvent, QError>> + Send + 'static>>;

/// Port implemented by each LLM adapter. Names are stable identifiers (e.g. `groq`).
#[async_trait]
pub trait ChatProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn supports_model(&self, model: &str) -> bool {
        match model.split_once('/') {
            Some((provider, _)) => provider == self.name(),
            None => true,
        }
    }
    async fn complete(&self, request: &ChatRequest) -> crate::Result<ChatResponse>;
    async fn stream(&self, _request: &ChatRequest) -> crate::Result<ProviderStream> {
        Err(QError::UnsupportedOperation(format!(
            "streaming não implementado pelo provedor {}",
            self.name()
        )))
    }
}

/// Read-only registry contract; implementations own provider ordering and lookup policy.
pub trait ProviderRegistry: Send + Sync {
    fn providers(&self) -> Vec<Arc<dyn ChatProvider>>;
    fn get(&self, name: &str) -> Option<Arc<dyn ChatProvider>> {
        self.providers().into_iter().find(|p| p.name() == name)
    }
}
