# q-rs — workspace de referência

Base Rust modular para uma CLI e uma API HTTP de chat compatível com o formato de Chat Completions da OpenAI.

## Estado desta entrega

Esta é uma **base arquitetural executável em intenção**, não uma migração integral do repositório original. Inclui contratos explícitos, provedor demo, adaptador JSON para endpoints OpenAI-compatible, API HTTP, interface SSE no servidor e registries de tools/skills. O provedor demo oferece streaming para validar o fluxo; o adaptador Groq/OpenRouter implementa completion JSON e tool calls no protocolo OpenAI-compatible. O ciclo de chat não-streaming agora publica somente as tools registradas, executa tool calls retornadas pelo modelo, adiciona resultados ao histórico e pede a resposta final ao modelo. O loop é limitado a 8 rodadas por padrão e 16 chamadas por rodada. Streaming com tool calls ainda não está integrado; skills continuam registráveis, mas não são invocadas automaticamente pelo modelo.

O ambiente em que esta atualização foi montada não tem `cargo`/`rustc`; portanto, **não foi possível executar `cargo test` ou confirmar a compilação**. Rode os comandos de validação abaixo antes de usar em produção.

## Crates

| Crate | Responsabilidade | Pode depender de |
|---|---|---|
| `q-core` | Tipos de domínio, erros e ports/traits | bibliotecas de serialização/traits; não conhece Axum nem SDKs de provedores |
| `q-application` | Validação, seleção de provider, fallback e casos de uso | `q-core` |
| `q-providers` | Adaptadores de providers (demo e protocolo OpenAI-compatible) | `q-core`, HTTP client |
| `q-server` | Rotas HTTP e tradução de DTOs para casos de uso | `q-application`, `q-core`, Axum |
| `q-cli` | Composition root: configuração do processo e escolha CLI/server | aplicação, providers, server |
| `q-tools` | Registro e execução de ferramentas | `q-core` |
| `q-skills` | Registro e execução de skills | `q-core` |

## Interfaces e contratos

- `ChatProvider`: `complete`, `stream`, `name` e política `supports_model`.
- `ProviderRegistry`: expõe os providers disponíveis; a implementação inicial é `StaticProviderRegistry`.
- `ChatService`: valida entrada, seleciona providers e tenta fallback em completion/streaming.
- `StreamEvent`: eventos neutros de domínio (`Started`, `Delta`, `Finished`, `Usage`), traduzidos para SSE na borda HTTP.
- `Tool`: declara nome/descrição/schema de argumentos e executa com JSON.
- `ToolExecutor`: porta consumida por `q-application`; `ToolRegistry` a implementa sem acoplar a aplicação ao crate `q-tools`.
- `Skill`: declara instruções e executa com input e `SkillContext`.
- `AppConfig`: tipos para configuração; o binário de exemplo lê variáveis de ambiente diretamente.

## Dependências e fluxo

```text
q-cli ───────► q-application ─────► q-core ◄──── q-providers
  │                    ▲               ▲
  └────► q-server ─────┘          q-tools / q-skills
```

`q-core` define contratos. Adaptadores implementam esses contratos. `q-application` não deve importar `reqwest`, `axum` ou detalhes de Groq/OpenRouter.

## Executar

