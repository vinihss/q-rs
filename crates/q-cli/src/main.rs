use q_application::ChatService;
use q_core::ChatProvider;
use q_core::{ChatRequest, Message};
use q_providers::{DemoProvider, OpenAiCompatibleProvider};
use q_tools::{CalculatorTool, ToolRegistry};
use std::{
    env, fs,
    io::{self, IsTerminal, Write},
    net::{TcpStream, ToSocketAddrs},
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
    thread::sleep,
    time::{Duration, Instant},
};

const DEFAULT_HOST: &str = "0.0.0.0";
const DEFAULT_PORT: u16 = 8787;
const DEFAULT_PROVIDER: &str = "demo";
const DEFAULT_MODEL: &str = "default";

const USAGE: &str = r#"q-rs — assistente de chat

Uso:
  q-rs                                   modo interativo (REPL)
  q-rs run "<texto>"                     executa e imprime a resposta
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

// ---------------------------------------------------------------------------
// Configuração em disco
// ---------------------------------------------------------------------------

fn config_dir() -> PathBuf {
    for var in ["Q_RS_CONFIG_DIR", "XDG_CONFIG_HOME"] {
        if let Some(value) = env::var_os(var) {
            if !value.is_empty() {
                let path = PathBuf::from(value);
                return if var == "XDG_CONFIG_HOME" {
                    path.join("q-rs")
                } else {
                    path
                };
            }
        }
    }
    match env::var_os("HOME") {
        Some(home) if !home.is_empty() => PathBuf::from(home).join(".config").join("q-rs"),
        _ => PathBuf::from(".config").join("q-rs"),
    }
}

fn pid_path() -> PathBuf {
    config_dir().join("server.pid")
}

fn log_path() -> PathBuf {
    config_dir().join("server.log")
}

fn config_path() -> PathBuf {
    config_dir().join("config")
}

fn env_path() -> PathBuf {
    config_dir().join("env")
}

/// Carrega `env` e `config` do diretório de configuração sem sobrescrever
/// variáveis já presentes no ambiente real (o ambiente sempre vence).
fn load_config_files() {
    let dir = config_dir();
    for name in ["env", "config"] {
        load_key_value_file(&dir.join(name));
    }
}

fn load_key_value_file(path: &Path) {
    let Ok(contents) = fs::read_to_string(path) else {
        return;
    };
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line).trim();
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        let value = unquote(value.trim());
        if env::var_os(key).is_none() {
            env::set_var(key, value);
        }
    }
}

fn unquote(value: &str) -> String {
    let bytes = value.as_bytes();
    if bytes.len() >= 2 {
        let first = bytes[0];
        let last = bytes[bytes.len() - 1];
        if (first == b'"' || first == b'\'') && first == last {
            return value[1..value.len() - 1].to_string();
        }
    }
    value.to_string()
}

// ---------------------------------------------------------------------------
// Argumentos
// ---------------------------------------------------------------------------

#[derive(Debug)]
enum Action {
    Repl,
    Run(String),
    ServerRun,
    ServerStart,
    ServerStop,
    ServerStatus,
    ServerInstall,
    Help,
}

#[derive(Debug)]
struct Cli {
    command: Action,
    host: Option<String>,
    port: Option<u16>,
    prompt: Option<String>,
    model: Option<String>,
    provider: Option<String>,
}

fn usage_error(message: &str) -> ! {
    eprintln!("erro: {message}");
    eprintln!();
    eprint!("{USAGE}");
    std::process::exit(2);
}

