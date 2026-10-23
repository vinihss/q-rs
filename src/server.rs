//! Endpoint HTTP: / (plain text), /v1/ask (SSE ou plain) e /v1/chat/completions (compatível OpenAI → provider do OpenCode).
use crate::core;
use axum::body::Body;
use axum::extract::{Json, Query};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response, Sse};
use axum::routing::{get, post};
use axum::Router;
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::{json, Value};
use std::convert::Infallible;

/// Ordem de tentativa: modelo pedido (se houver) + fallbacks, sem repetir.
fn attempts(requested: Option<&str>) -> Vec<String> {
    let mut list = Vec::new();
    let push = |list: &mut Vec<String>, s: String| {
        if !list.contains(&s) {
            list.push(s);
        }
    };
    match requested {
        Some(m) if !m.is_empty() && m != "fast" => push(&mut list, m.to_string()),
        _ => push(&mut list, core::DEFAULT_MODEL.to_string()),
    }
    for f in core::FALLBACKS {
        push(&mut list, f.to_string());
    }
    list
}

fn brief(s: &str) -> String {
    let one = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() > 160 {
        format!("{}…", one.chars().take(160).collect::<String>())
    } else {
        one
    }
}

// ---------- GET /health ----------
async fn health() -> &'static str {
    "ok"
}

// ---------- GET /v1/models (catálogo pro OpenCode) ----------
async fn models() -> Json<Value> {
    let data: Vec<Value> = attempts(None)
        .into_iter()
        .map(|id| json!({"id": id, "object": "model", "owned_by": "q-rs"}))
        .collect();
    Json(json!({"object": "list", "data": data}))
}

// ---------- GET /?q=...&model=... → resposta completa em text/plain ----------
#[derive(Deserialize)]
pub struct AskQuery {
    pub q: String,
    #[serde(default)]
    pub model: Option<String>,
}

/// Mesmo fallback do SSE, mas coleta os deltas e devolve o texto final.
async fn ask_collect(q: &str, model: Option<&str>) -> Result<String, (StatusCode, String)> {
    let mut errors: Vec<String> = Vec::new();
    for spec in attempts(model) {
        let r = match core::resolve(&spec) {
            Ok(r) => r,
            Err(e) => {
                errors.push(format!("{spec}: {e}"));
                continue;
            }
        };
        let messages = vec![
            json!({"role": "system", "content": core::system_prompt()}),
            json!({"role": "user", "content": q}),
        ];
        let body = core::build_body(&r, messages, true);
        match core::request(&r, &body).send().await {
            Ok(resp) if resp.status().is_success() => {
                let mut out = String::new();
                let mut deltas = Box::pin(core::delta_stream(resp));
                let mut stream_err = None;
                while let Some(item) = deltas.next().await {
                    match item {
                        Ok(s) => out.push_str(&s),
                        Err(e) => {
                            stream_err = Some(e);
                            break;
                        }
                    }
                }
                if !out.is_empty() {
                    // resposta parcial também vale (igual ao CLI)
                    return Ok(out);
                }
                errors.push(format!(
                    "{spec}: {}",
                    stream_err.unwrap_or_else(|| "stream vazio".into())
                ));
            }
            Ok(resp) => {
                let t = resp.text().await.unwrap_or_default();
                errors.push(format!("{spec}: {}", brief(&t)));
            }
            Err(e) => errors.push(format!("{spec}: {e}")),
        }
    }
    Err((StatusCode::BAD_GATEWAY, errors.join("\n")))
}

fn text_response(s: String) -> Response {
    Response::builder()
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(Body::from(s))
        .expect("response")
}

async fn ask_get(Query(p): Query<AskQuery>) -> Result<Response, (StatusCode, String)> {
    let answer = ask_collect(&p.q, p.model.as_deref()).await?;
    Ok(text_response(answer))
}

