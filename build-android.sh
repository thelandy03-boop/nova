#!/usr/bin/env bash
# Script para compilar el APK de Android de Pro Wave en Linux
set -e
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
bash "$DIR/scripts/build_android_apk.sh" "$@"
