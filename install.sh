#!/usr/bin/env bash
# ==============================================================================
# q-rs — instalação
# ==============================================================================
# Instala o binário `q-rs` no PATH do usuário, cria as configurações em
# ~/.config/q-rs (ou $XDG_CONFIG_HOME/q-rs) e, opcionalmente, registra o
# servidor como daemon systemd de usuário (perguntando a porta).
#
# Uso:
#   bash install.sh [--prefix DIR] [--port N] [--no-daemon] [--rebuild]
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

Q_PREFIX="${Q_PREFIX:-${HOME}/.local}"
Q_CONFIG_DIR="${Q_RS_CONFIG_DIR:-${XDG_CONFIG_HOME:-${HOME}/.config}/q-rs}"
Q_LEGACY_DIR="${HOME}/.config/q"
Q_PORT=""
WITH_DAEMON=1
REBUILD=0

usage() {
    cat <<'EOF'
q-rs — instalação

Uso: bash install.sh [opções]

Opções:
  --prefix DIR    instala o binário em DIR/bin (padrão: ~/.local)
  --port N        porta do servidor (sem a opção, o CLI pergunta)
  --no-daemon     não registra/inicia o servidor como daemon
  --rebuild       recompila e reinstala o binário mesmo se já existir
  -h, --help      esta ajuda
EOF
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --prefix)
            [ "$#" -ge 2 ] || { echo "erro: --prefix exige um diretório" >&2; exit 2; }
            Q_PREFIX="$2"
            shift 2
            ;;
        --prefix=*)
            Q_PREFIX="${1#*=}"
            shift
            ;;
        --port)
            [ "$#" -ge 2 ] || { echo "erro: --port exige um número" >&2; exit 2; }
            Q_PORT="$2"
            shift 2
            ;;
        --port=*)
            Q_PORT="${1#*=}"
            shift
            ;;
        --no-daemon)
            WITH_DAEMON=0
            shift
            ;;
        --rebuild)
            REBUILD=1
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "erro: opção desconhecida: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

if [ -n "${Q_PORT}" ]; then
    case "${Q_PORT}" in
        ''|*[!0-9]*) echo "erro: porta inválida: ${Q_PORT}" >&2; exit 2 ;;
    esac
    if [ "${Q_PORT}" -eq 0 ]; then
        echo "erro: porta inválida: 0" >&2
        exit 2
    fi
fi

Q_BIN_DIR="${Q_PREFIX}/bin"
Q_BINARY="${Q_BIN_DIR}/q-rs"

echo "=== q-rs: instalação ==="
echo "📍 prefixo:    ${Q_PREFIX}"
echo "📁 config:     ${Q_CONFIG_DIR}"

# ------------------------------------------------------------------------------
# 1. Binário
# ------------------------------------------------------------------------------
mkdir -p "${Q_BIN_DIR}"

if [ -x "${Q_BINARY}" ] && [ "${REBUILD}" -eq 0 ]; then
    echo "✅ binário já instalado: ${Q_BINARY} (use --rebuild para recompilar)"
else
    if ! command -v cargo >/dev/null 2>&1; then
        echo "erro: cargo não encontrado no PATH; instale o toolchain Rust" >&2
        exit 1
    fi
    echo "🔨 compilando (cargo build --release -p q-cli)..."
    ( cd "${SCRIPT_DIR}" && cargo build --release -p q-cli )
    install -m 0755 "${SCRIPT_DIR}/target/release/q-rs" "${Q_BINARY}"
    echo "✅ binário instalado: ${Q_BINARY}"
fi

# ------------------------------------------------------------------------------
# 2. Configuração em ~/.config/q-rs
# ------------------------------------------------------------------------------
mkdir -p "${Q_CONFIG_DIR}/skills"

if [ -f "${Q_LEGACY_DIR}/env" ] && [ ! -f "${Q_CONFIG_DIR}/env" ]; then
    cp "${Q_LEGACY_DIR}/env" "${Q_CONFIG_DIR}/env"
    chmod 600 "${Q_CONFIG_DIR}/env"
    echo "↪️  chaves migradas de ${Q_LEGACY_DIR}/env"
fi

