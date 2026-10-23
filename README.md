# q-rs — workspace refatorado

Exemplo funcional de arquitetura Rust com Cargo Workspace, API HTTP em Axum, CLI, fallback entre provedores OpenAI-compatible e provedor demo para testar sem chaves.

## Requisitos

- Rust stable e Cargo (use `rustup` para instalar)
- Para respostas reais: `GROQ_API_KEY` e/ou `OPENROUTER_API_KEY`

## Configuração

Copie `.env.example` para `.env` se quiser usá-lo como referência, mas o projeto não carrega `.env` automaticamente. Exporte as variáveis no shell:

```bash
export GROQ_API_KEY="sua-chave"
# opcional
export OPENROUTER_API_KEY="sua-chave"
```

O programa usa os provedores cujas variáveis de ambiente estejam configuradas. Se nenhum estiver configurado, usa o `DemoProvider`, que responde localmente e não chama uma IA.

## Executar

```bash
cargo fmt --all
cargo check --workspace
cargo test --workspace

# CLI (demo se não houver chaves)
cargo run -p q-cli -- ask "Explique ownership em Rust"

# API HTTP
cargo run -p q-cli -- serve 8787
```

O servidor escuta em `127.0.0.1` por padrão. Testes:

```bash
curl http://127.0.0.1:8787/health
curl http://127.0.0.1:8787/v1/models
curl -X POST http://127.0.0.1:8787/v1/ask \
  -H 'Content-Type: application/json' \
  -d '{"q":"O que é borrowing em Rust?"}'

curl -X POST http://127.0.0.1:8787/v1/chat/completions \
  -H 'Content-Type: application/json' \
  -d '{"model":"default","messages":[{"role":"user","content":"Olá"}]}'
```

## Estrutura

- `q-core`: tipos, erros e contrato `Provider`.
- `q-application`: casos de uso e política de fallback.
- `q-providers`: adaptador OpenAI-compatible e provedor demo.
- `q-server`: API HTTP, DTOs e mapeamento de erros.
- `q-cli`: composição das dependências e interface de terminal.

## Observações

- Este é um projeto de referência independente, não uma migração byte-a-byte do repositório original. Não implementa todas as funções do projeto anterior, como tools, skills e SSE streaming.
- A rota de chat retorna JSON não-streaming. O contrato pode ser estendido com streaming sem acoplar a lógica da aplicação ao Axum.
- Antes de expor o servidor fora de localhost, adicione autenticação, limites de uso e configuração explícita de bind.
