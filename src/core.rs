//! Núcleo do q: prompt, resolução de provider, streaming SSE e fallback.
use futures_util::Stream;
use serde_json::{json, Value};
use std::io::IsTerminal;
use std::sync::OnceLock;
use std::time::Instant;

pub const SYSTEM_BASE: &str = "Você é um assistente de respostas rápidas. Responda de forma curta, direta e em português. Sem enrolação, sem ferramentas, sem passos extras — só a resposta.";
pub const DEFAULT_MODEL: &str = "groq/openai/gpt-oss-20b";
pub const FALLBACKS: &[&str] = &[
    "groq/qwen/qwen3.8-27b",
    "openrouter/recursive/flint-v1.0:free",
    "openrouter/google/gemma-4-31b-it:free",
    "openrouter/nvidia/nemotron-3.5-lightning:free",
];

#[derive(Clone, Debug)]
pub struct Resolved {
    pub endpoint: &'static str,
    pub key: String,
    pub model_id: String,
    pub provider: &'static str,
}

/// Resolve "groq/<id>" | "openrouter/<id>" | "<id>" (assume groq).
pub fn resolve(spec: &str) -> Result<Resolved, String> {
    let groq_key = std::env::var("GROQ_API_KEY").unwrap_or_default();
    let or_key = std::env::var("OPENROUTER_API_KEY").unwrap_or_default();
    let (endpoint, key, model_id, provider) = if let Some(rest) = spec.strip_prefix("groq/") {
        ("https://api.groq.com/openai/v1/chat/completions", groq_key, rest.to_string(), "groq")
    } else if let Some(rest) = spec.strip_prefix("openrouter/") {
        ("https://openrouter.ai/api/v1/chat/completions", or_key, rest.to_string(), "openrouter")
    } else if !groq_key.is_empty() {
        ("https://api.groq.com/openai/v1/chat/completions", groq_key, spec.to_string(), "groq")
    } else {
        ("https://openrouter.ai/api/v1/chat/completions", or_key, spec.to_string(), "openrouter")
    };
    if key.is_empty() {
        return Err(format!("chave de API ausente para {provider}"));
    }
    Ok(Resolved { endpoint, key, model_id, provider })
}

/// Lista de tentativas: modelo pedido + fallbacks (sem repetir).
pub fn attempt_specs(model: &str) -> Vec<String> {
    let mut list = vec![model.to_string()];
    for f in FALLBACKS {
        let s = f.to_string();
        if !list.contains(&s) {
            list.push(s);
        }
    }
    list
}

pub fn attempts_note(r: &Resolved) -> String {
    format!("{}/{}", r.provider, r.model_id)
}

pub fn brief(s: &str) -> String {
    let one = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() > 160 {
        format!("{}…", one.chars().take(160).collect::<String>())
    } else {
        one
    }
}

/// System prompt = base fixa + ~/.config/q/instructions.md (Q_INSTRUCTIONS sobrepõe o caminho).
pub fn system_prompt() -> String {
    let path = std::env::var("Q_INSTRUCTIONS")
        .unwrap_or_else(|_| format!("{}/.config/q/instructions.md", std::env::var("HOME").unwrap_or_default()));
    match std::fs::read_to_string(&path) {
        Ok(extra) if !extra.trim().is_empty() => format!("{SYSTEM_BASE}\n\n{}", extra.trim()),
        _ => SYSTEM_BASE.to_string(),
    }
}

/// Client HTTP compartilhado (keep-alive → corta o TLS handshake em chamadas seguintes).
pub fn client() -> &'static reqwest::Client {
    static C: OnceLock<reqwest::Client> = OnceLock::new();
    C.get_or_init(|| {
        reqwest::Client::builder()
            .pool_idle_timeout(std::time::Duration::from_secs(90))
            .tcp_nodelay(true)
            .build()
            .expect("client http")
    })
}

pub fn build_body(r: &Resolved, messages: Vec<Value>, stream: bool) -> Value {
    let mut body = json!({
        "model": r.model_id,
        "messages": messages,
        "stream": stream,
        "temperature": 0.3,
        "max_tokens": 800,
    });
    // gpt-oss no Groq é reasoning por padrão: força effort baixo (resposta curta e barata).
    if r.provider == "groq" && r.model_id.starts_with("openai/gpt-oss") {
        body["reasoning_effort"] = json!("low");
    }
    body
}

pub fn request(r: &Resolved, body: &Value) -> reqwest::RequestBuilder {
    let mut req = client().post(r.endpoint).bearer_auth(&r.key).json(body);
    if r.provider == "openrouter" {
        req = req.header("HTTP-Referer", "https://opencode.ai").header("X-Title", "q-rs");
    }
    req
}

/// Consume a resposta SSE do upstream e emite os deltas de conteúdo (já decodificados).
pub fn delta_stream(resp: reqwest::Response) -> impl Stream<Item = Result<String, String>> {
    async_stream::try_stream! {
        let mut resp = resp;
        let mut buf: Vec<u8> = Vec::new();
        loop {
            let Some(chunk) = resp.chunk().await.map_err(|e| format!("stream: {e}"))? else {
                break;
            };
            buf.extend_from_slice(&chunk);
            while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
                let line = String::from_utf8_lossy(&buf[..pos]).trim_end_matches('\r').to_string();
                buf.drain(..=pos);
                if let Some(data) = line.strip_prefix("data:") {
                    let data = data.trim();
                    if data == "[DONE]" {
                        return;
                    }
                    if let Ok(v) = serde_json::from_str::<Value>(data) {
                        if let Some(content) = v["choices"][0]["delta"]["content"].as_str() {
                            if !content.is_empty() {
                                yield content.to_string();
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Cronômetro com saída estilo zsh (só em stderrTTY — não polui pipeline).
pub struct Timer(Instant);
impl Timer {
    pub fn start() -> Self {
        Timer(Instant::now())
    }
    pub fn finish(&self, note: &str) {
        if std::io::stderr().is_terminal() {
            eprintln!("\x1b[8m⏱ {:.2}s · {}\x1b[0m", self.0.elapsed().as_secs_f64(), note);
        }
    }
}
