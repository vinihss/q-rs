//! q-rs — pergunta rápida em Rust: CLI (paridade com o q zsh) + `serve` (endpoint).
mod core;
mod server;

use core::Timer;
use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;
use serde_json::json;
use std::io::{IsTerminal, Write};

const HELP: &str = "q-rs — pergunta rápida (streaming, API direta)

Uso: q-rs [-m provider/modelo] [pergunta...]
     q-rs                       prompt interativo
     echo \"pergunta\" | q-rs     lê do stdin
     q-rs serve [--port N]      endpoint HTTP (default 8787)

Endpoint:
  POST /v1/ask               {\"q\":\"...\",\"model\":\"groq/...\"} → SSE
  POST /v1/chat/completions  compatível OpenAI (usado como provider do OpenCode)
  GET  /v1/models            catálogo
  GET  /health

Modelos: groq/<id> ou openrouter/<id>. Default: groq/openai/gpt-oss-20b (fallback automático).
Chaves: GROQ_API_KEY e OPENROUTER_API_KEY.
Prompt:  base fixa + ~/.config/q/instructions.md (Q_INSTRUCTIONS muda o caminho).";

fn prompt_from_args(args: &[String]) -> Result<String, String> {
    if !args.is_empty() {
        return Ok(args.join(" "));
    }
    if !std::io::stdin().is_terminal() {
        let mut buf = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf).map_err(|e| e.to_string())?;
        return Ok(buf);
    }
    print!("❯ ");
    std::io::stdout().flush().ok();
    let mut line = String::new();
    std::io::stdin().read_line(&mut line).map_err(|e| e.to_string())?;
    Ok(line)
}

async fn cli(model: String, prompt: String) {
    let prompt = prompt.trim().to_string();
    if prompt.is_empty() {
        eprintln!("q-rs: pergunta vazia");
        std::process::exit(2);
    }
    let timer = Timer::start();
    let system = core::system_prompt();
    let mut errors: Vec<String> = Vec::new();

    for spec in core::attempt_specs(&model) {
        let r = match core::resolve(&spec) {
            Ok(r) => r,
            Err(e) => {
                errors.push(format!("{spec}: {e}"));
                continue;
            }
        };
        let messages = vec![
            json!({"role": "system", "content": system}),
            json!({"role": "user", "content": prompt}),
        ];
        let body = core::build_body(&r, messages, true);
        match core::request(&r, &body).send().await {
            Ok(resp) if resp.status().is_success() => {
                let mut out = tokio::io::stdout();
                let mut deltas = Box::pin(core::delta_stream(resp));
                let mut printed = false;
                let mut mid_stream_err = None;
                while let Some(item) = deltas.next().await {
                    match item {
                        Ok(s) => {
                            out.write_all(s.as_bytes()).await.ok();
                            out.flush().await.ok();
                            printed = true;
                        }
                        Err(e) => {
                            mid_stream_err = Some(e);
                            break;
                        }
                    }
                }
                if printed {
                    // resposta (parcial) chegou → não tenta outro modelo (evita duplicar)
                    println!();
                    timer.finish(&core::attempts_note(&r));
                    if let Some(e) = mid_stream_err {
                        eprintln!("q-rs: stream interrompido: {e}");
                    }
                    return;
                }
                errors.push(format!(
                    "{spec}: {}",
                    mid_stream_err.unwrap_or_else(|| "stream vazio".into())
                ));
            }
            Ok(resp) => {
                let t = resp.text().await.unwrap_or_default();
                errors.push(format!("{spec}: {}", core::brief(&t)));
            }
            Err(e) => errors.push(format!("{spec}: {e}")),
        }
    }
    eprintln!("q-rs: falhou em todos os modelos:");
    for e in &errors {
        eprintln!("  {e}");
    }
    std::process::exit(1);
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.first().map(String::as_str) == Some("serve") {
        let mut port: u16 = std::env::var("Q_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8787);
        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--port" | "-p" => {
                    port = args
                        .get(i + 1)
                        .and_then(|p| p.parse().ok())
                        .unwrap_or_else(|| {
                            eprintln!("q-rs: --port exige um número");
                            std::process::exit(2);
                        });
                    i += 2;
                }
                other => {
                    eprintln!("q-rs: opção desconhecida no serve: {other}");
                    std::process::exit(2);
                }
            }
        }
        server::run(port).await;
        return;
    }

    let mut model = core::DEFAULT_MODEL.to_string();
    let mut rest: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                println!("{HELP}");
                return;
            }
            "-m" | "--model" => {
                model = args.get(i + 1).cloned().unwrap_or_else(|| {
                    eprintln!("q-rs: -m exige um modelo");
                    std::process::exit(2);
                });
                i += 2;
            }
            other => {
                rest.push(other.to_string());
                i += 1;
            }
        }
    }

    let prompt = match prompt_from_args(&rest) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("q-rs: {e}");
            std::process::exit(2);
        }
    };
    cli(model, prompt).await;
}
