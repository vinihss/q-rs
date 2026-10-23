//! Stable domain contracts for q-rs. This crate must not depend on HTTP frameworks
//! or concrete provider SDKs.
pub mod config;
pub mod error;
pub mod message;
pub mod provider;
pub mod skill;
pub mod stream;
pub mod tool;

pub use config::{AppConfig, ProviderConfig, ServerConfig};
pub use error::{QError, Result};
pub use message::{ChatRequest, ChatResponse, Message, Role, Usage};
pub use provider::{ChatProvider, ProviderRegistry, ProviderStream};
pub use skill::{Skill, SkillContext, SkillDefinition, SkillResult};
pub use stream::StreamEvent;
pub use tool::{Tool, ToolCall, ToolDefinition, ToolExecutor, ToolOutput};
