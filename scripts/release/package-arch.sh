#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
version=$(python3 "$root/scripts/release/check-version.py")
# Arch disallows '-' in pkgver; alpha sorts before the corresponding stable version.
export RELEASE_VERSION=${version/-/}
mkdir -p "$root/release-assets"
docker run --rm \
  -e RELEASE_VERSION -e "OUTPUT_UID=$(id -u)" -e "OUTPUT_GID=$(id -g)" \
  -v "$root:/workspace:ro" -v "$root/release-assets:/output" \
  archlinux:base-devel@sha256:996c3a1d6b0d87b01242f6fcd8cfa3ad3eece1a67ab5c8f108e20af1d7b97bdd \
  bash -euo pipefail -c '
    useradd -m builder
    install -d -o builder -g builder /tmp/package
    cp /workspace/packaging/arch/PKGBUILD /tmp/package/
    cd /tmp/package
    mapfile -t debs < <(find /workspace/release-assets -maxdepth 1 -name "*.deb")
    test "${#debs[@]}" = 1
    bsdtar -xf "${debs[0]}"
    mkdir payload
    bsdtar -xf data.tar.* -C payload
    chown -R builder:builder /tmp/package
    su builder -c "cd /tmp/package && makepkg --nodeps --noconfirm"
    pacman -Syu --noconfirm
    pacman -U --noconfirm ./*.pkg.tar.zst
    pacman -Qk mc-gui-crafter
    test -x /usr/bin/mc-gui-crafter
    if ldd /usr/bin/mc-gui-crafter | grep "not found"; then exit 1; fi
    pacman -R --noconfirm mc-gui-crafter
    cp ./*.pkg.tar.zst /output/
    chown "$OUTPUT_UID:$OUTPUT_GID" /output/*.pkg.tar.zst
  '
