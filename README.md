# q-rs

Pergunta rápida em Rust: CLI com streaming token-a-token e servidor HTTP compatível com a API da OpenAI (funciona como provider do OpenCode).

- **Grátis**: Groq (free tier) com fallback automático pro OpenRouter (`:free`)
- **Rápido**: mediana de ~0,56s do CLI até o primeiro token; ~0,37s warm via HTTP keep-alive
- **Streaming**: resposta aparece conforme os tokens chegam
- **Fallback automático**: se o modelo principal falhar (429, erro), tenta a cadeia na ordem

## Instalação

```bash
git clone https://github.com/vinihss/q-rs.git
cd q-rs
cargo build --release
cp target/release/q-rs ~/.local/bin/
```

Chaves no ambiente (o `q-rs` só lê de env, nunca de arquivo de config):

```bash
export GROQ_API_KEY="gsk_..."
export OPENROUTER_API_KEY="sk-or-..."
```

## CLI

```bash
# pergunta direta
q-rs "Capital da Austrália?"

# modelo específico (groq/<id> ou openrouter/<id>)
q-rs -m openrouter/recursive/flint-v1.0:free "quem é você?"

# via stdin (pipeline)
echo "qual a raiz de 144?" | q-rs

# interativo (sem argumento, em terminal)
q-rs

# ajuda
q-rs -h
```

System prompt = base fixa (português, curto e direto) + `~/.config/q/instructions.md` (sobrescreva o caminho com `Q_INSTRUCTIONS`).

## Servidor HTTP

```bash
q-rs serve                          # 127.0.0.1:8787 (só local)
q-rs serve --bind 0.0.0.0           # expõe na LAN/Tailscale (atenção: sem auth)
q-rs serve --port 9000 --bind IP    # porta/endereço customizados
# Q_PORT e Q_BIND (env) sobrepõem os defaults
```

| Rota | Descrição |
| --- | --- |
| `POST /v1/ask` | Q&A curto: `{"q": "...", "model": "groq/..."}` → SSE |
| `POST /v1/chat/completions` | Compatível com a API da OpenAI (streaming e não-streaming) |
| `GET /v1/models` | Catálogo de modelos |
| `GET /health` | Saúde |

### Exemplos de chamada

```bash
# /v1/ask — SSE token-a-token (eventos "data: <delta>")
curl -N -X POST http://127.0.0.1:8787/v1/ask \
  -H 'Content-Type: application/json' \
  -d '{"q": "Capital da Austrália?"}'

# /v1/ask com modelo específico
curl -N -X POST http://127.0.0.1:8787/v1/ask \
  -H 'Content-Type: application/json' \
  -d '{"q": "quem é você?", "model": "groq/qwen/qwen3.8-27b"}'

# /v1/chat/completions — compatível com a API da OpenAI (streaming)
curl -N -X POST http://127.0.0.1:8787/v1/chat/completions \
  -H 'Content-Type: application/json' \
  -d '{
    "model": "groq/openai/gpt-oss-20b",
    "stream": true,
    "messages": [{"role": "user", "content": "say OK only"}]
  }'

# idem, sem streaming (resposta JSON única)
curl -s -X POST http://127.0.0.1:8787/v1/chat/completions \
  -H 'Content-Type: application/json' \
  -d '{"model": "groq/openai/gpt-oss-20b", "stream": false,
       "messages": [{"role": "user", "content": "say OK only"}]}'

# catálogo de modelos / saúde
curl http://127.0.0.1:8787/v1/models
curl http://127.0.0.1:8787/health
```

Detalhes:

- **`model`** aceita `groq/<id>`, `openrouter/<id>`, ou omite pra usar o default (`groq/openai/gpt-oss-20b`). Se o modelo falhar (429/erro), cai automaticamente na cadeia de fallback.
- Campos extras (`temperature`, `max_tokens`, `reasoning_effort`, tools) são encaminhados como estão.
- O `/v1/chat/completions` repassa o stream cru do upstream — funciona com qualquer cliente OpenAI-compatível apontando `baseURL` pra `http://127.0.0.1:8787/v1`.

### Acesso remoto (VPN/Tailscale)

Com o serviço em `--bind 0.0.0.0`, qualquer device do tailnet chama o IP Tailscale da máquina:

```bash
# de outro device (ex.: hostinger, iPhone) — IP Tailscale do servidor:
curl -N -X POST http://100.95.254.27:8787/v1/ask \
  -H 'Content-Type: application/json' \
  -d '{"q": "Capital da Austrália?"}'
```

O endpoint não tem autenticação — quem estiver na mesma rede/tailnet usa a cota do seu Groq/OpenRouter. Pra restringir ao Tailscale use `--bind <ip-do-tailscale>` em vez de `0.0.0.0`.

## Como provider do OpenCode

```jsonc
// ~/.config/opencode/opencode.jsonc
{
  "providers": {
    "q": {
      "name": "q-rs",
      "package": "@opencode/ai/providers/openai-compatible",
      "settings": {
        "baseURL": "http://127.0.0.1:8787/v1",
        "apiKey": "local"
      },
      "models": {
        "fast": { "modelID": "groq/openai/gpt-oss-20b" },
        "free": { "modelID": "openrouter/recursive/flint-v1.0:free" }
      }
    }
  }
}
```

Uso: `opencode run -m q/fast "sua pergunta"`.

Com systemd user (sobe no boot e se reinicia sozinho):

```ini
# ~/.config/systemd/user/q-rs-serve.service
[Unit]
Description=q-rs serve
After=network-online.target

[Service]
ExecStart=%h/.local/bin/q-rs serve --port 8787
EnvironmentFile=%h/.config/q/env   # GROQ_API_KEY=... / OPENROUTER_API_KEY=...
Restart=on-failure
RestartSec=2

[Install]
WantedBy=default.target
```

```bash
systemctl --user daemon-reload
systemctl --user enable --now q-rs-serve
```

## Cadeia de fallback

1. `groq/openai/gpt-oss-20b` (default; `reasoning_effort: low` no Groq)
2. `groq/qwen/qwen3.8-27b`
3. `openrouter/recursive/flint-v1.0:free`
4. `openrouter/google/gemma-4-31b-it:free`
5. `openrouter/nvidia/nemotron-3.5-lightning:free`

## Licença

MIT
