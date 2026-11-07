use std::sync::Arc;

use q_core::{ChatProvider, ChatRequest, ChatResponse, QError, Result};

use crate::ChatService;

impl ChatService {
    pub async fn complete_with_fallback(
        &self,
        providers: &[Arc<dyn ChatProvider>],
        request: &ChatRequest,
    ) -> Result<ChatResponse> {
        let mut last_error = None;
        for provider in providers {
            match provider.complete(request).await {
                Ok(response) => return Ok(response),
                Err(error) => {
                    eprintln!("q-rs: provider {} failed: {}", provider.name(), error);
                    last_error = Some(error);
                }
            }
        }
        match last_error {
            Some(_) => Err(QError::AllProvidersFailed),
            None => Err(QError::NoProvider(request.model.clone())),
        }
    }

    pub fn candidates(&self, model: &str) -> Vec<Arc<dyn ChatProvider>> {
        let mut providers = self.registry.providers();
        if let Some((prefix, _)) = model.split_once('/') {
            providers.retain(|p| p.name() == prefix);
        } else if model == "default" {
            if let Some(default) = &self.default_provider {
                providers.sort_by_key(|p| if p.name() == default { 0 } else { 1 });
            }
        } else {
            let matching: Vec<_> = providers
                .iter()
                .filter(|p| p.name() == model)
                .cloned()
                .collect();
            if !matching.is_empty() {
                return matching;
            }
        }
        providers
    }
}