if [ -d "${Q_LEGACY_DIR}/skills" ] && [ -z "$(find "${Q_CONFIG_DIR}/skills" -name '*.md' -print -quit 2>/dev/null)" ]; then
    cp -n "${Q_LEGACY_DIR}"/skills/*.md "${Q_CONFIG_DIR}/skills/" 2>/dev/null || true
    echo "↪️  skills migradas de ${Q_LEGACY_DIR}/skills"
fi

if [ ! -f "${Q_CONFIG_DIR}/env" ]; then
    umask 077
    cat > "${Q_CONFIG_DIR}/env" << 'EOF'
# Chaves de API do q-rs — variáveis de ambiente reais têm prioridade.
# GROQ_API_KEY=
# OPENROUTER_API_KEY=
EOF
    chmod 600 "${Q_CONFIG_DIR}/env"
    echo "✅ criado ${Q_CONFIG_DIR}/env (adicione suas chaves)"
else
    chmod 600 "${Q_CONFIG_DIR}/env" 2>/dev/null || true
    echo "✅ mantido ${Q_CONFIG_DIR}/env existente"
fi

# ------------------------------------------------------------------------------
# 3. Skills
# ------------------------------------------------------------------------------
echo "🛠️  instalando skills..."
for skill in tradutor resumir corretor revisor traduzir auxiliar; do
    target="${Q_CONFIG_DIR}/skills/${skill}.md"
    if [ -f "${target}" ]; then
        echo "✅ skill '${skill}' já existe"
        continue
    fi
    case "${skill}" in
        tradutor)
            cat > "${target}" << 'EOF'
# Tradutor

Traduza o texto de entrada de forma direta, em português.

- Mantenha o tom e contexto original.
- Responda apenas com o texto traduzido.
- Mantenha formatação markdown se o input a utilizar.
EOF
            ;;
        resumir)
            cat > "${target}" << 'EOF'
# Resumificador

Resuma o texto de entrada de forma clara e direta, em português.

- Identifique as ideias principais e elimine detalhes desnecessários.
- O resumo deve ter no máximo 150 palavras.
- Use tópicos quando possível para melhor leitura.
- Mantenha o significado original, não adicione opiniões pessoais.
EOF
            ;;
        corretor)
            cat > "${target}" << 'EOF'
# Corretor de Gramática

Corrige gramática e melhora a fluência de textos em português.

- Identifique erros de concordância, uso de verbos, pontuação e estrutura de frases.
- Sugira correções mantendo o significado original.
- Mantenha o tom formal ou informal conforme o texto de entrada.
- Responda APENAS com o texto corrigido, sem explicações desnecessárias.
EOF
            ;;
        revisor)
            cat > "${target}" << 'EOF'
# Revisor de Código

Revisa código em busca de melhorias, bugs e boas práticas.

- Analise o código fornecido com foco em clareza, eficiência e manutenibilidade.
- Identifique possíveis bugs, código morto ou anti-padrões.
- Sugira melhorias com exemplos de código quando relevante.
- Priorize mudanças que tenham impacto medível (performance, legibilidade).
- Responda em português, mantendo o estilo do código original quando sugerir mudanças.
EOF
            ;;
        traduzir)
            cat > "${target}" << 'EOF'
# Tradutor Bilíngue

Tradução entre português e inglês, preservando o significado e o tom.

- Detecte automaticamente a língua de entrada (PT ou EN).
- Traduza para a língua oposta mantendo contexto e terminologia técnica.
- Para textos técnicos, preserve termos em inglês entre aspas ou itálico.
- Responda com o texto original + tradução separada por "|||".
EOF
            ;;
        auxiliar)
            cat > "${target}" << 'EOF'
# Assistente de Tarefas

Ajuda com tarefas cotidianas, brainstorming e organização.

- Forneça sugestões práticas e diretas.
- Divida tarefas complexas em passos menores e acionáveis.
- Use tópicos para melhor legibilidade.
- Se solicitado, gere listas de verificação ou modelos.
EOF
            ;;
    esac
    echo "✅ skill '${skill}' criada"
done

# ------------------------------------------------------------------------------
# 4. PATH
# ------------------------------------------------------------------------------
case ":${PATH}:" in
    *":${Q_BIN_DIR}:"*) ;;
    *)
        echo "⚠️  ${Q_BIN_DIR} não está no PATH. Adicione ao seu shell:"
        echo "    export PATH=\"${Q_BIN_DIR}:\$PATH\""
        ;;
esac

# ------------------------------------------------------------------------------
# 5. Daemon (systemd de usuário, via CLI)
# ------------------------------------------------------------------------------
if [ "${WITH_DAEMON}" -eq 1 ]; then
    echo "⚙️  configurando servidor (daemon)..."
    daemon_args=(server install)
    if [ -n "${Q_PORT}" ]; then
        daemon_args+=(--port "${Q_PORT}")
    fi
    if ! Q_RS_CONFIG_DIR="${Q_CONFIG_DIR}" "${Q_BINARY}" "${daemon_args[@]}"; then
        echo "⚠️  não foi possível registrar o daemon; o binário está instalado." >&2
        echo "    Rode manualmente: ${Q_BINARY} server install" >&2
    fi
else
    echo "⏭️  daemon ignorado (--no-daemon); use 'q-rs server install' depois"
fi

# ------------------------------------------------------------------------------
# 6. Resumo
# ------------------------------------------------------------------------------
Q_HOST="0.0.0.0"
Q_SERVER_PORT="${Q_PORT:-8787}"
if [ -f "${Q_CONFIG_DIR}/config" ]; then
    while IFS='=' read -r key value; do
        case "${key}" in
            Q_RS_HOST) Q_HOST="${value}" ;;
            Q_RS_PORT) Q_SERVER_PORT="${value}" ;;
        esac
    done < "${Q_CONFIG_DIR}/config"
fi

echo ""
echo "=== q-rs: instalação concluída ==="
echo ""
echo "📍 binário:     ${Q_BINARY}"
echo "📁 config:      ${Q_CONFIG_DIR}"
echo "🔑 chaves:      ${Q_CONFIG_DIR}/env"
echo "🧩 skills:      ${Q_CONFIG_DIR}/skills/"
echo "🌐 servidor:    http://${Q_HOST}:${Q_SERVER_PORT} (bind ${Q_HOST} — acessível na rede)"
echo "📝 log:         ${Q_CONFIG_DIR}/server.log"
echo ""
echo "📦 comandos úteis:"
echo "   q-rs run \"<texto>\"                      # pergunta direta"
echo "   q-rs --prompt \"<system>\" run \"<texto>\"   # com system prompt"
echo "   q-rs                                    # modo interativo"
echo "   q-rs server start|stop|status           # daemon"
echo "   q-rs --help                             # ajuda completa"
echo ""
echo "=== pronto! ==="
