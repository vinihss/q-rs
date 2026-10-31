#!/bin/bash
# Bash puro: chama q-rs API e retorna só texto (sem JSON)
HOST="http://avalon:8787"
QUERY="$1"

echo "--- /v1/ask ---"
curl -s -X POST "$HOST/v1/ask" \
  -H "Content-Type: application/json" \
  -d "{\"q\":\"$QUERY\",\"model\":\"groq/gpt-oss-20b\"}" | jq -r '.answer // empty'


