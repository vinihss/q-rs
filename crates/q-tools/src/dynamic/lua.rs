use async_trait::async_trait;
use mlua::{Lua, Value as LuaValue};
use q_core::{QError, Result, Tool, ToolDefinition, ToolOutput};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// A tool implemented in Lua, loaded from a `.lua` file.
pub struct LuaTool {
    lua: Arc<Mutex<Lua>>,
    path: PathBuf,
    def: ToolDefinition,
}

// Safety: Lua is not Send/Sync by default; we guard access with a Mutex.
unsafe impl Send for LuaTool {}
unsafe impl Sync for LuaTool {}

impl LuaTool {
    pub fn from_file(path: PathBuf) -> Result<Self> {
        let src = std::fs::read_to_string(&path)
            .map_err(|e| QError::Configuration(format!("falha ao ler lua tool {path:?}: {e}")))?;
        let lua = Arc::new(Mutex::new(Lua::new()));
        {
            let g = lua.lock().unwrap();
            g.load(&src)
                .exec()
                .map_err(|e| QError::Configuration(format!("lua exec error: {e}")))?;
        }
        let (name, description, params_str) = {
            let g = lua.lock().unwrap();
            let def_fn: mlua::Function = g.globals().get("tool_definition").map_err(|e| {
                QError::Configuration(format!("tool_definition não encontrada: {e}"))
            })?;
            let def_val: LuaValue = def_fn
                .call(())
                .map_err(|e| QError::Configuration(format!("tool_definition call: {e}")))?;
            let tbl = match def_val {
                LuaValue::Table(t) => t,
                _ => {
                    return Err(QError::Configuration(
                        "tool_definition deve retornar table".into(),
                    ))
                }
            };
            let name: String = tbl
                .get("name")
                .map_err(|e| QError::Configuration(format!("name: {e}")))?;
            let description: String = tbl.get("description").unwrap_or_default();
            let params_str: String = tbl
                .get("parameters_schema")
                .unwrap_or_else(|_| "{}".to_string());
            (name, description, params_str)
        };
        let parameters_schema: serde_json::Value =
            serde_json::from_str(&params_str).unwrap_or(serde_json::json!({"type":"object"}));
        let def = ToolDefinition {
            name,
            description,
            parameters_schema,
        };
        Ok(Self { lua, path, def })
    }
}

#[async_trait]
impl Tool for LuaTool {
    fn definition(&self) -> ToolDefinition {
        self.def.clone()
    }

    async fn execute(&self, arguments: serde_json::Value) -> Result<ToolOutput> {
        let args_str = arguments.to_string();
        let res = {
            let lua = self.lua.lock().unwrap();
            let exec_fn: mlua::Function = lua
                .globals()
                .get("tool_execute")
                .map_err(|e| QError::InvalidRequest(format!("tool_execute não encontrada: {e}")))?;
            exec_fn
                .call::<String, String>(args_str)
                .map_err(|e| QError::InvalidRequest(format!("tool_execute call: {e}")))?
        };
        Ok(ToolOutput {
            content: res,
            metadata: Default::default(),
        })
    }
}
