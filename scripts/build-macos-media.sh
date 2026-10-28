#!/usr/bin/env bash
# Rebuild the editor's media runtime for Monterey from checksum-pinned sources.
# Homebrew supplies build tools only; none of its runtime bottles may leak in.
# Usage: build-macos-media.sh <aarch64-apple-darwin|x86_64-apple-darwin> <new-prefix>
set -euo pipefail
[ "$#" -eq 2 ] || { echo "usage: $0 <target> <new-prefix>" >&2; exit 2; }
[ "$(uname -s)" = Darwin ] || { echo 'macOS/Xcode build tools required' >&2; exit 2; }
target=$1
case "$target" in
  aarch64-apple-darwin) arch=arm64; vpx_target=arm64-darwin-gcc;;
  x86_64-apple-darwin) arch=x86_64; vpx_target=x86_64-darwin20-gcc;;
  *) echo "unsupported target: $target" >&2; exit 2;;
esac
[ "$(uname -m)" = "$arch" ] || { echo 'use a native runner for this architecture' >&2; exit 2; }
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
mkdir -p "$2"
prefix=$(cd "$2" && pwd)
[ -z "$(ls -A "$prefix")" ] || { echo 'use an empty media prefix; caches are restored by the workflow' >&2; exit 2; }
export MACOSX_DEPLOYMENT_TARGET=${MACOSX_DEPLOYMENT_TARGET:-12.0}
[ "$MACOSX_DEPLOYMENT_TARGET" = 12.0 ] || { echo 'this recipe qualifies macOS 12.0 only' >&2; exit 2; }
jobs=${CLOVERCUT_MEDIA_JOBS:-2}
sdk=$(xcrun --sdk macosx --show-sdk-path)
export CC="$(xcrun --find clang)"
export CXX="$(xcrun --find clang++)"
export CFLAGS="-O2 -fPIC -mmacosx-version-min=12.0 -isysroot $sdk -Werror=unguarded-availability-new"
export CXXFLAGS="$CFLAGS"
export LDFLAGS="-mmacosx-version-min=12.0 -isysroot $sdk -L$prefix/lib"
# Only the source-built codec libraries may satisfy external dependencies.
unset CPATH C_INCLUDE_PATH CPLUS_INCLUDE_PATH LIBRARY_PATH CMAKE_PREFIX_PATH PKG_CONFIG_PATH
export PKG_CONFIG_LIBDIR="$prefix/lib/pkgconfig"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$prefix/lib" "$prefix/include" "$prefix/share/licenses"

source_of() {
  local name=$1 strip=1
  "$root/scripts/ci/fetch.sh" "macos-source-$name" "$work/$name.tar"
  mkdir -p "$work/$name"
  [ "$name" != aom ] || strip=0
  tar -xf "$work/$name.tar" -C "$work/$name" --strip-components="$strip"
  local license
  for license in "$work/$name"/COPYING* "$work/$name"/LICENSE* "$work/$name"/PATENTS* "$work/$name"/AUTHORS* "$work/$name"/NOTICE*; do
    [ -f "$license" ] || continue
    mkdir -p "$prefix/share/licenses/$name"
    cp "$license" "$prefix/share/licenses/$name/"
  done
}
cmake_options=(
  -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX="$prefix"
  -DCMAKE_INSTALL_LIBDIR=lib -DCMAKE_POSITION_INDEPENDENT_CODE=ON
  -DCMAKE_OSX_DEPLOYMENT_TARGET=12.0 -DCMAKE_OSX_ARCHITECTURES="$arch"
  -DCMAKE_OSX_SYSROOT="$sdk" -DCMAKE_POLICY_VERSION_MINIMUM=3.5
  -DCMAKE_C_FLAGS="$CFLAGS" -DCMAKE_CXX_FLAGS="$CXXFLAGS"
  -DBUILD_SHARED_LIBS=OFF
)
cmake_library() {
  local name=$1
  shift
  cmake -S "$work/$name" -B "$work/$name-build" "${cmake_options[@]}" "$@"
  cmake --build "$work/$name-build" --parallel "$jobs"
  cmake --install "$work/$name-build"
}
meson_library() {
  local name=$1
  shift
  meson setup "$work/$name-build" "$work/$name" --prefix="$prefix" --libdir=lib \
    --buildtype=release --default-library=static "$@"
  meson compile -C "$work/$name-build" -j "$jobs"
  meson install -C "$work/$name-build"
}

source_of x264
(cd "$work/x264" && ./configure --prefix="$prefix" --enable-static --enable-pic \
  --disable-cli --bit-depth=all && make -j "$jobs" && make install)

