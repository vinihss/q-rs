use q_core::{ChatProvider, ProviderRegistry};
use std::sync::Arc;

#[derive(Clone, Default)]
pub struct StaticProviderRegistry {
    providers: Vec<Arc<dyn ChatProvider>>,
}
impl StaticProviderRegistry {
    pub fn new(providers: Vec<Arc<dyn ChatProvider>>) -> Self {
        Self { providers }
    }
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }
}
impl ProviderRegistry for StaticProviderRegistry {
    fn providers(&self) -> Vec<Arc<dyn ChatProvider>> {
        self.providers.clone()
    }
}
