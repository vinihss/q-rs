//! Application orchestration: validation, provider selection and fallback.
mod chat;
mod registry;
pub use chat::ChatService;
pub use registry::StaticProviderRegistry;
