# Segurança

- Não commitar `.env` nem credenciais.
- Não expor o servidor além de loopback sem autenticação, TLS e limites de tráfego.
- Não confiar em nomes, argumentos ou saída de tools/skills sem validação.
- Limitar tamanho de requests, tempo de execução e número de chamadas a tools antes de produção.
- Sanitizar mensagens de erro de providers remotos; respostas upstream podem conter detalhes sensíveis.
- Implementar timeouts, retries com backoff e redaction de logs antes de uso de produção.