// ---------- POST /v1/ask {"q":"...","model":"groq/...","format":"text"?} ----------
#[derive(Deserialize)]
pub struct AskPayload {
    pub q: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub format: Option<String>,
}

async fn ask(Json(p): Json<AskPayload>) -> Result<Response, (StatusCode, String)> {
    if p.format.as_deref() == Some("text") {
        let answer = ask_collect(&p.q, p.model.as_deref()).await?;
        return Ok(text_response(answer));
    }
    let mut errors: Vec<String> = Vec::new();
    for spec in attempts(p.model.as_deref()) {
        let r = match core::resolve(&spec) {
            Ok(r) => r,
            Err(e) => {
                errors.push(format!("{spec}: {e}"));
                continue;
            }
        };
        let messages = vec![
            json!({"role": "system", "content": core::system_prompt()}),
            json!({"role": "user", "content": p.q}),
        ];
        let body = core::build_body(&r, messages, true);
        match core::request(&r, &body).send().await {
            Ok(resp) if resp.status().is_success() => {
                let events = core::delta_stream(resp).filter_map(|item| async move {
                    match item {
                        Ok(d) => Some(Ok::<_, Infallible>(axum::response::sse::Event::default().data(d))),
                        Err(_) => None, // fim do stream: encerra o SSE
                    }
                });
                return Ok(Sse::new(events).into_response());
            }
            Ok(resp) => {
                let t = resp.text().await.unwrap_or_default();
                errors.push(format!("{spec}: {}", brief(&t)));
            }
            Err(e) => errors.push(format!("{spec}: {e}")),
        }
    }
    Err((StatusCode::BAD_GATEWAY, errors.join("\n")))
}

// ---------- POST /v1/chat/completions (pass-through OpenAI) ----------
async fn chat(mut body: Json<Value>) -> Result<Response, (StatusCode, String)> {
    let requested = body.0.get("model").and_then(|m| m.as_str()).map(|s| s.to_string());
    let mut errors: Vec<String> = Vec::new();

    for spec in attempts(requested.as_deref()) {
        let r = match core::resolve(&spec) {
            Ok(r) => r,
            Err(e) => {
                errors.push(format!("{spec}: {e}"));
                continue;
            }
        };
        // O upstream recebe só o id dele; o cliente fala em "groq/..." (ou alias).
        body.0["model"] = json!(r.model_id);
        match core::request(&r, &body.0).send().await {
            Ok(resp) if resp.status().is_success() => {
                let ct = resp
                    .headers()
                    .get(header::CONTENT_TYPE)
                    .cloned()
                    .unwrap_or_else(|| HeaderValue::from_static("text/event-stream"));
                // Repassa o stream cru (SSE ou JSON puro, conforme "stream" do cliente).
                let upstream = Response::builder()
                    .header(header::CONTENT_TYPE, ct)
                    .body(Body::from_stream(resp.bytes_stream()))
                    .expect("response");
                return Ok(upstream);
            }
            Ok(resp) => {
                let t = resp.text().await.unwrap_or_default();
                errors.push(format!("{spec}: {}", brief(&t)));
            }
            Err(e) => errors.push(format!("{spec}: {e}")),
        }
    }
    Err((StatusCode::BAD_GATEWAY, errors.join("\n")))
}

pub fn app() -> Router {
    Router::new()
        .route("/", get(ask_get))
        .route("/health", get(health))
        .route("/v1/models", get(models))
        .route("/v1/ask", post(ask))
        .route("/v1/chat/completions", post(chat))
}

pub async fn run(bind: &str, port: u16) {
    let addr: std::net::SocketAddr = format!("{bind}:{port}")
        .parse()
        .unwrap_or_else(|_| panic!("endereço inválido: {bind}:{port}"));
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
    eprintln!("q-rs serve em http://{addr}  (POST /v1/ask · POST /v1/chat/completions · GET /v1/models · GET /health)");
    axum::serve(listener, app()).await.expect("serve");
}