fn parse_cli() -> Cli {
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
            // Compatibilidade: modo servidor via variável de ambiente.
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

fn parse_port(value: &str) -> u16 {
    match value.trim().parse::<u16>() {
        Ok(port) if port != 0 => port,
        _ => usage_error(&format!("porta inválida: {value}")),
    }
}

// ---------------------------------------------------------------------------
// Chat
// ---------------------------------------------------------------------------

fn env_or(key: &str, fallback: &str) -> String {
    env::var(key).unwrap_or_else(|_| fallback.to_owned())
}

fn env_non_empty(key: &str) -> Option<String> {
    env::var(key).ok().filter(|value| !value.trim().is_empty())
}

fn build_chat(default_provider: &str) -> ChatService {
    let mut providers: Vec<Arc<dyn ChatProvider>> = vec![Arc::new(DemoProvider)];
    if let Some(key) = env_non_empty("GROQ_API_KEY") {
        providers.push(Arc::new(OpenAiCompatibleProvider::new(
            "groq",
            env_or("GROQ_BASE_URL", "https://api.groq.com/openai/v1/"),
            key,
            env_or("GROQ_MODEL", "openai/gpt-oss-20b"),
        )));
    }
    if let Some(key) = env_non_empty("OPENROUTER_API_KEY") {
        providers.push(Arc::new(OpenAiCompatibleProvider::new(
            "openrouter",
            env_or("OPENROUTER_BASE_URL", "https://openrouter.ai/api/v1"),
            key,
            env_or("OPENROUTER_MODEL", "openrouter/recursive/flint-v1.0:freeb"),
        )));
    }

    let mut tools = ToolRegistry::new();
    tools
        .register(CalculatorTool)
        .expect("calculator tool registration must succeed");
    ChatService::new(providers)
        .with_default_provider(default_provider)
        .with_tools(Arc::new(tools))
        .with_max_tool_rounds(8)
}

fn build_request(input: &str, cli: &Cli) -> ChatRequest {
    let model = cli
        .model
        .clone()
        .or_else(|| env_non_empty("Q_RS_DEFAULT_MODEL"))
        .unwrap_or_else(|| DEFAULT_MODEL.to_string());
    let mut request = ChatRequest::new(model, vec![Message::user(input)]);
    let system_prompt = cli
        .prompt
        .clone()
        .or_else(|| env_non_empty("Q_RS_SYSTEM_PROMPT"));
    request.system_prompt = system_prompt;
    request
}

async fn ask(chat: &ChatService, input: &str, cli: &Cli) -> Result<String, String> {
    chat.complete(&build_request(input, cli))
        .await
        .map(|answer| answer.content)
        .map_err(|error| error.to_string())
}

async fn run_once(chat: &ChatService, input: &str, cli: &Cli) -> ! {
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

async fn run_repl(chat: &ChatService, cli: &Cli) {
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

// ---------------------------------------------------------------------------
// Servidor
// ---------------------------------------------------------------------------

fn state_path() -> PathBuf {
    config_dir().join("server.state")
}

fn write_state(host: &str, port: u16) {
    let _ = fs::write(state_path(), format!("{host}\n{port}\n"));
}

fn read_state() -> Option<(String, u16)> {
    let raw = fs::read_to_string(state_path()).ok()?;
    let mut lines = raw.lines();
    let host = lines.next()?.trim().to_string();
    let port = lines.next()?.trim().parse::<u16>().ok()?;
    if host.is_empty() || port == 0 {
        return None;
    }
    Some((host, port))
}

/// Resolve o endereço do servidor: flags > ambiente/config > estado do último
/// `server start` (apenas quando `use_state`) > padrões.
fn resolve_addr(cli: &Cli, use_state: bool) -> (String, u16) {
    let state = if use_state { read_state() } else { None };
    let host = cli
        .host
        .clone()
        .or_else(|| env_non_empty("Q_RS_HOST"))
        .or_else(|| state.as_ref().map(|(host, _)| host.clone()))
        .unwrap_or_else(|| DEFAULT_HOST.to_string());
    let port = cli
        .port
        .or_else(|| env_non_empty("Q_RS_PORT").and_then(|value| value.trim().parse::<u16>().ok()))
        .or_else(|| state.as_ref().map(|(_, port)| *port))
        .filter(|port| *port != 0)
        .unwrap_or(DEFAULT_PORT);
    (host, port)
}

fn probe_host(host: &str) -> &str {
    match host {
        "0.0.0.0" | "::" | "[::]" => "127.0.0.1",
        other => other,
    }
}

fn port_open(host: &str, port: u16) -> bool {
    let timeout = Duration::from_millis(300);
    let Ok(addrs) = (probe_host(host), port).to_socket_addrs() else {
        return false;
    };
    addrs
        .into_iter()
        .any(|addr| TcpStream::connect_timeout(&addr, timeout).is_ok())
}

fn read_pid() -> Option<u32> {
    let raw = fs::read_to_string(pid_path()).ok()?;
    raw.trim().parse::<u32>().ok()
}

/// Envia um sinal sem poluir a saída com erros de processo inexistente.
fn signal(pid: u32, name: &str) -> bool {
    Command::new("kill")
        .arg(name)
        .arg(pid.to_string())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn pid_alive(pid: u32) -> bool {
    Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn server_url(host: &str, port: u16) -> String {
    format!("http://{host}:{port}")
}

fn ensure_config_dir() -> io::Result<PathBuf> {
    let dir = config_dir();
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn server_run(chat: ChatService, cli: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    let (host, port) = resolve_addr(cli, false);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    println!("q-rs server listening at {}", server_url(&host, port));
    if probe_host(&host) != host {
        println!("acessível externamente em http://<ip-desta-maquina>:{port} (bind {host})");
    }
    runtime.block_on(q_server::run(Arc::new(chat), &host, port))?;
    Ok(())
}

fn server_start(cli: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    let (host, port) = resolve_addr(cli, false);

    if let Some(pid) = read_pid() {
        if pid_alive(pid) {
            println!(
                "servidor já em execução (pid {pid}) em {}",
                server_url(&host, port)
            );
            return Ok(());
        }
        let _ = fs::remove_file(pid_path());
    }

    let dir = ensure_config_dir()?;
    let exe = env::current_exe()?;
    let log = log_path();
    let command = format!(
        "nohup {} server run --host {} --port {} >> {} 2>&1 & echo $!",
        shell_quote(&exe.to_string_lossy()),
        shell_quote(&host),
        port,
        shell_quote(&log.to_string_lossy()),
    );
    let output = Command::new("sh").arg("-c").arg(&command).output()?;
    if !output.status.success() {
        return Err(format!(
            "falha ao iniciar o servidor: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into());
    }
    let pid: u32 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .map_err(|_| "não foi possível obter o pid do servidor iniciado".to_string())?;
    fs::write(pid_path(), pid.to_string())?;

    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if port_open(&host, port) {
            write_state(&host, port);
            println!(
                "servidor iniciado (pid {pid}) em {}",
                server_url(&host, port)
            );
            println!("config: {}  log: {}", dir.display(), log.display());
            return Ok(());
        }
        if !pid_alive(pid) {
            let _ = fs::remove_file(pid_path());
            return Err(format!(
                "o servidor encerrou durante a inicialização; veja o log: {}",
                log.display()
            )
            .into());
        }
        sleep(Duration::from_millis(200));
    }
    Err(format!(
        "o servidor não respondeu em 10s; veja o log: {}",
        log.display()
    )
    .into())
}

fn server_stop() -> Result<(), Box<dyn std::error::Error>> {
    let Some(pid) = read_pid() else {
        println!("servidor não está rodando");
        return Ok(());
    };
    if !pid_alive(pid) {
        let _ = fs::remove_file(pid_path());
        println!("servidor não estava rodando (pid file obsoleto removido)");
        return Ok(());
    }

    let _ = signal(pid, "-TERM");
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if !pid_alive(pid) {
            let _ = fs::remove_file(pid_path());
            println!("servidor parado (pid {pid})");
            return Ok(());
        }
        sleep(Duration::from_millis(100));
    }

    let _ = signal(pid, "-KILL");
    let _ = fs::remove_file(pid_path());
    println!("servidor forçado a parar (pid {pid})");
    Ok(())
}

fn server_status(cli: &Cli) -> i32 {
    let (host, port) = resolve_addr(cli, true);
    let url = server_url(&host, port);
    let log = log_path();
    let Some(pid) = read_pid() else {
        println!("servidor: parado");
        println!("url esperada: {url}   config: {}", config_dir().display());
        return 1;
    };
    if !pid_alive(pid) {
        println!("servidor: parado (pid file obsoleto: {pid})");
        return 1;
    }
    let answering = port_open(&host, port);
    println!("servidor: em execução (pid {pid})");
    println!("url: {url}");
    println!(
        "porta: {}",
        if answering {
            "respondendo"
        } else {
            "sem resposta"
        }
    );
    println!("log: {}", log.display());
    if answering {
        0
    } else {
        1
    }
}

fn prompt_port(default_port: u16) -> u16 {
    print!("Porta do servidor [{default_port}]: ");
    let _ = io::stdout().flush();
    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_err() {
        return default_port;
    }
    let input = input.trim();
    if input.is_empty() {
        return default_port;
    }
    match input.parse::<u16>() {
        Ok(port) if port != 0 => port,
        _ => {
            eprintln!("porta inválida: {input} (usando {default_port})");
            default_port
        }
    }
}

fn server_install(cli: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    let (host, port) = resolve_addr(cli, false);
    let port = match cli.port {
        Some(port) => port,
        None if io::stdin().is_terminal() => prompt_port(port),
        None => port,
    };
    let dir = ensure_config_dir()?;
    fs::create_dir_all(dir.join("skills"))?;

    let config = format!("Q_RS_HOST={host}\nQ_RS_PORT={port}\n");
    fs::write(config_path(), config)?;
    let _ = Command::new("chmod")
        .args(["600", &config_path().to_string_lossy()])
        .status();

    if !env_path().exists() {
        fs::write(
            env_path(),
            "# chaves de API do q-rs — variáveis de ambiente reais têm prioridade\n\
             # GROQ_API_KEY=\n\
             # OPENROUTER_API_KEY=\n",
        )?;
        let _ = Command::new("chmod")
            .args(["600", &env_path().to_string_lossy()])
            .status();
    }

    let unit_path = env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(|home| {
            PathBuf::from(home)
                .join(".config")
                .join("systemd")
                .join("user")
                .join("q-rs.service")
        });
    if let Some(unit_path) = unit_path {
        if let Some(parent) = unit_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let exe = env::current_exe()?;
        let unit = format!(
            "[Unit]\n\
             Description=q-rs server\n\
             After=network-online.target\n\
             \n\
             [Service]\n\
             Type=simple\n\
             ExecStart={exe} server run --host {host} --port {port}\n\
             EnvironmentFile={env}\n\
             Restart=on-failure\n\
             RestartSec=2\n\
             \n\
             [Install]\n\
             WantedBy=default.target\n",
            exe = exe.display(),
            env = env_path().display(),
        );
        fs::write(&unit_path, unit)?;
        let reload = Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false);
        let enable = Command::new("systemctl")
            .args(["--user", "enable", "q-rs.service"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false);
        if reload && enable {
            println!("serviço systemd habilitado: {}", unit_path.display());
        } else {
            println!(
                "unit gravada em {} (systemctl --user indisponível; use `q-rs server start`)",
                unit_path.display()
            );
        }
    }

    println!("configuração gravada: {}", config_path().display());
    println!("url do servidor: {}", server_url(&host, port));
    println!("próximos passos: q-rs server start   |   q-rs server status");
    Ok(())
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

fn main() -> Result<(), Box<dyn std::error::Error>> {
    load_config_files();
    let cli = parse_cli();

    match &cli.command {
        Action::Help => {
            print!("{USAGE}");
            return Ok(());
        }
        Action::ServerRun => {
            let provider = cli
                .provider
                .clone()
                .or_else(|| env_non_empty("Q_RS_DEFAULT_PROVIDER"))
                .unwrap_or_else(|| DEFAULT_PROVIDER.to_string());
            return server_run(build_chat(&provider), &cli);
        }
        Action::ServerStart => return server_start(&cli),
        Action::ServerStop => return server_stop(),
        Action::ServerStatus => std::process::exit(server_status(&cli)),
        Action::ServerInstall => return server_install(&cli),
        Action::Repl | Action::Run(_) => {}
    }

    let provider = cli
        .provider
        .clone()
        .or_else(|| env_non_empty("Q_RS_DEFAULT_PROVIDER"))
        .unwrap_or_else(|| DEFAULT_PROVIDER.to_string());
    let chat = Arc::new(build_chat(&provider));
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    match cli.command {
        Action::Run(ref text) => runtime.block_on(run_once(chat.as_ref(), text, &cli)),
        _ => runtime.block_on(run_repl(chat.as_ref(), &cli)),
    }
    Ok(())
}
