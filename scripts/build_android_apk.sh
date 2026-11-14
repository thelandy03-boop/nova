#!/usr/bin/env bash
# Script automatizado para compilar y generar el APK de Pro Wave para Android en Linux
set -euo pipefail

TARGET="${1:-universal}" # "universal", "x86_64" o "arm64-v8a"
PROFILE="${2:-release}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${ROOT_DIR}"

export PATH="${HOME}/.cargo/bin:${PATH}"

# Localizar Android SDK y NDK
if [ -z "${ANDROID_HOME:-}" ]; then
    if [ -d "${HOME}/Android/Sdk" ]; then
        export ANDROID_HOME="${HOME}/Android/Sdk"
    elif [ -d "/usr/lib/android-sdk" ]; then
        export ANDROID_HOME="/usr/lib/android-sdk"
    else
        echo "Error: No se encontró ANDROID_HOME. Configura la variable de entorno." >&2
        exit 1
    fi
fi

if [ -z "${NDK_HOME:-}" ]; then
    if [ -d "${ANDROID_HOME}/ndk/26.1.10909125" ]; then
        export NDK_HOME="${ANDROID_HOME}/ndk/26.1.10909125"
    else
        # Tomar el primer NDK disponible
        FIRST_NDK=$(find "${ANDROID_HOME}/ndk" -mindepth 1 -maxdepth 1 -type d | sort -V | tail -n 1)
        if [ -n "${FIRST_NDK}" ]; then
            export NDK_HOME="${FIRST_NDK}"
        else
            echo "Error: No se encontró NDK en ${ANDROID_HOME}/ndk." >&2
            exit 1
        fi
    fi
fi

echo -e "\033[1;36m==========================================================\033[0m"
echo -e "\033[1;36m   Compilando Pro Wave DAW para Android (${TARGET}) en Linux\033[0m"
echo -e "\033[1;36m==========================================================\033[0m"

# Verificar cargo-ndk
if ! command -v cargo-ndk &>/dev/null; then
    echo -e "\033[1;33mInstalando cargo-ndk...\033[0m"
    cargo install cargo-ndk
fi

# Instalar targets de rustup necesarios
if [ "${TARGET}" = "universal" ] || [ "${TARGET}" = "x86_64" ]; then
    rustup target add x86_64-linux-android || true
fi
if [ "${TARGET}" = "universal" ] || [ "${TARGET}" = "arm64-v8a" ]; then
    rustup target add aarch64-linux-android || true
fi

# 1. Compilación de la biblioteca nativa en Rust con cargo-ndk
echo -e "\n\033[1;33m[1/4] Compilando librerias nativas con cargo ndk...\033[0m"
PROFILE_FLAG=""
if [ "${PROFILE}" = "release" ]; then
    PROFILE_FLAG="--release"
fi

TARGETS=()
if [ "${TARGET}" = "universal" ]; then
    TARGETS=("x86_64" "arm64-v8a")
else
    TARGETS=("${TARGET}")
fi

for t in "${TARGETS[@]}"; do
    echo -e " \033[1;36m-> Compilando para ${t}...\033[0m"
    cargo ndk -t "${t}" -P 26 build -p prowave-android ${PROFILE_FLAG}
done

# 2. Preparar empaquetado APK
echo -e "\n\033[1;33m[2/4] Preparando estructura de empaquetado...\033[0m"
SDK="${ANDROID_HOME}"
BUILD_TOOLS="${SDK}/build-tools/34.0.0"
if [ ! -d "${BUILD_TOOLS}" ]; then
    BUILD_TOOLS=$(find "${SDK}/build-tools" -mindepth 1 -maxdepth 1 -type d | sort -V | tail -n 1)
fi

PLATFORM="${SDK}/platforms/android-35/android.jar"
if [ ! -f "${PLATFORM}" ]; then
    PLATFORM=$(find "${SDK}/platforms" -name "android.jar" | sort -V | tail -n 1)
fi

MANIFEST="${ROOT_DIR}/crates/prowave-android/AndroidManifest.xml"
PKG_DIR="${ROOT_DIR}/target/android-pkg"
mkdir -p "${ROOT_DIR}/dist"
mkdir -p "${PKG_DIR}"

for t in "${TARGETS[@]}"; do
    RUST_TARGET="aarch64-linux-android"
    if [ "${t}" = "x86_64" ]; then
        RUST_TARGET="x86_64-linux-android"
    fi
    SUB_DIR="debug"
    if [ "${PROFILE}" = "release" ]; then
        SUB_DIR="release"
    fi
    SO_SRC="${ROOT_DIR}/target/${RUST_TARGET}/${SUB_DIR}/libprowave_android.so"
    DEST_DIR="${PKG_DIR}/lib/${t}"
    mkdir -p "${DEST_DIR}"
    cp -f "${SO_SRC}" "${DEST_DIR}/libprowave_android.so"
done

UNALIGNED="${PKG_DIR}/unaligned.apk"
ALIGNED="${PKG_DIR}/aligned.apk"
FINAL_APK="${ROOT_DIR}/dist/ProWave-${TARGET}.apk"

rm -f "${UNALIGNED}" "${ALIGNED}" "${FINAL_APK}"

# 3. Empaquetar con aapt y zipalign
echo -e "\n\033[1;33m[3/4] Empaquetando recursos y alineando con zipalign...\033[0m"
"${BUILD_TOOLS}/aapt" package -F "${UNALIGNED}" -M "${MANIFEST}" -I "${PLATFORM}" -f

pushd "${PKG_DIR}" > /dev/null
for t in "${TARGETS[@]}"; do
    "${BUILD_TOOLS}/aapt" add "unaligned.apk" "lib/${t}/libprowave_android.so"
done
popd > /dev/null

"${BUILD_TOOLS}/zipalign" -f -p 4 "${UNALIGNED}" "${ALIGNED}"

# 4. Firmar APK
echo -e "\n\033[1;33m[4/4] Firmando APK con apksigner (clave persistente)...\033[0m"
KEYSTORE_DIR="${ROOT_DIR}/crates/prowave-android/keystore"
KEYSTORE="${KEYSTORE_DIR}/prowave-debug.keystore"
mkdir -p "${KEYSTORE_DIR}"

if [ ! -f "${KEYSTORE}" ]; then
    keytool -genkeypair -keystore "${KEYSTORE}" -storepass android -alias androiddebugkey -keypass android -dname "CN=Android Debug,O=Android,C=US" -validity 10000 -keyalg RSA -keysize 2048
fi

"${BUILD_TOOLS}/apksigner" sign --ks "${KEYSTORE}" --ks-pass pass:android --out "${FINAL_APK}" "${ALIGNED}"

if [ "${TARGET}" = "universal" ]; then
    cp -f "${FINAL_APK}" "${ROOT_DIR}/dist/ProWave-arm64-v8a.apk"
fi

APK_SIZE=$(du -h "${FINAL_APK}" | cut -f1)
echo -e "\n\033[1;32m==========================================================\033[0m"
echo -e "\033[1;32m   ¡APK generado con éxito!\033[0m"
echo -e "\033[1;32m   Ubicación: ${FINAL_APK}\033[0m"
echo -e "\033[1;32m   Tamaño: ${APK_SIZE}\033[0m"
echo -e "\033[1;32m==========================================================\033[0m"
