#!/usr/bin/env bash
# build_deb.sh: Build Debian/Ubuntu (.deb) package for bdd
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
DIST_DIR="${REPO_ROOT}/dist"

VERSION=$(grep -m1 '^version = ' "${REPO_ROOT}/Cargo.toml" | cut -d '"' -f 2)
ARCH_RAW=$(uname -m)
case "${ARCH_RAW}" in
    x86_64)  DEB_ARCH="amd64" ;;
    aarch64) DEB_ARCH="arm64" ;;
    armv7l)  DEB_ARCH="armhf" ;;
    *)       DEB_ARCH="${ARCH_RAW}" ;;
esac

PKG_NAME="bdd_${VERSION}_${DEB_ARCH}"
STAGING_DIR="${DIST_DIR}/${PKG_NAME}"
OUTPUT_DEB="${DIST_DIR}/${PKG_NAME}.deb"

echo "================================================================="
echo " Building Debian Package: ${PKG_NAME}.deb"
echo "================================================================="

if ! command -v dpkg-deb >/dev/null 2>&1; then
    echo "Error: 'dpkg-deb' is required but not installed." >&2
    exit 1
fi

echo "==> Building release binaries with cargo..."
cargo build --release --workspace --manifest-path "${REPO_ROOT}/Cargo.toml"

# Clean and prepare directory tree
rm -rf "${STAGING_DIR}"
mkdir -p "${STAGING_DIR}/DEBIAN"
mkdir -p "${STAGING_DIR}/usr/bin"
mkdir -p "${STAGING_DIR}/usr/lib"
mkdir -p "${STAGING_DIR}/usr/lib/pkgconfig"
mkdir -p "${STAGING_DIR}/usr/include"
mkdir -p "${STAGING_DIR}/usr/share/man/man1"
mkdir -p "${STAGING_DIR}/usr/share/doc/bdd"
mkdir -p "${STAGING_DIR}/usr/share/bdd"

install -m 0755 "${REPO_ROOT}/target/release/bdd" "${STAGING_DIR}/usr/bin/bdd"
strip "${STAGING_DIR}/usr/bin/bdd" 2>/dev/null || true

if [ -f "${REPO_ROOT}/target/release/bdd-mcp" ]; then
    install -m 0755 "${REPO_ROOT}/target/release/bdd-mcp" "${STAGING_DIR}/usr/bin/bdd-mcp"
    strip "${STAGING_DIR}/usr/bin/bdd-mcp" 2>/dev/null || true
fi

install -m 0755 "${REPO_ROOT}/target/release/libbdd.so" "${STAGING_DIR}/usr/lib/libbdd.so"
strip "${STAGING_DIR}/usr/lib/libbdd.so" 2>/dev/null || true

if [ -f "${REPO_ROOT}/target/release/libbdd.a" ]; then
    install -m 0644 "${REPO_ROOT}/target/release/libbdd.a" "${STAGING_DIR}/usr/lib/libbdd.a"
fi

# Install C header
install -m 0644 "${REPO_ROOT}/include/bdd.h" "${STAGING_DIR}/usr/include/bdd.h"

# Install pkg-config file
cat << _EOF_PC_ > "${STAGING_DIR}/usr/lib/pkgconfig/bdd.pc"
prefix=/usr
exec_prefix=\${prefix}
libdir=\${prefix}/lib
includedir=\${prefix}/include

Name: bdd
Description: High-performance bitstream slicing, transcoding, and inspection library
Version: ${VERSION}
Libs: -L\${libdir} -lbdd
Cflags: -I\${includedir}
_EOF_PC_
chmod 0644 "${STAGING_DIR}/usr/lib/pkgconfig/bdd.pc"

# Install compressed man page
if [ -f "${REPO_ROOT}/bdd.1" ]; then
    gzip -9cn "${REPO_ROOT}/bdd.1" > "${STAGING_DIR}/usr/share/man/man1/bdd.1.gz"
    chmod 0644 "${STAGING_DIR}/usr/share/man/man1/bdd.1.gz"
fi

# Install documentation & license
install -m 0644 "${REPO_ROOT}/README.md" "${STAGING_DIR}/usr/share/doc/bdd/README.md"
if [ -f "${REPO_ROOT}/llms.txt" ]; then
    install -m 0644 "${REPO_ROOT}/llms.txt" "${STAGING_DIR}/usr/share/doc/bdd/llms.txt"
fi
if [ -f "${REPO_ROOT}/LICENSE" ]; then
    install -m 0644 "${REPO_ROOT}/LICENSE" "${STAGING_DIR}/usr/share/doc/bdd/copyright"
fi

# Install format presets
if [ -f "${REPO_ROOT}/presets.json" ]; then
    install -m 0644 "${REPO_ROOT}/presets.json" "${STAGING_DIR}/usr/share/bdd/presets.json"
fi

# Calculate installed size in KB
INSTALLED_SIZE=$(du -sk "${STAGING_DIR}" | cut -f1)

# Generate DEBIAN/control
cat << _EOF_CTRL_ > "${STAGING_DIR}/DEBIAN/control"
Package: bdd
Version: ${VERSION}
Section: utils
Priority: optional
Architecture: ${DEB_ARCH}
Installed-Size: ${INSTALLED_SIZE}
Maintainer: Esa Turtiainen <eturtiainen@gmail.com>
Homepage: https://github.com/e-t-u/bdd
Description: Sub-byte bitstream slicer, binary structure prober, and transcoder
 bdd is a high-performance command-line tool and C-ABI shared library to
 interpret, manipulate, merge, transcode, and create arbitrary bit streams,
 sub-byte AI model weights (NVFP4, FP6, FP8, BF16), and network/multimedia protocols.
_EOF_CTRL_
chmod 0644 "${STAGING_DIR}/DEBIAN/control"

# Maintainer post-install / post-remove scripts
cat << '_EOF_POST_' > "${STAGING_DIR}/DEBIAN/postinst"
#!/bin/sh
set -e
if [ "$1" = "configure" ]; then
    if command -v ldconfig >/dev/null 2>&1; then
        ldconfig
    fi
fi
exit 0
_EOF_POST_
chmod 0755 "${STAGING_DIR}/DEBIAN/postinst"

cat << '_EOF_RM_' > "${STAGING_DIR}/DEBIAN/postrm"
#!/bin/sh
set -e
if [ "$1" = "remove" ]; then
    if command -v ldconfig >/dev/null 2>&1; then
        ldconfig
    fi
fi
exit 0
_EOF_RM_
chmod 0755 "${STAGING_DIR}/DEBIAN/postrm"

# Build package
dpkg-deb --build --root-owner-group "${STAGING_DIR}" "${OUTPUT_DEB}"
rm -rf "${STAGING_DIR}"

echo "==> Verifying package contents:"
dpkg-deb --contents "${OUTPUT_DEB}"

echo -e "\nDebian package created: ${OUTPUT_DEB}"
