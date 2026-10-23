# q-rs

Pergunta rápida em Rust: CLI com streaming token-a-token e servidor HTTP compatível com a API da OpenAI (funciona como provider do OpenCode).

**Site:** https://github.com/vinihss/q-rs  
**Binário:** `~/.local/bin/q-rs` (~3,8 MB)

---

## 📦 Instalação

```bash
# Via script (recomendado)
bash /home/vinicius/q-rs/install.sh

# Ou manual:
git clone https://github.com/vinihss/q-rs.git
cd q-rs
cargo build --release
cp target/release/q-rs ~/.local/bin/
```

### Chaves de API (ambiente do serviço)
O `q-rs` lê as chaves das variáveis de ambiente `GROQ_API_KEY` e `OPENROUTER_API_KEY`.  
Coloque-as no arquivo `~/.config/q/env` (formato `VAR=valor`, permissions 600) para o serviço systemd usar.

---

## 🚀 CLI (Linha de Comando)

```bash
# Pergunta direta (usa modelo default + fallback automático)
q-rs "Capital da Austrália?"

# Com modelo específico
q-rs -m openrouter/recursive/flint-v1.0:free "quem é você?"

# Habilidades (skills)
q-rs -s tradutor "Hello world"
q-rs -s resumir "Texto longo para resumir..."
q-rs -s corretor "voce foi ao mercado"
q-rs -s auxiliar "organizar festa"
q-rs -s traduzir "How are you?"
q-rs -s revisor "seu código aqui"

# Via stdin (pipeline)
echo "qual a raiz de 144?" | q-rs

# Interativo (sem argumento, em terminal)
q-rs

# Listar skills disponíveis
q-rs skills

# Tool: executar comando shell (limitado e seguro)
q-rs tool shell "echo ola"
q-rs tool shell "pwd"

# Modelo específico via argumento
q-rs -m groq/qwen/qwen3.8-27b "modelo específico"

# Ajuda
q-rs -h
```

### Flags CLI
| Flag | Descrição |
|------|-----------|
| `-m --model` | Modelo a usar (ex: `groq/openai/gpt-oss-20b`) |
| `-s --skill` | Habilidade a usar (veja `q-rs skills`) |
| `-h --help` | Mostra ajuda |

---

## 🛠️ Tools (ferramentas limitadas e seguras)

O q-rs inclui tools restritas para execução de comandos e leitura de arquivos.

```bash
# Executar comando shell (bloqueia pipes, redirecionamento, operadores perigosos)
q-rs tool shell "echo ola mundo"

# Ler arquivo relativo ao projeto
q-rs tool fs_read --path Cargo.toml

# Lista de tools disponíveis
q-rs tool list
```

**Segurança:**
- Comandos com `|`, `>`, `<`, `&&`, `||` são bloqueados
- Máximo 100 caracteres por comando
- Apenas `shell` e `fs_read` estão disponíveis
- Caminhos de arquivo devem ser relativos ao projeto

---

## 🌐 Servidor HTTP (`q-rs serve`)

```bash
q-rs serve              # 127.0.0.1:8787 (só local)
q-rs serve --bind 0.0.0.0  # expõe na LAN/Tailscale (seu IP: 100.95.254.27)
```

| Rota | Descrição |
|------|-----------|
| `GET /` | `?q=...` (+ `model=` opcional) → resposta em **text/plain** |
| `POST /v1/ask` | `{"q": "...", "format": "text"?}` → SSE (default) ou **text/plain** |
| `POST /v1/chat/completions` | Compatível com a API da OpenAI (streaming e não-streaming) |
| `GET /v1/models` | Catálogo de modelos |
| `GET /health` | Saúde |

### Exemplos de chamada

```bash
# plain text — uma linha, sem SSE (funciona até no navegador/Atalhos)
curl "http://100.95.254.27:8787/?q=Capital+da+Austrália"
# → Canberra.   [text/plain; charset=utf-8]

# plain text via POST
curl -s -X POST http://127.0.0.1:8787/v1/ask \
  -H 'Content-Type: application/json' \
  -d '{"q": "Capital da Austrália?", "format": "text"}'

# /v1/ask — SSE token-a-token (eventos "data: <delta>")
curl -N -X POST http://127.0.0.1:8787/v1/ask \
  -H 'Content-Type: application/json' \
  -d '{"q": "Capital da Austrália?"}'

# /v1/chat/completions — compatível com a API da OpenAI (streaming)
curl -N -X POST http://127.0.0.1:8787/v1/chat/completions \
  -H 'Content-Type: application/json' \
  -d '{"model": "groq/openai/gpt-oss-20b", "stream": true,
       "messages": [{"role": "user", "content": "oi"}]}'

# Catálogo de modelos
curl http://127.0.0.1:8787/v1/models

# Saúde
curl http://127.0.0.1:8787/health
```

### Acesso remoto (VPN/Tailscale)

Com o serviço em `--bind 0.0.0.0`, qualquer device do tailnet chama o IP Tailscale da máquina:

```bash
# De outro device (ex.: hostinger, iPhone) — IP Tailscale do servidor:
curl "http://100.95.254.27:8787/?q=Capital+da+Austrália"
```

**⚠️ Atenção:** o endpoint não tem autenticação — quem estiver na mesma rede/tailnet usa a cota do seu Groq/OpenRouter.

---

## 🧠 Skills (system prompt personalizável)

Skills são arquivos `~/.config/q/skills/<nome>.md` que adicionam instruções ao system prompt. O q-rs já vem com 6 skills padrão:

| Skill | Entrada | Saída |
|-------|---------|-------|
| **tradutor** | "Hello world" | "Olá mundo." |
| **resumir** | Texto longo | Resumo claro (até 150 palavras) |
| **corretor** | "voce foi ao mercado" | Texto corrigido |
| **revisor** | Código Rust | Análise de bugs e melhorias |
| **traduzir** | "How are you?" | "Como vai?" |
| **auxiliar** | "festa de 10 pessoas" | Plano com 10 itens |

### Como usar:

```bash
q-rs -s tradutor "texto para traduzir"
q-rs -s resumir "Seu texto longo aqui"
q-rs -s corretor "texto com erros"
q-rs -s auxiliar "sua tarefa aqui"
q-rs -s traduzir "texto em inglês"
q-rs -s revisor "seu código aqui"

# Via API:
curl "http://100.95.254.27:8787/?q=texto&skill=tradutor"

# Listar:
q-rs skills
```

### Como criar sua própria skill:
Coloque um arquivo `~/.config/q/skills/<nome>.md` com o seguinte formato (padrão OpenCode/claude):

```markdown
# Nome da Skill

Instruções para o modelo.

## Exemplos

### Entrada
Texto de exemplo.

### Saída
Resposta esperada.
```
```

---

## 🔄 Fallback automático

Em todos os caminhos, se o modelo principal falhar (429, erro), a cadeia segue:

1. `groq/openai/gpt-oss-20b` (default; `reasoning_effort: low` no Groq)
2. `groq/qwen/qwen3.8-27b`
3. `openrouter/recursive/flint-v1.0:free`
4. `openrouter/google/gemma-4-31b-it:free`
5. `openrouter/nvidia/nemotron-3.5-lightning:free`

---

## 🤖 Provider do OpenCode

Adicione ao seu `~/.config/opencode/opencode.jsonc`:

```jsonc
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

---

## 📄 Licença

MIT

---

**GitHub:** https://github.com/vinihss/q-rs  
**Documentação:** https://github.com/vinihss/q-rs/blob/main/README.md