#!/usr/bin/env python3
"""Chama endpoints da q-rs API e retorna somente texto (sem JSON)."""
import sys, urllib.request, json

HOST = "http://localhost:8787"

def post_json(path, payload):
    data = json.dumps(payload).encode()
    req = urllib.request.Request(
        HOST + path,
        data=data,
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=15) as resp:
        return resp.read().decode()

def ask(text):
    print("--- /v1/ask ---")
    raw = post_json("/v1/ask", {"q": text})
    body = json.loads(raw)
    print(body.get("answer", raw).strip())

def chat(text):
    print("--- /v1/chat/completions ---")
    raw = post_json("/v1/chat/completions", {
        "model": "default",
        "messages": [{"role":"user","content":text}],
    })
    body = json.loads(raw)
    choices = body.get("choices", [])
    msg = choices[0].get("message", {}) if choices else {}
    print(msg.get("content", raw).strip())

if __name__ == "__main__":
    query = " ".join(sys.argv[1:]) if sys.argv[1:] else "olá"
    ask(query)
    chat(query)
