use std::{env, fs, path::Path, path::PathBuf};

pub fn config_dir() -> PathBuf {
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

pub fn pid_path() -> PathBuf {
    config_dir().join("server.pid")
}

pub fn log_path() -> PathBuf {
    config_dir().join("server.log")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config")
}

pub fn env_path() -> PathBuf {
    config_dir().join("env")
}

pub fn state_path() -> PathBuf {
    config_dir().join("server.state")
}

/// Carrega `env` e `config` do diretório de configuração sem sobrescrever
/// variáveis já presentes no ambiente real (o ambiente sempre vence).
pub fn load_config_files() {
    let dir = config_dir();
    for name in ["env", "config"] {
        load_key_value_file(&dir.join(name));
    }
}

pub fn load_key_value_file(path: &Path) {
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

pub fn unquote(value: &str) -> String {
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

pub fn env_or(key: &str, fallback: &str) -> String {
    env::var(key).unwrap_or_else(|_| fallback.to_owned())
}

pub fn env_non_empty(key: &str) -> Option<String> {
    env::var(key).ok().filter(|value| !value.trim().is_empty())
}
