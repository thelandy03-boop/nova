#!/bin/bash
# Script de ejecución de Nova con comprobación de novedades upstream (SoundCraft y Pro-Wave)

set -e
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Comprobación rápida y silenciosa de upstream
if [ -f "$DIR/scripts/check_upstream.py" ]; then
    CHECK_OUTPUT=$(python3 "$DIR/scripts/check_upstream.py" 2>/dev/null || true)
    if [[ "$CHECK_OUTPUT" == *"UPDATE_FOUND"* ]]; then
        echo -e "\033[1;33m"
        echo "======================================================================"
        echo "🔔 ¡Novedades detectadas en los repositorios de SoundCraft o Pro-Wave!"
        echo "======================================================================"
        echo -e "\033[0m"
    fi
fi

# Ejecutar Nova
exec cargo run --bin prowave "$@"
