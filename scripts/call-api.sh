#!/bin/bash
# Bash puro: chama q-rs API e retorna só texto (sem JSON)
HOST="http://localhost:8787"
QUERY="${*:-olá}"

echo "--- /v1/ask ---"
curl -s -X POST "$HOST/v1/ask" \
  -H "Content-Type: application/json" \
  -d "{\"q\":\"$QUERY\"}" | jq -r '.answer // empty'

echo "--- /v1/chat/completions ---"
curl -s -X POST "$HOST/v1/chat/completions" \
  -H "Content-Type: application/json" \
  -d "{\"model\":\"default\",\"messages\":[{\"role\":\"user\",\"content\":\"$QUERY\"}]}" | jq -r '.choices[0].message.content // empty'
