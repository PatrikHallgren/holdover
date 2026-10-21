#!/bin/bash
# Build the Holdover package inside an Arch container. The workflow mounts
# the repository at /src and runs this file with the Arch image.
set -euo pipefail
pacman -Syu --noconfirm
pacman -S --noconfirm rust cargo webkit2gtk-4.1 gtk3 openssl pkgconf
if ! id builder >/dev/null 2>&1; then
  useradd -m builder
fi
echo "builder ALL=(ALL) NOPASSWD: ALL" >> /etc/sudoers
chown -R builder /src
su builder -c "cd /src/packaging && makepkg --noconfirm --skipinteg"
mapfile -t archives < <(ls /src/packaging/holdover-*.pkg.tar.*)
test "${#archives[@]}" -ge 1
tar -tf "${archives[0]}" > /tmp/holdover-pkg-list
need=(
  usr/bin/holdover
  usr/bin/holdover-cli
  usr/share/applications/se.hallgren.holdover.desktop
  usr/share/icons/hicolor/scalable/apps/se.hallgren.holdover.svg
  usr/share/holdover/holdover.json.tpl
  usr/share/licenses/holdover/LICENSE
)
for path in "${need[@]}"; do
  grep -qx "$path" /tmp/holdover-pkg-list
done
echo "package contains the window, the CLI, the desktop entry, the icon, the theme template, and the MIT licence"