# The three x265 bit depths share one static archive, as in its upstream multilib recipe.
source_of x265
x265_options=("${cmake_options[@]}" -DENABLE_SHARED=OFF -DENABLE_CLI=OFF -DENABLE_LIBNUMA=OFF)
for depth in 12 10; do
  extra=()
  [ "$depth" != 12 ] || extra=(-DMAIN12=ON)
  cmake -S "$work/x265/source" -B "$work/x265-$depth" "${x265_options[@]}" \
    -DHIGH_BIT_DEPTH=ON -DEXPORT_C_API=OFF ${extra[@]+"${extra[@]}"}
  cmake --build "$work/x265-$depth" --parallel "$jobs"
done
mkdir "$work/x265-8"
ln -s "$work/x265-10/libx265.a" "$work/x265-8/libx265_main10.a"
ln -s "$work/x265-12/libx265.a" "$work/x265-8/libx265_main12.a"
cmake -S "$work/x265/source" -B "$work/x265-8" "${x265_options[@]}" \
  '-DEXTRA_LIB=x265_main10.a;x265_main12.a' "-DEXTRA_LINK_FLAGS=-L$work/x265-8" \
  -DLINKED_10BIT=ON -DLINKED_12BIT=ON
cmake --build "$work/x265-8" --parallel "$jobs"
mv "$work/x265-8/libx265.a" "$work/x265-8/libx265_main.a"
/usr/bin/libtool -static -o "$work/x265-8/libx265.a" \
  "$work/x265-8/libx265_main.a" "$work/x265-8/libx265_main10.a" "$work/x265-8/libx265_main12.a"
cmake --install "$work/x265-8"

source_of libvpx
(cd "$work/libvpx" && ./configure --prefix="$prefix" --target="$vpx_target" \
  --enable-static --disable-shared --enable-pic --enable-vp9-highbitdepth \
  --disable-examples --disable-tools --disable-docs --disable-unit-tests \
  --as=nasm && make -j "$jobs" && make install)
source_of opus
(cd "$work/opus" && ./configure --prefix="$prefix" --enable-static --disable-shared \
  --with-pic --disable-doc --disable-extra-programs && make -j "$jobs" && make install)
source_of aom
cmake_library aom -DENABLE_TESTS=OFF -DENABLE_DOCS=OFF -DENABLE_TOOLS=OFF -DENABLE_EXAMPLES=OFF
source_of svt-av1
cmake_library svt-av1 -DBUILD_APPS=OFF -DBUILD_TESTING=OFF -DNATIVE=OFF
source_of dav1d
meson_library dav1d -Denable_tools=false -Denable_examples=false -Denable_tests=false
source_of libsamplerate
cmake_library libsamplerate -DLIBSAMPLERATE_EXAMPLES=OFF -DBUILD_TESTING=OFF
source_of rubberband
# Preserve the previous libsamplerate backend without Homebrew's libsndfile/CLI chain.
meson_library rubberband -Dfft=vdsp -Dresampler=libsamplerate -Djni=disabled -Dladspa=disabled \
  -Dlv2=disabled -Dvamp=disabled -Dcmdline=disabled -Dtests=disabled

source_of ffmpeg
mkdir "$work/ffmpeg-build"
(cd "$work/ffmpeg-build" && "$work/ffmpeg/configure" \
  --prefix="$prefix" --cc="$CC" --cxx="$CXX" \
  --enable-shared --disable-static --enable-pic --disable-debug --disable-doc --disable-ffplay \
  --disable-autodetect --enable-gpl --enable-version3 --enable-pthreads \
  --enable-libx264 --enable-libx265 --enable-libvpx --enable-libopus \
  --enable-libaom --enable-libsvtav1 --enable-libdav1d --enable-librubberband \
  --enable-videotoolbox --enable-audiotoolbox --enable-avfoundation --enable-securetransport \
  --enable-zlib --enable-bzlib --enable-iconv \
  --disable-filter=transpose_vt \
  --pkg-config-flags=--static --extra-cflags="$CFLAGS -I$prefix/include" \
  --extra-cxxflags="$CXXFLAGS -I$prefix/include" --extra-ldflags="$LDFLAGS" --extra-libs=-lc++ \
  && make -j "$jobs" && make install)
# transpose_vt needs VTPixelRotationSessionCreate (macOS 13). Clovercut rotates
# with its own renderer; retain VideoToolbox decoding/encoding on Monterey.
nm -u "$prefix"/lib/libavfilter*.dylib > "$work/filter-imports.txt"
if grep -q VTPixelRotationSession "$work/filter-imports.txt"; then
  echo 'Monterey-incompatible rotation API remains in libavfilter' >&2
  exit 1
fi
awk '$1 ~ /^macos-source-/ { print }' "$root/scripts/ci/deps.lock" > "$prefix/share/media-sources.lock"
{
  printf 'target=%s\nminimum_macos=12.0\n' "$target"
  "$CC" --version
  "$prefix/bin/ffmpeg" -hide_banner -buildconf
} > "$prefix/share/media-build.txt" 2>&1
"$root/scripts/ci/check-min-os.sh" "$prefix" 12.0 --enforce
printf 'Built Monterey media runtime at %s\n' "$prefix"
