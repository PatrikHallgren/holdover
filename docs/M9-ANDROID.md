# Milestone 9 Android companion

This is the same Tauri 2 app as the desktop, built for Android. Solve, the range card, profiles, Field, group photographs, and Sync are on the phone. Ballistics and group statistics stay in `holdover-core`. The `.holdover` bundle is the one from Milestone 7. iOS is out of scope.

Version 1.4.0 is the number in the crate, the window title, and the PKGBUILD. `versionCode` is `major * 1000000 + minor * 1000 + patch`, so 1.4.0 is 1004000. No tag and no release are created by this change.

The phone this build is for is a OnePlus 13, model CPH2655, Android 16 (API 36), `arm64-v8a`. `compileSdk` and `targetSdk` are 36. `minSdk` is 26. The APK contains only `arm64-v8a`.

## Signing

An update installs over the existing app, and keeps the library, only when two things are true: the new APK is signed with the **same key** as the one already on the phone, and its `versionCode` is higher.

The release key is yours. It is not in the repository. Generate it once, on the Omarchy machine, and keep a copy somewhere that survives a disk failure. Losing it means the next APK cannot replace the installed one. Uninstalling to switch keys deletes the phone library, so export a bundle first.

```sh
bash packaging/android/new-keystore.sh
```

That writes the keystore to `~/.config/holdover/holdover-release.keystore` and the passwords to `packaging/android/keystore.properties`. Both are gitignored. `packaging/android/keystore.properties.example` shows the shape, with no real password.

CI signs with `packaging/android/ci-test.keystore`. The password `holdover-ci` and the alias `holdover-ci` are public on purpose. That key is only for the CI artifact. It cannot update an APK you signed with the release key, and a release APK cannot update a phone that already has the CI build. Install the release-signed APK first on the phone you actually use. Use a CI APK only to check that a file installs, or uninstall before switching keys.

## SDK on the Omarchy machine

`adb` is already at `/usr/bin/adb`. The Android SDK and NDK are not. Rust stays mise's `rust@1.98.1`. Java 21 can come from mise or from the distro JDK.

```sh
sudo pacman -S --needed jdk21-openjdk unzip curl
mise install rust@1.98.1
mise exec rust@1.98.1 -- rustup target add aarch64-linux-android
export JAVA_HOME=/usr/lib/jvm/java-21-openjdk
mise exec rust@1.98.1 -- bash packaging/android/setup-sdk.sh
mise exec rust@1.98.1 -- bash packaging/android/install-tauri-cli.sh
```

`setup-sdk.sh` installs command-line tools, platform `android-36`, build-tools `36.0.0`, and NDK `29.0.13846066` under `~/Android/Sdk`, then writes `~/.config/holdover/android-env.sh`. If mise provides the JDK instead of pacman:

```sh
mise install java@temurin-21
export JAVA_HOME="$(mise where java@temurin-21)"
```

Source the env file in a later shell:

```sh
. ~/.config/holdover/android-env.sh
```

## Build and install

From the repo root, on this branch, after the SDK step and after `new-keystore.sh`:

```sh
. ~/.config/holdover/android-env.sh
bash packaging/android/build-apk.sh
```

The script prints the APK path. It is under `app/src-tauri/gen/android/app/build/outputs/apk/arm64/release/` and the name ends in `arm64-release.apk`. Plug the phone in. `adb devices` should list the CPH2655 as `device`. If it says `unauthorized`, accept the prompt on the phone.

```sh
adb install -r app/src-tauri/gen/android/app/build/outputs/apk/arm64/release/app-arm64-release.apk
```

`-r` replaces an app signed with the same key. The first install can also be done by copying the APK onto the phone and opening it, after the file manager is allowed to install unknown apps. There is no Play Store build.

A later 1.4.x or 1.5.0 APK, signed with the same key, installs the same way and keeps the library in the app's private storage. The bundle export is the backup.

The desktop package is unchanged in kind. From `packaging/`:

```sh
sudo pacman -S --needed base-devel webkit2gtk-4.1 gtk3 openssl hicolor-icon-theme
mise exec rust@1.98.1 -- makepkg -si
```

The recipe version is 1.4.0. This does not create a tag.

## What to check

- **Install.** About on the phone reads 1.4.0 and mentions arm64. Settings, Follow phone, tracks the phone's dark and light settings. Light and Dark still switch the bundled palettes. Type stays readable on both. The screen row is the page name with ▾. Every control you use has a visible label. Metric, US, and Custom are on the page. Metric speeds are m/s.
- **Solve and cards.** Finish onboarding and add the example profile. Solve shows a hold. Range card still builds the PDF. Profiles edit and save. Swedish targets are first in the target list. T2–T6 and the US rows keep their placeholder badges. Nothing on Targets invents a diameter.
- **Field.** Field, Measure with this phone. Allow location when asked, then measure again. Station pressure, altitude, azimuth, inclination, and cant that the phone actually returned are labelled Measured (phone). Temperature and humidity stay what you type. Use this air writes the current air. A chronograph string shows the mean in m/s, and in the selected velocity unit when that is not m/s. Open in truing fills the distance and the elevation correction and does not write a fit by itself. Save this fix as the weather place stays off unless you check it.
- **Group, same photograph as the desktop.** On the desktop, `mise exec rust@1.98.1 -- ./target/release/holdover-cli group-check` still prints that the synthetic photograph passed. Copy `tests/fixtures/synthetic-group.png` to the phone (`adb push tests/fixtures/synthetic-group.png /sdcard/Download/`). Groups, import that file (Take photo is the camera; the file button is the import). Calibrate the same way as Milestone 8: 200 mm by 280 mm, aim point at the centre, distance 100 m, five shots, bullet 7.82 mm, corners in order. The extreme spread, mean radius, and offset match the desktop for that file, within the 1% already required. Take photo uses the camera and the same import command.
- **Sync round trip.** On the desktop, with a throwaway library:

  ```sh
  mkdir -p /tmp/holdover-m9
  HOLDOVER_DB=/tmp/holdover-m9/desktop.db ./target/release/holdover
  ```

  Finish onboarding, add the example, Sync, Export, coordinates off. Note the `.holdover` path. Copy it to the phone (`adb push` into `/sdcard/Download/`, or LocalSend). On the phone, Sync, Import, choose that file. Apply. The example profile is there. Change a profile name on the phone, Sync, Export. The share sheet is how the file leaves the phone (LocalSend, Drive, or a cable copy of the shared file). On the desktop, import that bundle into `/tmp/holdover-m9/desktop.db` or a second `HOLDOVER_DB`. The phone's edit shows as Updated or Conflict, using the same Keep mine / Take theirs / Keep both choices. A second import of an unchanged file stays Unchanged. The phone's device id does not replace the desktop's.

- **CI artifact.** The workflow uploads `holdover-android-arm64`. That APK is signed with the public test key. Do not install it over the release-signed app if you want to keep updating in place.

## Not in this build

No LAN sync. That is Milestone 10. No iOS. No Swedish locale and no extra text-size control. T2–T6 and the US rulebook targets stay unmeasured. HEIC is still refused. The phone does not read a temperature or humidity sensor. A barometer reading is station pressure only. Group scores still use published rings only.
