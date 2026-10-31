# Contratos públicos

## ChatProvider

- `name()`: identificador estável e único.
- `supports_model(model)`: política para prefixos de modelo; `provider/model` seleciona explicitamente o adapter.
- `complete(request)`: retorna uma resposta completa.
- `stream(request)`: retorna stream de eventos; a implementação padrão retorna `UnsupportedOperation`.

## ChatService

Valida a solicitação e delega ao provider. O fallback de completion tenta os providers elegíveis na ordem de registro. Streaming retorna o primeiro stream que o provider conseguir abrir; erros que aconteçam depois da abertura são emitidos pelo próprio stream e não acionam fallback automaticamente.

## Tool

`definition()` fornece nome, descrição e JSON Schema de argumentos. `execute(arguments)` executa a ação. A validação do schema, autorização e timeout precisam ser aplicados na integração/orquestração antes de executar tools não confiáveis.

## Skill

`definition()` fornece nome, descrição e instruções. `run(input, context)` executa a skill e retorna conteúdo mais valores de contexto. Skills não devem assumir acesso a recursos privilegiados.

## StreamEvent

- `Started`: identificador e modelo.
- `Delta`: fragmento de conteúdo.
- `Finished`: razão de finalização.
- `Usage`: contagem de tokens quando fornecida pelo provider.
