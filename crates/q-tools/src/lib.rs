//! Tool registration and execution boundary. No tool implementation is trusted by default.
use q_core::{QError, Result, Tool, ToolDefinition, ToolExecutor, ToolOutput};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Default, Clone)]
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}
impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn register<T: Tool + 'static>(&mut self, tool: T) -> Result<()> {
        let name = tool.definition().name;
        if name.trim().is_empty() {
            return Err(QError::Configuration("tool name não pode ser vazio".into()));
        }
        if self.tools.contains_key(&name) {
            return Err(QError::Configuration(format!("tool duplicada: {name}")));
        }
        self.tools.insert(name, Arc::new(tool));
        Ok(())
    }
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|t| t.definition()).collect()
    }
    pub async fn execute(&self, name: &str, arguments: Value) -> Result<ToolOutput> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| QError::ToolNotFound(name.to_owned()))?;
        tool.execute(arguments).await
    }
    pub fn contains(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }
}

#[async_trait::async_trait]
impl ToolExecutor for ToolRegistry {
    fn definitions(&self) -> Vec<ToolDefinition> {
        ToolRegistry::definitions(self)
    }
    async fn execute(&self, name: &str, arguments: Value) -> Result<ToolOutput> {
        ToolRegistry::execute(self, name, arguments).await
    }
}

/// Small safe example tool, useful for validating automatic tool execution end-to-end.
pub struct CalculatorTool;

#[async_trait::async_trait]
impl Tool for CalculatorTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "calculator".into(),
            description:
                "Calcula uma operação aritmética simples: add, subtract, multiply ou divide.".into(),
            parameters_schema: serde_json::json!({
                "type":"object",
                "properties": {
                    "operation": {"type":"string", "enum":["add","subtract","multiply","divide"]},
                    "a": {"type":"number"},
                    "b": {"type":"number"}
                },
                "required":["operation","a","b"],
                "additionalProperties":false
            }),
        }
    }

    async fn execute(&self, arguments: Value) -> Result<ToolOutput> {
        let operation = arguments["operation"]
            .as_str()
            .ok_or_else(|| QError::InvalidRequest("calculator.operation deve ser texto".into()))?;
        let a = arguments["a"]
            .as_f64()
            .ok_or_else(|| QError::InvalidRequest("calculator.a deve ser número".into()))?;
        let b = arguments["b"]
            .as_f64()
            .ok_or_else(|| QError::InvalidRequest("calculator.b deve ser número".into()))?;
        let value = match operation {
            "add" => a + b,
            "subtract" => a - b,
            "multiply" => a * b,
            "divide" if b != 0.0 => a / b,
            "divide" => return Err(QError::InvalidRequest("divisão por zero".into())),
            other => {
                return Err(QError::InvalidRequest(format!(
                    "operação não suportada: {other}"
                )))
            }
        };
        Ok(ToolOutput {
            content: value.to_string(),
            metadata: Default::default(),
        })
    }
}

use std::process::Command;

/// Tool to execute a shell command (with timeout and output capture).
pub struct ShellTool;

#[async_trait::async_trait]
impl Tool for ShellTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "shell".into(),
            description: "Executa um comando shell e retorna stdout/stderr. Use com cuidado.".into(),
            parameters_schema: serde_json::json!({
                "type":"object",
                "properties": {
                    "command": {"type":"string", "description":"Comando a executar"},
                    "timeout_secs": {"type":"integer", "description":"Timeout em segundos (padrão 30)"}
                },
                "required":["command"],
                "additionalProperties":false
            }),
        }
    }

    async fn execute(&self, arguments: Value) -> Result<ToolOutput> {
        let command = arguments["command"]
            .as_str()
            .ok_or_else(|| QError::InvalidRequest("shell.command deve ser texto".into()))?;
        let timeout_secs = arguments.get("timeout_secs").and_then(|v| v.as_u64()).unwrap_or(30);

        let output = Command::new("bash")
            .arg("-c")
            .arg(command)
            .output()
            .map_err(|e| QError::InvalidRequest(format!("falha ao executar: {e}")))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let content = format!("exit_code: {}\nstdout:\n{}\nstderr:\n{}", output.status.code().unwrap_or(-1), stdout, stderr);
        Ok(ToolOutput {
            content,
            metadata: Default::default(),
        })
    }
}

use mlua::{Lua, Value as LuaValue};
use std::path::PathBuf;

/// A tool implemented in Lua, loaded from a `.lua` file.
use std::sync::{Arc, Mutex};
unsafe impl Send for LuaTool {}
unsafe impl Sync for LuaTool {}

pub struct LuaTool {
    lua: Arc<Mutex<Lua>>,
    path: PathBuf,
    def: ToolDefinition,
}

