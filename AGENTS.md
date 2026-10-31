# q-rs — AGENTS.md

Compact guidance for future OpenCode sessions. Every line answers: "Would an agent likely miss this without help?"

## Quick commands

| Action | Command |
|---|---|
| Format all sources | `cargo fmt --all -- --check` |
| Check workspace compilation | `cargo check --workspace` |
| Run all tests | `cargo test --workspace` |
| CLI one-shot (demo provider) | `cargo run -p q-cli -- run "2+2?"` |
| Server foreground | `cargo run -p q-cli -- server run --port 8787` |
| Server daemon | `q-rs server start` / `stop` / `status` |
| Install (binary + config + daemon) | `bash install.sh [--port N] [--no-daemon]` |
| Build release binary | `cargo build --release -p q-cli` |

## Monorepo structure & ownership

This is a 7-crate Rust workspace (`Cargo.toml` root). Crate boundaries and owners:

- **`q-core`** — Domain contracts, types, errors, ports/traits. **Must NOT** depend on HTTP frameworks or concrete provider SDKs.
- **`q-application`** — Validation, provider selection, fallback, chat service (tool execution, round-limits). Depends on `q-core` only. **No HTTP, no reqwest/axum.** Entry point: `ChatService::complete()` / `stream()`.
- **`q-providers`** — Provider adapters (demo, OpenAI-compatible via reqwest). Depends on `q-core` + `reqwest`.
- **`q-server`** — HTTP routes (Axum), DTO translation, SSE streaming. Depends on `q-application`, `q-core`, `axum`.
- **`q-cli`** — Composition root: configures providers (env vars `GROQ_API_KEY`/`OPENROUTER_API_KEY`), tool registration (`CalculatorTool`), default provider, mode selection. Depends on all crates. Binary: `q-rs`.
- **`q-tools`** — Tool registry and execution boundary. Depends on `q-core`. `ToolRegistry` enforces name uniqueness, registration, and trusted execution.
- **`q-skills`** — Skill registry and execution. Depends on `q-core`. `SkillRegistry` enforces name uniqueness and `Skill::run(input, context) -> Result<SkillResult>`.

Shared dependency ordering (always): `cargo fmt -> cargo check --workspace -> cargo test --workspace`.

## Architecture constraints (NOT obvious from filenames)

1. **No HTTP in `q-core` or `q-application`**. Adding `reqwest`/`axum` there will break the contract.
2. **Provider fallback order**: `ChatService::candidates()` selects providers by model prefix/default. If `GROQ_API_KEY`/`OPENROUTER_API_KEY` are set in `q-cli`'s `main.rs`, those providers are appended; otherwise only `demo` is available.
3. **Tool definitions come from the trusted registry, NOT from the client**. `ChatService::complete()` sets `working.tools` from `self.tools.as_ref().map(|e| e.definitions())`. Client-supplied `tools` in `ChatRequest` are ignored for execution; only registry-trusted tools are exposed to the model.
4. **Skills are not invoked automatically**. `SkillRegistry::run()` must be called explicitly. The model does NOT auto-invoke skills — only registered tools via `ToolExecutor` are executed automatically on `tool_calls` from the model.
5. **`QError::Provider` wraps reqwest errors**. When `OpenAiCompatibleProvider::complete()` gets a non-2xx response, it returns `QError::Provider { provider, message: "HTTP {status}: {excerpt}" }`. Callers should expect this error type.
6. **`AppConfig::validate()` checks**: mode must be "cli" or "server"; port must not be 0; enabled providers must have an API key. This is the single source of truth for config validity.
7. **Server config defaults**: `q-cli` defaults to host `0.0.0.0` and port `8787` (external access by design); override with `--host`/`--port`, `Q_RS_HOST`/`Q_RS_PORT`, or `~/.config/q-rs/config`. `ServerConfig::default()` in `q-core` (127.0.0.1:8080) is legacy and not used by the binary.
8. **CLI is subcommand-driven**: `q-rs run "<text>"`, `q-rs server run|start|stop|status|install`, `q-rs --prompt "<system>" ...`. `Q_RS_MODE` is no longer required (only honored as a legacy fallback for `server`). `q-cli/src/main.rs` has no clap — argument parsing is manual.
9. **Daemon is PID-file based** (no systemd dependency): `server start` spawns `nohup <exe> server run` detached and writes `~/.config/q-rs/server.pid` + `server.state` (last host/port); `server stop` sends SIGTERM (SIGKILL after 5s); `server status` exits 0 only if the PID is alive and the port answers. `server install` additionally writes `~/.config/systemd/user/q-rs.service` when possible.

## Environment & setup quirks

