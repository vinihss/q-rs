#!/usr/bin/env bash
set -euo pipefail

# This project is source code, not a system-wide installer.
# Run this script from the repository root to remove build artifacts only.
if command -v cargo >/dev/null 2>&1; then
  cargo clean
else
  rm -rf target
fi

echo "Artefatos de compilação removidos. Os fontes e configurações foram preservados."
