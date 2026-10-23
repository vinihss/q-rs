use q_application::ChatService;
use q_core::ChatProvider;
use q_core::{ChatRequest, Message};
use q_providers::{DemoProvider, OpenAiCompatibleProvider};
use q_tools::{CalculatorTool, ToolRegistry};
use std::{
    env,
    io::{self, Write},
    sync::Arc,
};

fn env_or(key: &str, fallback: &str) -> String {
    env::var(key).unwrap_or_else(|_| fallback.to_owned())
}
fn build_chat() -> ChatService {
    let mut providers: Vec<Arc<dyn ChatProvider>> = vec![Arc::new(DemoProvider)];
    if let Ok(key) = env::var("GROQ_API_KEY") {
        if !key.trim().is_empty() {
            providers.push(Arc::new(OpenAiCompatibleProvider::new(
                "groq",
                env_or("GROQ_BASE_URL", "https://api.groq.com/openai/v1/"),
                key,
                env_or("GROQ_MODEL", "openai/gpt-oss-20b"),
            )));
        }
    }
    if let Ok(key) = env::var("OPENROUTER_API_KEY") {
        if !key.trim().is_empty() {
            providers.push(Arc::new(OpenAiCompatibleProvider::new(
                "openrouter",
                env_or("OPENROUTER_BASE_URL", "https://openrouter.ai/api/v1"),
                key,
                env_or("OPENROUTER_MODEL", "openrouter/recursive/flint-v1.0:freeb"),
            )));
        }
    }
    
    let mut tools = ToolRegistry::new();
    tools
        .register(CalculatorTool)
        .expect("calculator tool registration must succeed");
    ChatService::new(providers)
        .with_default_provider(env_or("Q_RS_DEFAULT_PROVIDER", "demo"))
        .with_tools(Arc::new(tools))
        .with_max_tool_rounds(8)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let chat = Arc::new(build_chat());
    let mode = env_or("Q_RS_MODE", "cli");
    if mode == "server" {
        let host = env_or("Q_RS_HOST", "127.0.0.1");
        let port: u16 = env_or("Q_RS_PORT", "8080").parse()?;
        q_server::run(chat, &host, port).await?;
        return Ok(());
    }
    println!("q-rs — modo CLI (provedor demo disponível; Ctrl+D para sair)");
    loop {
        print!("you> ");
        io::stdout().flush()?;
        let mut input = String::new();
        if io::stdin().read_line(&mut input)? == 0 {
            break;
        }
        let input = input.trim();
        if input.is_empty() {
            continue;
        }
        if input == "/quit" || input == "/exit" {
            break;
        }
        let model = env_or("Q_RS_DEFAULT_MODEL", "default");
        let request = ChatRequest::new(model, vec![Message::user(input)]);
        match chat.complete(&request).await {
            Ok(answer) => println!("assistant> {}", answer.content),
            Err(error) => eprintln!("erro: {error}"),
        }
    }
    Ok(())
}