- **Rust toolchain**: `cargo` and `rustc` must be available. Verify before CI runs.
- **Config dir**: `$Q_RS_CONFIG_DIR` > `$XDG_CONFIG_HOME/q-rs` > `~/.config/q-rs`. It holds `env` (API keys), `config` (`Q_RS_HOST`/`Q_RS_PORT`, written by `server install`), `server.pid`, `server.state`, `server.log`, `skills/`.
- **Config files are auto-loaded by `q-cli`** (`env` then `config`, KEY=VALUE lines, `#` comments, optional `export`/quotes). **Real environment variables always win** — a value already exported is never overwritten.
- **Env vars**:
  - `Q_RS_DEFAULT_PROVIDER` — default provider name ("demo" if unset).
  - `Q_RS_DEFAULT_MODEL` — default model string ("default" if unset).
  - `Q_RS_SYSTEM_PROMPT` — system prompt (overridden by `--prompt`/`-p`; `/system` sets it for the session).
  - `GROQ_API_KEY` / `OPENROUTER_API_KEY` — register the OpenAI-compatible providers.
  - `Q_RS_HOST` / `Q_RS_PORT` — server bind address (defaults `0.0.0.0` / `8787`).
  - `GROQ_BASE_URL` / `OPENROUTER_BASE_URL` / `GROQ_MODEL` / `OPENROUTER_MODEL` — optional overrides.
  - `Q_RS_MODE` — legacy only; no longer required.
- **`install.sh`** builds with `cargo build --release -p q-cli`, installs to `${Q_PREFIX:-$HOME/.local}/bin/q-rs`, creates `~/.config/q-rs/{env,skills}`, migrates the legacy `~/.config/q` dir, then runs `q-rs server install [--port N]` (skip with `--no-daemon`). Flags: `--prefix`, `--port`, `--no-daemon`, `--rebuild`, `--help`. It never writes systemd units itself.

## Testing quirks

- **Workspace tests**: `cargo test --workspace` runs tests across all crates. Some tests are defined inline in `q-application` (chat service tool execution, validation) and `q-tools` (CalculatorTool end-to-end).
- **Integration/http tests require services**. The `demo` provider is deterministic and does not need external keys. `OpenAiCompatibleProvider` tests require `GROQ_API_KEY`/`OPENROUTER_API_KEY` and network access — these are marked with `#[cfg(test)]` and may be skipped in CI if keys are absent.
- **Flaky/external tests** are opt-in; the core suite (`cargo test --workspace`) should pass with only the demo provider.
- **Test isolation**: `q-application` tests use `DemoForTest` (a local demo provider impl) and `q_tools::ToolRegistry` to verify tool execution end-to-end without external deps.

## Common pitfalls / gotchas

1. **Forgetting the workspace flag**. Many commands need `--workspace` (e.g., `cargo check --workspace`, `cargo test --workspace`). Without it, only the current crate is checked.
2. **Confusing `q-application` with HTTP deps**. New contributors may be tempted to add `axum`/`reqwest` to `q-application` for convenience. This violates the architecture contract. Add them to `q-providers` or `q-server` instead.
3. **Client-supplied `tools` are ignored**. `ChatRequest` has a `tools` field, but `ChatService::complete()` overwrites it with trusted registry definitions. If you expect client tools to work automatically, you'll be surprised.
4. **Skill auto-invocation is not implemented**. Skills are registered and can be run via `SkillRegistry::run()`, but the chat loop does NOT call skills when the model returns `tool_calls`. Only `ToolExecutor` (tools) are auto-executed.
5. **Provider failure falls through silently**. `complete_with_fallback()` iterates providers and logs failures via `eprintln!`, then returns `QError::AllProvidersFailed` if all fail. The HTTP handler maps this to 502 Bad Gateway.
6. **Port zero is rejected**. `AppConfig::validate()` rejects `ServerConfig { port: 0 }`; `q-cli` also rejects `--port 0`. Always ensure port > 0.
7. **`Q_RS_MODE` is legacy**. Entry point is chosen by subcommand now; don't add new behavior keyed on `Q_RS_MODE`.
8. **`server start` uses the binary at `current_exe()`**. Testing with `cargo run` daemonizes `target/debug/q-rs`, not the installed binary — fine for dev, but `install.sh` should be used to validate the real install path.
9. **`server status` reads `server.state`** for the last host/port, so it reports the right URL after `stop` even without flags. Flags/env always override it.

## Existing instruction sources (referenced, not duplicated)

- **`README.md`** — High-level overview, crate responsibilities, dependency flow diagram, execution commands, architecture rules, next steps. This file is the primary narrative source; AGENTS.md distills the agent-relevant facts.
- **`install.sh`** — Installation flow: binary build/copy, config dir + `env`, skill injection, `q-rs server install` for the daemon.
- **`Cargo.toml`** (root and per-crate) — Workspace members, dependency versions, dev-dependency wiring. The source of truth for what compiles together.
- **`opencode.json`** (if present) — Any OpenCode-specific configuration; not checked beyond what's in the workspace root.

## What to verify before starting work

1. Run `cargo fmt --all -- --check` and `cargo check --workspace` to confirm the workspace compiles.
2. Run `cargo test --workspace` to confirm the test suite passes (at least with the demo provider).
3. If adding a new provider: ensure its crate depends on `q-core` only (no `axum`/`reqwest` in `q-core`/`q-application`).
4. If adding a new tool: implement `q_core::Tool`, register in `q_tools::ToolRegistry`, and ensure the registry is wired via `ChatService::with_tools()` in the composition root (`q-cli`).
5. If adding a new skill: implement `q_core::Skill`, register in `q_skills::SkillRegistry`, and call `SkillRegistry::run()` explicitly — it will NOT be called automatically from the chat loop.
6. Check `AppConfig::validate()` constraints (mode, port, API keys) before runtime.