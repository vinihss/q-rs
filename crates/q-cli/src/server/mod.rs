use crate::config;
use q_application::ChatService;
use std::{
    env, fs,
    io::{self, IsTerminal, Write},
    net::{TcpStream, ToSocketAddrs},
    path::PathBuf,
    process::Command,
    sync::Arc,
    thread::sleep,
    time::{Duration, Instant},
};

const DEFAULT_HOST: &str = "0.0.0.0";
const DEFAULT_PORT: u16 = 8787;

pub fn state_path() -> PathBuf {
    config::state_path()
}

pub fn write_state(host: &str, port: u16) {
    let _ = fs::write(state_path(), format!("{host}\n{port}\n"));
}

pub fn read_state() -> Option<(String, u16)> {
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
pub fn resolve_addr(cli: &crate::cli::Cli, use_state: bool) -> (String, u16) {
    let state = if use_state { read_state() } else { None };
    let host = cli
        .host
        .clone()
        .or_else(|| config::env_non_empty("Q_RS_HOST"))
        .or_else(|| state.as_ref().map(|(host, _)| host.clone()))
        .unwrap_or_else(|| DEFAULT_HOST.to_string());
    let port = cli
        .port
        .or_else(|| {
            config::env_non_empty("Q_RS_PORT").and_then(|value| value.trim().parse::<u16>().ok())
        })
        .or_else(|| state.as_ref().map(|(_, port)| *port))
        .filter(|port| *port != 0)
        .unwrap_or(DEFAULT_PORT);
    (host, port)
}

pub fn probe_host(host: &str) -> &str {
    match host {
        "0.0.0.0" | "::" | "[::]" => "127.0.0.1",
        other => other,
    }
}

pub fn port_open(host: &str, port: u16) -> bool {
    let timeout = Duration::from_millis(300);
    let Ok(addrs) = (probe_host(host), port).to_socket_addrs() else {
        return false;
    };
    addrs
        .into_iter()
        .any(|addr| TcpStream::connect_timeout(&addr, timeout).is_ok())
}

pub fn read_pid() -> Option<u32> {
    let raw = fs::read_to_string(config::pid_path()).ok()?;
    raw.trim().parse::<u32>().ok()
}

/// Envia um sinal sem poluir a saída com erros de processo inexistente.
pub fn signal(pid: u32, name: &str) -> bool {
    Command::new("kill")
        .arg(name)
        .arg(pid.to_string())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

pub fn pid_alive(pid: u32) -> bool {
    Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

pub fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub fn server_url(host: &str, port: u16) -> String {
    format!("http://{host}:{port}")
}

pub fn ensure_config_dir() -> io::Result<PathBuf> {
    let dir = config::config_dir();
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn server_run(
    chat: ChatService,
    cli: &crate::cli::Cli,
) -> Result<(), Box<dyn std::error::Error>> {
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

pub fn server_start(cli: &crate::cli::Cli) -> Result<(), Box<dyn std::error::Error>> {
    let (host, port) = resolve_addr(cli, false);

    if let Some(pid) = read_pid() {
        if pid_alive(pid) {
            println!(
                "servidor já em execução (pid {pid}) em {}",
                server_url(&host, port)
            );
            return Ok(());
        }
        let _ = fs::remove_file(config::pid_path());
    }

    let dir = ensure_config_dir()?;
    let exe = env::current_exe()?;
    let log = config::log_path();
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
    fs::write(config::pid_path(), pid.to_string())?;

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
            let _ = fs::remove_file(config::pid_path());
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

pub fn server_stop() -> Result<(), Box<dyn std::error::Error>> {
    let Some(pid) = read_pid() else {
        println!("servidor não está rodando");
        return Ok(());
    };
    if !pid_alive(pid) {
        let _ = fs::remove_file(config::pid_path());
        println!("servidor não estava rodando (pid file obsoleto removido)");
        return Ok(());
    }

    let _ = signal(pid, "-TERM");
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if !pid_alive(pid) {
            let _ = fs::remove_file(config::pid_path());
            println!("servidor parado (pid {pid})");
            return Ok(());
        }
        sleep(Duration::from_millis(100));
    }

    let _ = signal(pid, "-KILL");
    let _ = fs::remove_file(config::pid_path());
    println!("servidor forçado a parar (pid {pid})");
    Ok(())
}

pub fn server_status(cli: &crate::cli::Cli) -> i32 {
    let (host, port) = resolve_addr(cli, true);
    let url = server_url(&host, port);
    let log = config::log_path();
    let Some(pid) = read_pid() else {
        println!("servidor: parado");
        println!(
            "url esperada: {url}   config: {}",
            config::config_dir().display()
        );
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

pub fn prompt_port(default_port: u16) -> u16 {
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

pub fn server_install(cli: &crate::cli::Cli) -> Result<(), Box<dyn std::error::Error>> {
    let (host, port) = resolve_addr(cli, false);
    let port = match cli.port {
        Some(port) => port,
        None if io::stdin().is_terminal() => prompt_port(port),
        None => port,
    };
    let dir = ensure_config_dir()?;
    fs::create_dir_all(dir.join("skills"))?;

    let config = format!("Q_RS_HOST={host}\nQ_RS_PORT={port}\n");
    fs::write(config::config_path(), config)?;
    let _ = Command::new("chmod")
        .args(["600", &config::config_path().to_string_lossy()])
        .status();

    if !config::env_path().exists() {
        fs::write(
            config::env_path(),
            "# chaves de API do q-rs — variáveis de ambiente reais têm prioridade\n\
             # GROQ_API_KEY=\n\
             # OPENROUTER_API_KEY=\n",
        )?;
        let _ = Command::new("chmod")
            .args(["600", &config::env_path().to_string_lossy()])
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
            env = config::env_path().display(),
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

    println!("configuração gravada: {}", config::config_path().display());
    println!("url do servidor: {}", server_url(&host, port));
    println!("próximos passos: q-rs server start   |   q-rs server status");
    Ok(())
}
