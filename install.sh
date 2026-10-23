#!/usr/bin/env bash
# ==============================================================================
# q-rs — Instalação e setup rápido
# ==============================================================================
# Este script instala o q-rs, configura o serviço systemd e cria atalhos.
# Execute como: bash install.sh   (ou ./install.sh após chmod +x)
# ==============================================================================

set -euo pipefail

Q_USER="vinicius"
Q_HOME="/home/${Q_USER}"
Q_PROJECT="${Q_HOME}/q-rs"
Q_BIN="${Q_HOME}/.local/bin"
Q_SKILLS="${Q_HOME}/.config/q/skills"
Q_CONF="${Q_HOME}/.config/q"
Q_SYSTEMD="${Q_HOME}/.config/systemd/user"

echo "=== q-rs: Instalação ==="

# 1. Verificar se já está instalado
if [ -x "${Q_BIN}/q-rs" ]; then
    echo "✅ q-rs já está instalado em ${Q_BIN}/q-rs"
    BINARY_EXISTS=true
else
    BINARY_EXISTS=false
fi

# 2. Criar diretórios necessários
echo "📁 Criando diretórios..."
mkdir -p "${Q_BIN}"
mkdir -p "${Q_SKILLS}"
mkdir -p "${Q_CONF}"
mkdir -p "${Q_SYSTEMD}"

# 3. Instalar binário (se não existir ou rebuild)
if [ "${BINARY_EXISTS}" = false ]; then
    echo "🔨 Construindo q-rs..."
    cd "${Q_PROJECT}"
    cargo build --release 2>&1 | tail -1
    cp "${Q_PROJECT}/target/release/q-rs" "${Q_BIN}/q-rs"
    chmod +x "${Q_BIN}/q-rs"
    echo "✅ Binário instalado: ${Q_BIN}/q-rs"
else
    echo "✅ Binário já existe"
fi

# 4. Instalar skills (se não existirem)
echo "🛠️  Instalando skills..."
for skill in tradutor resumir corretor revisor traduzir auxiliar; do
    if [ ! -f "${Q_SKILLS}/${skill}.md" ]; then
        case "${skill}" in
            tradutor)
                cat > "${Q_SKILLS}/${skill}.md" << 'EOF'
# Tradutor

Traduza o texto de entrada de forma direta, em português.

- Mantenha o tom e contexto original.
- Responda apenas com o texto traduzido.
- Mantenha formatação markdown se o input a utilizar.
EOF
                ;;
            resumir)
                cat > "${Q_SKILLS}/${skill}.md" << 'EOF'
# Resumificador

Resuma o texto de entrada de forma clara e direta, em português.

- Identifique as ideias principais e elimine detalhes desnecessários.
- O resumo deve ter no máximo 150 palavras.
- Use tópicos quando possível para melhor leitura.
- Mantenha o significado original, não adicione opiniões pessoais.
EOF
                ;;
            corretor)
                cat > "${Q_SKILLS}/${skill}.md" << 'EOF'
# Corretor de Gramática

Corrige gramática e melhora a fluência de textos em português.

- Identifique erros de concordância, uso de verbos, pontuação e estrutura de frases.
- Sugira correções mantendo o significado original.
- Mantenha o tom formal ou informal conforme o texto de entrada.
- Responda APENAS com o texto corrigido, sem explicações desnecessárias.
EOF
                ;;
            revisor)
                cat > "${Q_SKILLS}/${skill}.md" << 'EOF'
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
                cat > "${Q_SKILLS}/${skill}.md" << 'EOF'
# Tradutor Bilíngue

Tradução entre português e inglês, preservando o significado e o tom.

- Detecte automaticamente a língua de entrada (PT ou EN).
- Traduza para a língua oposta mantendo contexto e terminologia técnica.
- Para textos técnicos, preserve termos em inglês entre aspas ou itálico.
- Responda com o texto original + tradução separada por "|||".
EOF
                ;;
            auxiliar)
                cat > "${Q_SKILLS}/${skill}.md" << 'EOF'
# Assistente de Tarefas

Ajuda com tarefas cotidianas, brainstorming e organização.

- Forneça sugestões práticas e diretas.
- Divida tarefas complexas em passos menores e acionáveis.
- Use tópicos para melhor legibilidade.
- Se solicitado, gere listas de verificação ou modelos.
EOF
                ;;
        esac
        echo "✅ Skill '${skill}' criada"
    else
        echo "✅ Skill '${skill}' já existe"
    fi
done

# 5. Configurar systemd user service
echo "⚙️  Configurando systemd service..."

# Verificar se o serviço já está configurado
if [ ! -f "${Q_SYSTEMD}/q-rs-serve.service" ]; then
    cat > "${Q_SYSTEMD}/q-rs-serve.service" << EOF
[Unit]
Description=q-rs serve — pergunta rápida (Groq/OpenRouter com fallback)
After=network-online.target

[Service]
Type=simple
ExecStart=${Q_BIN}/q-rs serve --port 8787
EnvironmentFile=${Q_CONF}/env
Restart=on-failure
RestartSec=2

[Install]
WantedBy=default.target
EOF
    echo "✅ Service file criado: ${Q_SYSTEMD}/q-rs-serve.service"
else
    echo "✅ Service file já existe"
fi

# 6. Habilitar service no boot
echo "🚀 Habilitando service no boot..."
systemctl --user daemon-reload 2>/dev/null || echo "   (systemctl --user não disponível neste ambiente)"
systemctl --user enable --now q-rs-serve.service 2>/dev/null || echo "   (iniciando service manualmente)"

# 6. Verificar ambiente de chaves
echo "🔑 Verificando ambiente de chaves..."
if [ -f "${Q_CONF}/env" ]; then
    echo "   ✅ Arquivo de chaves existe: ${Q_CONF}/env"
    # Exportar variáveis de ambiente do arquivo env
    set -a
    source "${Q_CONF}/env"
    set +a
    head -2 "${Q_CONF}/env" 2>/dev/null || echo "   (arquivo vazio)"
else
    echo "⚠️  Arquivo de chaves não encontrado: ${Q_CONF}/env"
    echo "   Crie este arquivo com:"
    echo "   GROQ_API_KEY='sua-chave-groq'"
    echo "   OPENROUTER_API_KEY='sua-chave-openrouter'"
fi

# 7. Resumo final
echo ""
echo "=== q-rs: Instalação Concluída ==="
echo ""
echo "📍 Binário:        ${Q_BIN}/q-rs"
echo "📁 Skills:         ${Q_SKILLS}/"
echo "🌐 Servidor:       http://127.0.0.1:8787 (ou IP do Tailscale)"
echo "📦 Skills disponíveis:"
for s in tradutor resumir corretor revisor traduzir auxiliar; do
    if [ -f "${Q_SKILLS}/${s}.md" ]; then
        echo "   - ${s}"
    fi
done
echo ""
echo "📦 Comandos úteis:"
echo "   q-rs <pergunta>                     # Pergunta direta"
echo "   q-rs -s tradutor \"texto\"              # Usar skill"
echo "   q-rs tool shell \"comando\"             # Executar tool shell"
echo "   q-rs skills                           # Listar skills"
echo "   systemctl --user status q-rs-serve  # Status do serviço"
echo ""
echo "=== Pronto! ==="

