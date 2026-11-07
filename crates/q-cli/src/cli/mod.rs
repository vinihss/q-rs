use std::env;

pub const USAGE: &str = r#"q-rs — assistente de chat

Uso:
  q-rs                                   modo interativo (REPL)
  q-rs run "<texto>"                     executa e imprime a resposta
  q-rs tool run <nome> <json>            executa uma tool diretamente (sem LLM)
  q-rs "<texto>"                         atalho para `run`
  q-rs server run [--host H] [--port P]  servidor em foreground
  q-rs server start [--host H] [--port P]  sobe o servidor em background (daemon)
  q-rs server stop                       para o servidor em background
  q-rs server status                     estado do servidor
  q-rs server install [--port P] [--host H]  grava config e unit systemd do usuário
  q-rs --help                            esta ajuda

Opções globais:
  -p, --prompt <texto>    system prompt desta execução (vence Q_RS_SYSTEM_PROMPT)
      --model <modelo>    modelo padrão desta execução
      --provider <nome>   provedor padrão desta execução
      --host <host>       host do servidor (padrão 0.0.0.0)
      --port <porta>      porta do servidor (padrão 8787)

Configuração:
  ${Q_RS_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/q-rs}/env
      GROQ_API_KEY=... / OPENROUTER_API_KEY=...
  ${Q_RS_CONFIG_DIR:-$HOME/.config/q-rs}/config
      Q_RS_HOST=0.0.0.0 / Q_RS_PORT=8787

Comandos do REPL: /system <texto>, /system, /quit, /exit
"#;

#[derive(Debug)]
pub enum Action {
    Repl,
    Run(String),
    ServerRun,
    ServerStart,
    ServerStop,
    ServerStatus,
    ServerInstall,
    ToolRun { name: String, args: String },
    Help,
}

#[derive(Debug)]
pub struct Cli {
    pub command: Action,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub prompt: Option<String>,
    pub model: Option<String>,
    pub provider: Option<String>,
}

pub fn usage_error(message: &str) -> ! {
    eprintln!("erro: {message}");
    eprintln!();
    eprint!("{USAGE}");
    std::process::exit(2);
}

pub fn parse_cli() -> Cli {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut cli = Cli {
        command: Action::Repl,
        host: None,
        port: None,
        prompt: None,
        model: None,
        provider: None,
    };
    let mut positionals: Vec<String> = Vec::new();
    let mut index = 0;

    while index < args.len() {
        let arg = args[index].as_str();
        let (flag, inline) = match arg.split_once('=') {
            Some((flag, value)) if flag.starts_with("--") => (flag, Some(value.to_string())),
            _ => (arg, None),
        };
        match flag {
            "-p" | "--prompt" | "--model" | "--provider" | "--host" | "--port" => {
                let value = match inline {
                    Some(value) => value,
                    None => {
                        index += 1;
                        match args.get(index) {
                            Some(value) => value.clone(),
                            None => usage_error(&format!("{flag} exige um valor")),
                        }
                    }
                };
                match flag {
                    "-p" | "--prompt" => cli.prompt = Some(value),
                    "--model" => cli.model = Some(value),
                    "--provider" => cli.provider = Some(value),
                    "--host" => cli.host = Some(value),
                    "--port" => cli.port = Some(parse_port(&value)),
                    _ => unreachable!(),
                }
            }
            "-h" | "--help" | "help" => cli.command = Action::Help,
            other if other.starts_with('-') && other != "-" => {
                usage_error(&format!("opção desconhecida: {other}"))
            }
            other => positionals.push(other.to_string()),
        }
        index += 1;
    }

    if matches!(cli.command, Action::Help) {
        return cli;
    }

    match positionals.first().map(String::as_str) {
        None => {
            if env::var("Q_RS_MODE")
                .map(|m| m == "server")
                .unwrap_or(false)
            {
                cli.command = Action::ServerRun;
            }
        }
        Some("run") => {
            let text = positionals[1..].join(" ");
            if text.trim().is_empty() {
                usage_error("`run` exige um texto: q-rs run \"<texto>\"");
            }
            cli.command = Action::Run(text);
        }
        Some("tool") => {
            match positionals.get(1).map(String::as_str) {
                Some("run") => {
                    let name = positionals.get(2).cloned().unwrap_or_default();
                    let args = positionals[3..].join(" ");
                    if name.is_empty() {
                        usage_error("use: q-rs tool run <nome> <json>");
                    }
                    cli.command = Action::ToolRun { name, args: if args.trim().is_empty() { "{}".to_string() } else { args } };
                }
                Some(other) => usage_error(&format!("subcomando de tool desconhecido: {other}")),
                None => usage_error("use: q-rs tool run <nome> <json>"),
            }
        }
        Some("server") => {
            cli.command = match positionals.get(1).map(String::as_str) {
                Some("run") => Action::ServerRun,
                Some("start") => Action::ServerStart,
                Some("stop") => Action::ServerStop,
                Some("status") => Action::ServerStatus,
                Some("install") => Action::ServerInstall,
                Some(other) => usage_error(&format!("subcomando de server desconhecido: {other}")),
                None => usage_error("use: q-rs server <run|start|stop|status|install>"),
            };
            if positionals.len() > 2 {
                usage_error(&format!(
                    "argumento inesperado: {}",
                    positionals[2..].join(" ")
                ));
            }
        }
        Some(_) => cli.command = Action::Run(positionals.join(" ")),
    }
    cli
}

pub fn parse_port(value: &str) -> u16 {
    match value.trim().parse::<u16>() {
        Ok(port) if port != 0 => port,
        _ => usage_error(&format!("porta inválida: {value}")),
    }
}
