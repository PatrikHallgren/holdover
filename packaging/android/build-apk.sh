#!/bin/sh
# Build the arm64-v8a release APK.
# Patrik's machine signs with packaging/android/keystore.properties.
# CI sets HOLDOVER_ANDROID_CI=1 and signs with the public test key
# packaging/android/ci-test.keystore. That test key is not the phone's update key.
set -eu

root="$(CDPATH= cd -- "$(dirname "$0")/../.." && pwd)"
cd "$root"

env_file="${XDG_CONFIG_HOME:-$HOME/.config}/holdover/android-env.sh"
if [ -f "$env_file" ]; then
  # shellcheck disable=SC1090
  . "$env_file"
fi

if [ -z "${ANDROID_HOME:-}" ] || [ -z "${NDK_HOME:-}" ] || [ -z "${JAVA_HOME:-}" ]; then
  echo "ANDROID_HOME, NDK_HOME, and JAVA_HOME must be set. Run packaging/android/setup-sdk.sh." >&2
  exit 1
fi

if ! command -v cargo-tauri >/dev/null 2>&1 && ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is not on PATH." >&2
  exit 1
fi
if ! cargo tauri --version >/dev/null 2>&1; then
  echo "cargo-tauri 2.12.x is not installed. packaging/android/install-tauri-cli.sh fetches the 2.12.4 binary." >&2
  exit 1
fi

props_src="$root/packaging/android/keystore.properties"
props_dest="$root/app/src-tauri/gen/android/keystore.properties"
if [ -f "$props_src" ]; then
  cp "$props_src" "$props_dest"
elif [ "${HOLDOVER_ANDROID_CI:-}" = "1" ]; then
  cat >"$props_dest" <<EOF
storeFile=$root/packaging/android/ci-test.keystore
storePassword=holdover-ci
keyAlias=holdover-ci
keyPassword=holdover-ci
EOF
else
  echo "No packaging/android/keystore.properties." >&2
  echo "Run packaging/android/new-keystore.sh and keep that key. An APK signed with another key cannot update the phone in place." >&2
  exit 1
fi
chmod 600 "$props_dest"

cd "$root/app/src-tauri"
cargo tauri android build --target aarch64 --apk --split-per-abi --ci

apk="$(find "$root/app/src-tauri/gen/android/app/build/outputs/apk" -name '*.apk' | head -n 1)"
echo "APK: ${apk:-not found}"
