use crate::config;
use q_application::ChatService;
use q_core::{ChatProvider, ChatRequest, Message, Tool};
use q_providers::{DemoProvider, OpenAiCompatibleProvider};
use q_tools::{CalculatorTool, LuaTool, ShellTool, ToolRegistry};
use std::{
    env, fs,
    io::{self, Write},
    path::PathBuf,
    sync::Arc,
};

const DEFAULT_MODEL: &str = "default";

pub fn build_chat(default_provider: &str) -> ChatService {
    let mut providers: Vec<Arc<dyn ChatProvider>> = vec![Arc::new(DemoProvider)];
    if let Some(key) = config::env_non_empty("GROQ_API_KEY") {
        providers.push(Arc::new(OpenAiCompatibleProvider::new(
            "groq",
            config::env_or("GROQ_BASE_URL", "https://api.groq.com/openai/v1/"),
            key,
            config::env_or("GROQ_MODEL", "openai/gpt-oss-20b"),
        )));
    }
    if let Some(key) = config::env_non_empty("OPENROUTER_API_KEY") {
        providers.push(Arc::new(OpenAiCompatibleProvider::new(
            "openrouter",
            config::env_or("OPENROUTER_BASE_URL", "https://openrouter.ai/api/v1"),
            key,
            config::env_or("OPENROUTER_MODEL", "openrouter/recursive/flint-v1.0:freeb"),
        )));
    }

    let mut tools = ToolRegistry::new();
    tools
        .register(CalculatorTool)
        .expect("calculator tool registration must succeed");
    tools
        .register(ShellTool)
        .expect("shell tool registration must succeed");

    // Load dynamic Lua tools from ~/.config/q-rs/tools/*.lua
    let tools_dir = env::var("HOME")
        .map(|h| PathBuf::from(h).join(".config/q-rs/tools"))
        .unwrap_or_default();
    if tools_dir.exists() {
        if let Ok(entries) = fs::read_dir(&tools_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("lua") {
                    match LuaTool::from_file(path.clone()) {
                        Ok(tool) => {
                            let name = tool.definition().name.clone();
                            if tools.register(tool).is_ok() {
                                eprintln!("loaded lua tool: {name}");
                            }
                        }
                        Err(e) => eprintln!("failed to load lua tool {path:?}: {e}"),
                    }
                }
            }
        }
    }
    ChatService::new(providers)
        .with_default_provider(default_provider)
        .with_default_model(
            config::env_non_empty("Q_RS_DEFAULT_MODEL").unwrap_or_else(|| "default".to_string()),
        )
        .with_tools(Arc::new(tools))
        .with_max_tool_rounds(8)
}

pub fn build_request(input: &str, cli: &crate::cli::Cli) -> ChatRequest {
    let model = cli
        .model
        .clone()
        .or_else(|| config::env_non_empty("Q_RS_DEFAULT_MODEL"))
        .unwrap_or_else(|| DEFAULT_MODEL.to_string());
    let mut request = ChatRequest::new(model, vec![Message::user(input)]);
    let system_prompt = cli
        .prompt
        .clone()
        .or_else(|| config::env_non_empty("Q_RS_SYSTEM_PROMPT"));
    request.system_prompt = system_prompt;
    request
}

pub async fn ask(chat: &ChatService, input: &str, cli: &crate::cli::Cli) -> Result<String, String> {
    chat.complete(&build_request(input, cli))
        .await
        .map(|answer| answer.content)
        .map_err(|error| error.to_string())
}

pub async fn run_once(chat: &ChatService, input: &str, cli: &crate::cli::Cli) -> ! {
    match ask(chat, input, cli).await {
        Ok(answer) => {
            println!("{answer}");
            std::process::exit(0);
        }
        Err(error) => {
            eprintln!("erro: {error}");
            std::process::exit(1);
        }
    }
}

pub async fn run_repl(chat: &ChatService, cli: &crate::cli::Cli) {
    println!("q-rs — modo interativo (Ctrl+D ou /quit para sair)");
    loop {
        print!("you> ");
        if io::stdout().flush().is_err() {
            break;
        }
        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(0) => break,
            Ok(_) => {}
            Err(error) => {
                eprintln!("erro lendo entrada: {error}");
                break;
            }
        }
        let input = input.trim();
        if input.is_empty() {
            continue;
        }
        if input == "/quit" || input == "/exit" {
            break;
        }
        if let Some(rest) = input.strip_prefix("/system ") {
            env::set_var("Q_RS_SYSTEM_PROMPT", rest.trim());
            println!("system prompt definido.");
            continue;
        }
        if input == "/system" {
            println!(
                "system prompt: {}",
                env::var("Q_RS_SYSTEM_PROMPT").unwrap_or_default()
            );
            continue;
        }
        match ask(chat, input, cli).await {
            Ok(answer) => println!("assistant> {answer}"),
            Err(error) => eprintln!("erro: {error}"),
        }
    }
}
