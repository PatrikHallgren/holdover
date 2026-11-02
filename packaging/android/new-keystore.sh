#!/bin/sh
# Create the release keystore Holdover uses for in-place updates on a phone.
# The keystore and the password stay out of git. Back both up. A later APK
# updates the installed app only when it is signed with this same key and its
# versionCode is higher. Losing the key means the next APK cannot replace the
# installed one; the phone would have to uninstall, which deletes the library.
set -eu

dest="${HOLDOVER_KEYSTORE:-${XDG_CONFIG_HOME:-$HOME/.config}/holdover/holdover-release.keystore}"
props="${HOLDOVER_KEYSTORE_PROPS:-$(CDPATH= cd -- "$(dirname "$0")/../.." && pwd)/packaging/android/keystore.properties}"
alias_name="${HOLDOVER_KEY_ALIAS:-holdover}"

if [ -e "$dest" ]; then
  echo "Refusing to overwrite $dest" >&2
  echo "That file is the signing key. Keep it." >&2
  exit 1
fi

if ! command -v keytool >/dev/null 2>&1; then
  echo "keytool is not on PATH. Install a JDK 21 and set JAVA_HOME." >&2
  exit 1
fi

mkdir -p "$(dirname "$dest")"
mkdir -p "$(dirname "$props")"
umask 077

printf "Keystore password (it is not stored in the repository): "
stty -echo
IFS= read -r pass
stty echo
printf "\n"
if [ -z "$pass" ]; then
  echo "A password is required." >&2
  exit 1
fi

keytool -genkeypair -v \
  -keystore "$dest" \
  -storetype PKCS12 \
  -alias "$alias_name" \
  -keyalg RSA -keysize 4096 -validity 10000 \
  -storepass "$pass" -keypass "$pass" \
  -dname "CN=Patrik Hallgren, O=Holdover, C=SE"

cat >"$props" <<EOF
storeFile=$dest
storePassword=$pass
keyAlias=$alias_name
keyPassword=$pass
EOF
chmod 600 "$props" "$dest"
unset pass

echo "Keystore: $dest"
echo "Gradle properties: $props"
echo "Copy the keystore somewhere that survives a disk failure. Do not commit it."
