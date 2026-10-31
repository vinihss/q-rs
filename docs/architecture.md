# Arquitetura e direção das dependências

## Princípio

O domínio e os casos de uso não conhecem detalhes de transporte. Dependências de frameworks e provedores ficam nas bordas.

## Ports

- `ChatProvider`: completa uma conversa e, opcionalmente, fornece um stream de eventos.
- `ProviderRegistry`: consulta os adaptadores disponíveis.
- `Tool`: descreve schema e executa uma ferramenta.
- `Skill`: descreve instruções e executa uma capacidade reutilizável.

## Adapters

- `q-providers`: converte tipos internos para o protocolo remoto e traduz erros de rede/resposta.
- `q-server`: converte JSON HTTP em `ChatRequest`, chama o caso de uso e serializa a resposta.
- `q-cli`: cria os adapters, injeta as dependências e decide o modo de execução.

## Política de fallback

A aplicação tenta providers em ordem. Um prefixo explícito como `groq/model-id` restringe a seleção ao provider `groq`. Para `default`, o provider configurado como padrão é priorizado. A camada de aplicação não deve conhecer nomes de modelos específicos de um fornecedor.

## Evolução segura

O registry atual é estático. Se futuramente houver plugins dinâmicos, estabelecer uma fronteira de segurança explícita: plugins compilados/dinâmicos não devem receber acesso irrestrito a segredos, filesystem ou rede por padrão.
