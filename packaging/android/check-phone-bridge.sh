#!/bin/sh
# Fail if a release APK can hit the launch crash from PR #13:
# a Rust thread called FindClass for se.hallgren.holdover.PhoneBridge, and
# that lookup cannot see app classes. The class has to be in the dex, and
# the native library has to load it through the context class loader.
set -eu

apk="${1:?apk path}"
python3 - "$apk" <<'PY'
import sys
import zipfile

apk = sys.argv[1]
with zipfile.ZipFile(apk) as bundle:
    names = bundle.namelist()
    dex = b"".join(bundle.read(name) for name in names if name.endswith(".dex"))
    native = b"".join(
        bundle.read(name)
        for name in names
        if name.endswith("lib/arm64-v8a/libholdover_lib.so")
    )

missing = []
for needle in (b"Lse/hallgren/holdover/PhoneBridge;",):
    if needle not in dex:
        missing.append(needle)
for needle in (
    b"se.hallgren.holdover.PhoneBridge",
    b"getClassLoader",
    b"()Ljava/lang/ClassLoader;",
    b"loadClass",
    b"(Ljava/lang/String;)Ljava/lang/Class;",
):
    if needle not in native:
        missing.append(needle)

if missing:
    print(
        "PhoneBridge launch check failed. The class must be in the dex, and "
        "libholdover_lib.so must load it with Context.getClassLoader().loadClass. "
        "FindClass on the theme thread crashes launch with ClassNotFoundException.",
        file=sys.stderr,
    )
    for needle in missing:
        print(" missing", needle.decode(), file=sys.stderr)
    sys.exit(1)
print("PhoneBridge is in the dex and the native library uses the app class loader.")
PY
