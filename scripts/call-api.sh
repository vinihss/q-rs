#!/bin/bash
# Bash puro: chama q-rs API e retorna só texto (sem JSON)
HOST="http://avalon:8787"
QUERY="$1"


curl -s -X POST "$HOST/v1/chat/completions" \
  -H "Content-Type: application/json" \
  -d "{\"model\":\"default\",\"messages\":[{\"role\":\"user\",\"content\":\"$QUERY\"}]}" | jq -r '.choices[0].message.content // empty'
