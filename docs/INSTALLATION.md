# Linux installation

Download packages from [GitHub Releases](https://github.com/tulcoturo/clovercut/releases). Release 0.2.0 provides AppImage, `.deb` and `.rpm` packages for x86_64 and ARM64 (aarch64).

One-line install (picks the native package for your distro, falls back to the AppImage):

```bash
curl -fsSL https://raw.githubusercontent.com/tulcoturo/clovercut/main/scripts/install.sh | sh
```

On Arch-based distributions, an AUR helper is used only if the AUR reports the
selected release version. If the AUR is behind or unavailable, the installer uses
the checksum-verified AppImage instead.

When moving from an AppImage to a native package, an existing launcher in
`~/.local/bin` or `/usr/local/bin` may still come first in `PATH`. The installer
reports that path and preserves it. Run `/usr/bin/clovercut` to use the native
package, then review the older launcher before removing it.

Manual installation (x86_64 examples; use `aarch64` for ARM64):

```bash
# Debian / Ubuntu / Mint / Pop!_OS
curl -fsSLO https://github.com/tulcoturo/clovercut/releases/download/v0.2.0/Clovercut-0.2.0-x86_64.deb
sudo apt install ./Clovercut-0.2.0-x86_64.deb

# Fedora / RHEL
curl -fsSLO https://github.com/tulcoturo/clovercut/releases/download/v0.2.0/Clovercut-0.2.0-x86_64.rpm
sudo dnf install ./Clovercut-0.2.0-x86_64.rpm
# openSUSE: sudo zypper install ./Clovercut-0.2.0-x86_64.rpm

# Arch / CachyOS / Manjaro / EndeavourOS (prebuilt AppImage repackaged, updated by CI when the AUR maintainer key is configured)
yay -S clovercut-bin
# or: paru -S clovercut-bin
```

## Run an AppImage

```bash
chmod +x Clovercut-0.2.0-x86_64.AppImage
./Clovercut-0.2.0-x86_64.AppImage
```

*(On systems without FUSE installed, launch with `APPIMAGE_EXTRACT_AND_RUN=1`).*

## Software rendering

If graphics drivers cause startup issues:

```bash
clovercut --software
```
This forces software rasterization for the session without altering your system GPU settings.


## Packaging and AUR maintenance

Packagers: [build-deb.sh](../scripts/build-deb.sh) and [build-rpm.sh](../scripts/build-rpm.sh) build both formats from the staged Linux folder with only `dpkg-deb` / `rpmbuild` ([nfpm.yaml](../assets/linux/nfpm.yaml) provides an optional equivalent nfpm description). The AUR package lives in [assets/arch](../assets/arch/) (`PKGBUILD` + `.SRCINFO`, package name `clovercut-bin`).

AUR automation requires the repository secret `AUR_SSH_KEY` for an account with write access to `clovercut-bin`. It follows successful releases without modifying release tags. Explicit `VERSION` or `PREFIX` installer overrides use the verified AppImage directly.