É necessário Rust estável e Cargo instalados.

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
```

Instalação (binário no PATH do usuário + configuração em `~/.config/q-rs`):

```bash
bash install.sh                  # compila, instala em ~/.local/bin, pergunta a porta do daemon
bash install.sh --port 8787      # sem prompt
bash install.sh --no-daemon      # só binário e config
```

Uso do CLI (não chama um LLM por padrão — usa o provider `demo`):

```bash
q-rs run "quanto é 2+2?"                  # one-shot
q-rs --prompt "responda em uma palavra" run "oi"
q-rs                                      # modo interativo (REPL)
q-rs --help
```

Servidor HTTP (bind `0.0.0.0:8787` por padrão, acessível na rede):

```bash
q-rs server install --port 8787   # grava ~/.config/q-rs/config + unit systemd de usuário
q-rs server start                 # sobe em background (PID em ~/.config/q-rs/server.pid)
q-rs server status                # exit 0 = rodando
q-rs server stop
q-rs server run --port 9000       # foreground (debug)
```

`cargo run -p q-cli -- run "..."` funciona igual durante o desenvolvimento.

Rotas disponíveis:

- `GET /health`
- `GET /v1/models`
- `POST /v1/ask` com `{"q":"Olá"}`
- `POST /v1/chat/completions` com `{"model":"default","messages":[{"role":"user","content":"Olá"}]}`
- `POST /v1/chat/completions` com `"stream":true` para receber eventos SSE. O provider selecionado precisa implementar `stream`; `demo` implementa, os adapters remotos ainda retornam `UnsupportedOperation`.

Para usar endpoints OpenAI-compatible, configure `GROQ_API_KEY` ou `OPENROUTER_API_KEY` em `~/.config/q-rs/env` (ou no ambiente do shell). O CLI carrega `~/.config/q-rs/env` e `~/.config/q-rs/config` na inicialização; variáveis já presentes no ambiente têm prioridade. Diretório alternativo: `Q_RS_CONFIG_DIR` ou `$XDG_CONFIG_HOME/q-rs`.

Exemplo de request:

```bash
curl http://127.0.0.1:8787/v1/chat/completions \
  -H 'content-type: application/json' \
  -d '{"model":"default","messages":[{"role":"user","content":"Olá"}]}'
```

## Regras de arquitetura

1. Não coloque chamadas HTTP em `q-core` ou `q-application`.
2. Novos providers implementam `ChatProvider` em `q-providers` ou em crate externo.
3. Novas tools implementam `q_core::Tool` e são registradas em `ToolRegistry`.
4. Novas skills implementam `q_core::Skill` e são registradas em `SkillRegistry`.
5. Não execute tools arbitrárias sem validação de schema, autorização, timeout e limites de recursos.
6. Nunca registre API keys, headers de autorização ou conteúdo sensível em logs.

## Próximos passos recomendados

1. Executar `cargo fmt`, `cargo check` e `cargo test`; corrigir eventuais incompatibilidades de toolchain.
2. Implementar parser SSE real no adapter OpenAI-compatible.
3. Adicionar validação de schema genérica, autorização por tool e timeouts por execução.
4. Integrar tool calls ao streaming, com eventos estruturados de chamada de ferramenta.
5. Integrar skills ao roteamento da aplicação, se necessário.
6. Adicionar carregamento de configuração validado e timeouts/retries configuráveis.
7. Adicionar testes HTTP e testes de contrato para cada provider.

## Execução automática de tools

O CLI registra uma ferramenta de exemplo chamada `calculator`. Para permitir que um modelo remoto a selecione, configure uma chave e escolha o provedor como padrão, por exemplo:

```bash
export GROQ_API_KEY="..."
export Q_RS_DEFAULT_PROVIDER=groq
export Q_RS_DEFAULT_MODEL=default
cargo run -p q-cli
```

Quando o modelo retorna `tool_calls`, `ChatService` executa somente ferramentas presentes no `ToolRegistry` confiável. O resultado é enviado de volta como mensagem com `role: tool`, e o modelo é chamado novamente para formular a resposta. O modelo não executa código arbitrário: cada ferramenta precisa estar registrada e implementar validação própria dos argumentos.

O modo `demo` é determinístico e não solicita ferramentas; use um modelo remoto que suporte tool/function calling para testar a seleção automática. O loop de tools aplica limite de rodadas e de chamadas por rodada. Ainda é recomendado adicionar timeouts por ferramenta, autorização por usuário e validação genérica contra `parameters_schema` antes de expor ferramentas sensíveis.
