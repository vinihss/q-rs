#!/usr/bin/env zsh
# q — pergunta rápida em uma linha de comando.
# Chama a API do modelo DIRETO (sem OpenCode, sem sessão), com streaming.
# Default: Groq (openai/gpt-oss-20b) e fallback automático para free do OpenRouter.
#
# Uso:
#   q "Qual a capital da Austrália?"
#   q -m openrouter/recursive/flint-v1.0:free "pergunta"
#   q                     # prompt interativo
#   echo "pergunta" | q   # stdin
set -uo pipefail
setopt typeset_silent
zmodload zsh/datetime

SYSTEM_PROMPT='Você é um assistente de respostas rápidas. Responda de forma curta, direta e em português. Sem enrolação, sem ferramentas, sem passos extras — só a resposta.'
# Instruções do usuário: anexadas ao prompt base se o arquivo existir.
INSTRUCTIONS_FILE="${Q_INSTRUCTIONS:-$HOME/.config/q/instructions.md}"
if [[ -f "$INSTRUCTIONS_FILE" ]]; then
  _extra=$(<$INSTRUCTIONS_FILE)
  [[ -n "${_extra//[[:space:]]/}" ]] && SYSTEM_PROMPT="$SYSTEM_PROMPT"$'\n\n'"$_extra"
fi
DEFAULT_MODEL="groq/openai/gpt-oss-20b"
FALLBACK_MODELS=(
  "groq/qwen/qwen3.8-27b"
  "openrouter/recursive/flint-v1.0:free"
  "openrouter/google/gemma-4-31b-it:free"
  "openrouter/nvidia/nemotron-3.5-lightning:free"
)

usage() {
  cat <<'EOF'
q — pergunta rápida (streaming, API direta)

Uso: q [-m provider/modelo] [pergunta...]
     q                      prompt interativo
     echo "pergunta" | q    lê do stdin

Modelos: groq/<id> ou openrouter/<id> (IDs do catálogo do provider)
Default: groq/openai/gpt-oss-20b, com fallback automático para free do OpenRouter.
Chaves: GROQ_API_KEY e OPENROUTER_API_KEY (variáveis de ambiente).
Prompt:  base fixa + ~/.config/q/instructions.md (se existir; Q_INSTRUCTIONS muda o caminho).
EOF
}

MODEL="$DEFAULT_MODEL"
while [[ $# -gt 0 ]]; do
  case "$1" in
    -m|--model) [[ -n "${2:-}" ]] || { print -u2 "q: -m exige um modelo"; exit 2 }; MODEL="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    --) shift; break ;;
    -*) print -u2 "q: opção desconhecida: $1"; usage >&2; exit 2 ;;
    *) break ;;
  esac
done

if [[ $# -gt 0 ]]; then
  PROMPT="$*"
elif [[ ! -t 0 ]]; then
  PROMPT="$(cat)"
else
  print -n "❯ "
  read -r PROMPT
fi
[[ -n "${PROMPT//[[:space:]]/}" ]] || { print -u2 "q: pergunta vazia"; exit 2 }

# Resolve "provider/modelo" -> endpoint, chave e id do modelo.
resolve() {
  local spec="$1"
  case "$spec" in
    groq/*)
      ENDPOINT="https://api.groq.com/openai/v1/chat/completions"
      KEY="${GROQ_API_KEY:-}"
      MODEL_ID="${spec#groq/}"
      PROVIDER="groq" ;;
    openrouter/*)
      ENDPOINT="https://openrouter.ai/api/v1/chat/completions"
      KEY="${OPENROUTER_API_KEY:-}"
      MODEL_ID="${spec#openrouter/}"
      PROVIDER="openrouter" ;;
    *)
      if [[ -n "${GROQ_API_KEY:-}" ]]; then
        ENDPOINT="https://api.groq.com/openai/v1/chat/completions"
        KEY="$GROQ_API_KEY"; MODEL_ID="$spec"; PROVIDER="groq"
      else
        ENDPOINT="https://openrouter.ai/api/v1/chat/completions"
        KEY="${OPENROUTER_API_KEY:-}"; MODEL_ID="$spec"; PROVIDER="openrouter"
      fi ;;
  esac
  [[ -n "$KEY" ]] || return 3
}

# Faz uma tentativa com streaming. Imprime os deltas em stdout.
# Define HTTP_CODE e ERROR_BODY (globais).
stream_once() {
  local body http_line="" err="" delta
  body=$(jq -n --arg m "$MODEL_ID" --arg s "$SYSTEM_PROMPT" --arg p "$PROMPT" \
    '{model:$m, messages:[{role:"system",content:$s},{role:"user",content:$p}], stream:true, temperature:0.3, max_tokens:800}')
  if [[ "$PROVIDER" == "groq" && "$MODEL_ID" == openai/gpt-oss-* ]]; then
    body=$(print -r -- "$body" | jq '. + {reasoning_effort:"low"}')
  fi

  curl -sN --max-time 90 "$ENDPOINT" \
    -H "Authorization: Bearer $KEY" \
    -H "Content-Type: application/json" \
    -H "HTTP-Referer: https://opencode.ai" \
    -H "X-Title: q-cli" \
    -d "$body" \
    -w $'\n__HTTP__%{http_code}\n' 2>/dev/null |
  while IFS= read -r line; do
    case "$line" in
      'data: [DONE]'*) ;;
      'data: '*)
        delta=$(print -r -- "${line#data: }" | jq -r '.choices[0].delta.content // empty' 2>/dev/null)
        if [[ -n "$delta" ]]; then
          print -rn -- "$delta"
          PRINTED=1
        fi ;;
      __HTTP__*) http_line="${line#__HTTP__}" ;;
      *) err="$err$line"$'\n' ;;
    esac
  done

  HTTP_CODE="${http_line:-000}"
  ERROR_BODY="$err"
  [[ "$HTTP_CODE" == "200" ]]
}

t0=$EPOCHREALTIME
tried=()
PRINTED=0
ok=0

for spec in "$MODEL" "${FALLBACK_MODELS[@]}"; do
  resolve "$spec" || { ERROR_BODY="chave de API ausente (defina GROQ_API_KEY ou OPENROUTER_API_KEY)"; continue; }
  if stream_once; then
    ok=1
    break
  fi
  tried+=("$spec ($HTTP_CODE)")
  # Só troca de modelo se nada foi impresso ainda (evita resposta duplicada).
  [[ $PRINTED -eq 1 ]] && break
done

elapsed=$(( EPOCHREALTIME - t0 ))
(( ok )) && print
if [[ $ok -eq 1 ]]; then
  [[ -t 2 ]] && print -u2 -P "%F{8}⏱ $(printf '%.2f' $elapsed)s · $PROVIDER/$MODEL_ID%f"
  exit 0
fi
print -u2 "q: falhou em todos os modelos: ${(j:, :)tried}"
[[ -n "${ERROR_BODY:-}" ]] && print -u2 "$ERROR_BODY" | head -5
exit 1