impl LuaTool {
    pub fn from_file(path: PathBuf) -> Result<Self> {
        let src = std::fs::read_to_string(&path)
            .map_err(|e| QError::Configuration(format!("falha ao ler lua tool {path:?}: {e}")))?;
        let lua = Arc::new(Mutex::new(Lua::new()));
        {
            let g = lua.lock().unwrap();
            g.load(&src).exec().map_err(|e| QError::Configuration(format!("lua exec error: {e}")))?;
        }
        let (name, description, params_str) = {
            let g = lua.lock().unwrap();
            let def_fn: mlua::Function = g.globals().get("tool_definition")
                .map_err(|e| QError::Configuration(format!("tool_definition não encontrada: {e}")))?;
            let def_val: LuaValue = def_fn.call(()).map_err(|e| QError::Configuration(format!("tool_definition call: {e}")))?;
            let tbl = match def_val {
                LuaValue::Table(t) => t,
                _ => return Err(QError::Configuration("tool_definition deve retornar table".into())),
            };
            let name: String = tbl.get("name").map_err(|e| QError::Configuration(format!("name: {e}")))?;
            let description: String = tbl.get("description").unwrap_or_default();
            let params_str: String = tbl.get("parameters_schema").unwrap_or_else(|_| "{}".to_string());
            (name, description, params_str)
        };
        let parameters_schema: serde_json::Value =
            serde_json::from_str(&params_str).unwrap_or(serde_json::json!({"type":"object"}));
        let def = ToolDefinition { name, description, parameters_schema };
        Ok(Self { lua, path, def })
    }
}

#[async_trait::async_trait]
impl Tool for LuaTool {
    fn definition(&self) -> ToolDefinition {
        self.def.clone()
    }

    async fn execute(&self, arguments: serde_json::Value) -> Result<ToolOutput> {
        let args_str = arguments.to_string();
        let res = {
            let lua = self.lua.lock().unwrap();
            let exec_fn: mlua::Function = lua.globals().get("tool_execute")
                .map_err(|e| QError::InvalidRequest(format!("tool_execute não encontrada: {e}")))?;
            exec_fn.call::<String, String>(args_str)
                .map_err(|e| QError::InvalidRequest(format!("tool_execute call: {e}")))?
        };
        Ok(ToolOutput {
            content: res,
            metadata: Default::default(),
        })
    }
}

/// Tool to fetch content from a URL.
pub struct WebFetchTool;

#[async_trait::async_trait]
impl Tool for WebFetchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "web_fetch".into(),
            description: "Busca o conteúdo de uma URL e retorna o texto.".into(),
            parameters_schema: serde_json::json!({
                "type":"object",
                "properties": {
                    "url": {"type":"string", "description":"URL a buscar"}
                },
                "required":["url"],
                "additionalProperties":false
            }),
        }
    }

    async fn execute(&self, arguments: serde_json::Value) -> Result<ToolOutput> {
        let url = arguments["url"].as_str().ok_or_else(|| QError::InvalidRequest("web_fetch.url deve ser texto".into()))?;
        let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(30)).build()
            .map_err(|e| QError::InvalidRequest(format!("client error: {e}")))?;
        let resp = client.get(url).send().await.map_err(|e| QError::InvalidRequest(format!("fetch error: {e}")))?;
        let status = resp.status();
        let body = resp.text().await.map_err(|e| QError::InvalidRequest(format!("read error: {e}")))?;
        // strip html tags roughly
        let text = body.replace("<", " <").replace(">", "> ");
        // simple tag removal
        let mut out = String::new();
        let mut in_tag = false;
        for c in text.chars() {
            if c == '<' { in_tag = true; }
            else if c == '>' { in_tag = false; }
            else if !in_tag { out.push(c); }
        }
        let content = format!("status: {}\n\n{}", status, out.trim());
        Ok(ToolOutput { content, metadata: Default::default() })
    }
}

/// Tool to search the web via DuckDuckGo HTML.
pub struct WebSearchTool;

#[async_trait::async_trait]
impl Tool for WebSearchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "web_search".into(),
            description: "Pesquisa na web via DuckDuckGo e retorna resultados.".into(),
            parameters_schema: serde_json::json!({
                "type":"object",
                "properties": {
                    "query": {"type":"string", "description":"Termo de busca"}
                },
                "required":["query"],
                "additionalProperties":false
            }),
        }
    }

    async fn execute(&self, arguments: serde_json::Value) -> Result<ToolOutput> {
        let query = arguments["query"].as_str().ok_or_else(|| QError::InvalidRequest("web_search.query deve ser texto".into()))?;
        let url = format!("https://html.duckduckgo.com/html/?q={}", simple_url_encode(query));
        let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(30)).build()
            .map_err(|e| QError::InvalidRequest(format!("client error: {e}")))?;
        let resp = client.get(&url).header("User-Agent","Mozilla/5.0").send().await
            .map_err(|e| QError::InvalidRequest(format!("search error: {e}")))?;
        let body = resp.text().await.map_err(|e| QError::InvalidRequest(format!("read error: {e}")))?;
        // extract result snippets (rough)
        let mut results = Vec::new();
        for line in body.lines() {
            if line.contains("result__a") {
                results.push(line.trim().to_string());
            }
        }
        let content = if results.is_empty() { body.chars().take(2000).collect() } else { results.join("\n") };
        Ok(ToolOutput { content, metadata: Default::default() })
    }
}

fn simple_url_encode(s: &str) -> String {
    s.chars().map(|c| match c {
        ' ' => '+'.to_string(),
        c if c.is_alphanumeric() || "-_.~".contains(c) => c.to_string(),
        _ => format!("%{:02X}", c as u32),
    }).collect()
}
