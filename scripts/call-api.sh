#!/bin/sh
# Chama endpoints da q-rs API e retorna somente texto (sem JSON)
HOST="http://localhost:8787"
QUERY="${*:-olá}"

echo "--- /v1/ask ---"
curl -s -X POST "$HOST/v1/ask" \
  -H "Content-Type: application/json" \
  -d "{\"q\":\"$QUERY\"}" | python3 -c "
import sys, json; d=json.load(sys.stdin); print(d.get('answer','').strip())
"

echo "--- /v1/chat/completions ---"
curl -s -X POST "$HOST/v1/chat/completions" \
  -H "Content-Type: application/json" \
  -d "{\"model\":\"default\",\"messages\":[{\"role\":\"user\",\"content\":\"$QUERY\"}]}" | python3 -c "
import sys, json; d=json.load(sys.stdin); c=d.get('choices',[]); msg=c[0].get('message',{}) if c else {}; print(msg.get('content','').strip())
"
