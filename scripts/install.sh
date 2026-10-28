#!/usr/bin/env sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Install a checksum-verified native package, with an AppImage fallback.
# VERSION=0.1.8 or PREFIX="$HOME/.local" explicitly selects the AppImage.
set -eu
REPO=tulcoturo/clovercut
need() { command -v "$1" >/dev/null 2>&1 || { echo "install: need $1" >&2; exit 1; }; }
die() { echo "install: $*" >&2; exit 1; }
have() { command -v "$1" >/dev/null 2>&1; }
[ "$(uname -s)" = Linux ] || die 'this installer is for Linux'
case "$(uname -m)" in
 x86_64) arch=x86_64;;
 aarch64|arm64) arch=aarch64;;
 *) die 'unsupported architecture';;
esac
need curl
need sha256sum
explicit_version=${VERSION:-}
if [ -n "$explicit_version" ]; then
 tag=v${explicit_version#v}
else
 tag=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -1)
fi
printf '%s\n' "$tag" | grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$' || die 'invalid release tag'
ver=${tag#v}
base=https://github.com/$REPO/releases/download/$tag
work=$(mktemp -d)
cleanup() { rm -rf "$work"; if [ -n "${pending:-}" ]; then rm -f "$pending"; fi; }
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
curl -fsSL "$base/SHA256SUMS" -o "$work/SHA256SUMS"
checksum() {
 awk -v file="$1" '$2 == file || $2 == "*" file {print $1}' "$work/SHA256SUMS"
}
download() {
 file=Clovercut-$ver-$arch.$1
 sum=$(checksum "$file")
 printf '%s\n' "$sum" | grep -Eq '^[0-9a-fA-F]{64}$' || die "missing or invalid checksum: $file"
 curl -fsSL "$base/$file" -o "$work/$file"
 printf '%s  %s\n' "$sum" "$work/$file" | sha256sum -c - >/dev/null || die "checksum mismatch: $file"
}
privileged() {
 if [ "$(id -u)" -eq 0 ]; then "$@"
 else need sudo; sudo "$@"; fi
}
native_launcher() {
 # An earlier portable install can precede /usr/bin in PATH. Keep custom
 # launchers intact and identify the executable the shell will actually use.
 launcher=$(command -v clovercut || :)
 case "$launcher" in
  ''|/usr/bin/clovercut|/bin/clovercut) ;;
  *) echo "Your PATH selects $launcher. Run /usr/bin/clovercut to use the native package; review the existing launcher before removing it.";;
 esac
}
appimage() {
 download AppImage
 if [ -n "${PREFIX:-}" ]; then prefix=$PREFIX
 elif [ "$(id -u)" -eq 0 ]; then prefix=/usr/local
 else prefix=$HOME/.local; fi
 mkdir -p "$prefix/bin"
 pending=$(mktemp "$prefix/bin/.clovercut.XXXXXX")
 cp "$work/$file" "$pending"
 chmod 755 "$pending"
 mv -f "$pending" "$prefix/bin/clovercut"
 pending=
 echo "Installed Clovercut $ver at $prefix/bin/clovercut"
 case ":$PATH:" in *":$prefix/bin:"*) ;; *) echo "Add $prefix/bin to PATH to run clovercut.";; esac
}
# AUR helpers cannot pin an arbitrary historical release, so honour explicit
# version/prefix requests through the verified AppImage instead.
if [ -n "$explicit_version" ] || [ -n "${PREFIX:-}" ]; then appimage; exit 0; fi
if [ -f /etc/os-release ]; then . /etc/os-release; distro="${ID_LIKE:-} ${ID:-}"; else distro=; fi
case "$distro" in
 *arch*|*cachyos*|*manjaro*|*endeavouros*)
  if [ "$(id -u)" -ne 0 ]; then
   for helper in yay paru; do
    if have "$helper"; then
     # A release may precede AUR publication. Never silently install an older one.
     aur_version=$(curl -fsSL --connect-timeout 10 --max-time 20 \
       'https://aur.archlinux.org/rpc/v5/info?arg%5B%5D=clovercut-bin' 2>/dev/null \
       | sed -n 's/.*"Version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')
     case "$aur_version" in
      "$ver"-*) "$helper" -S clovercut-bin; echo 'AUR installation complete. Run clovercut.'; native_launcher; exit 0;;
      *) echo "AUR version $ver is not confirmed; using the verified AppImage."; break;;
     esac
    fi
   done
  fi;;
 *debian*|*ubuntu*|*mint*|*pop*)
  if have apt-get && [ -n "$(checksum "Clovercut-$ver-$arch.deb")" ]; then
   download deb; privileged apt-get install -y "$work/$file"; native_launcher; exit 0
  fi;;
 *fedora*|*rhel*|*centos*|*suse*|*opensuse*)
  if [ -n "$(checksum "Clovercut-$ver-$arch.rpm")" ]; then
   if have dnf; then download rpm; privileged dnf install -y "$work/$file"; native_launcher; exit 0
   elif have zypper; then download rpm; privileged zypper --non-interactive install "$work/$file"; native_launcher; exit 0; fi
  fi;;
esac
echo 'Using the portable AppImage (no supported native package/manager for this release).'
appimage
