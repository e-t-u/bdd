#!/usr/bin/env bash
# build_c_lib.sh: Build standalone C library SDK archive (.tar.gz) for bdd
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
DIST_DIR="${REPO_ROOT}/dist"

VERSION=$(grep -m1 '^version = ' "${REPO_ROOT}/Cargo.toml" | cut -d '"' -f 2)
ARCH_RAW=$(uname -m)
case "${ARCH_RAW}" in
    x86_64)  ARCH="linux-x86_64" ;;
    aarch64) ARCH="linux-aarch64" ;;
    armv7l)  ARCH="linux-armhf" ;;
    *)       ARCH="linux-${ARCH_RAW}" ;;
esac

PKG_NAME="bdd-c-${VERSION}-${ARCH}"
STAGING_DIR="${DIST_DIR}/${PKG_NAME}"
OUTPUT_TAR="${DIST_DIR}/${PKG_NAME}.tar.gz"

echo "================================================================="
echo " Building C Library SDK Archive: ${PKG_NAME}.tar.gz"
echo "================================================================="

# Ensure release binaries and libraries exist
if [ ! -f "${REPO_ROOT}/target/release/libbdd.so" ] || [ ! -f "${REPO_ROOT}/target/release/libbdd.a" ]; then
    echo "==> Building release libraries with cargo..."
    cargo build --release --manifest-path "${REPO_ROOT}/Cargo.toml"
fi

rm -rf "${STAGING_DIR}"
mkdir -p "${STAGING_DIR}/include"
mkdir -p "${STAGING_DIR}/lib/pkgconfig"
mkdir -p "${STAGING_DIR}/examples"

# Install C header
install -m 0644 "${REPO_ROOT}/include/bdd.h" "${STAGING_DIR}/include/bdd.h"

# Install shared and static libraries
install -m 0755 "${REPO_ROOT}/target/release/libbdd.so" "${STAGING_DIR}/lib/libbdd.so"
strip "${STAGING_DIR}/lib/libbdd.so" 2>/dev/null || true

if [ -f "${REPO_ROOT}/target/release/libbdd.a" ]; then
    install -m 0644 "${REPO_ROOT}/target/release/libbdd.a" "${STAGING_DIR}/lib/libbdd.a"
fi

# Generate pkg-config template
cat << _EOF_PC_ > "${STAGING_DIR}/lib/pkgconfig/bdd.pc"
prefix=\${pcfiledir}/../..
exec_prefix=\${prefix}
libdir=\${prefix}/lib
includedir=\${prefix}/include

Name: bdd
Description: High-performance bitstream slicing, transcoding, and inspection library
Version: ${VERSION}
Libs: -L\${libdir} -lbdd
Cflags: -I\${includedir}
_EOF_PC_
chmod 0644 "${STAGING_DIR}/lib/pkgconfig/bdd.pc"

# Copy example C programs
if [ -d "${REPO_ROOT}/contrib/c" ]; then
    for f in "${REPO_ROOT}/contrib/c"/*.c; do
        if [ -f "$f" ]; then
            install -m 0644 "$f" "${STAGING_DIR}/examples/"
        fi
    done
fi

# Example Makefile
cat << 'EOF_EX_MK' > "${STAGING_DIR}/examples/Makefile"
CC ?= gcc
CFLAGS ?= -Wall -Wextra -O2 -I../include
LDFLAGS ?= -L../lib -lbdd -Wl,-rpath,'$$ORIGIN/../lib'

SRCS := $(wildcard *.c)
PROGS := $(SRCS:.c=)

all: $(PROGS)

%: %.c
	$(CC) $(CFLAGS) $< -o $@ $(LDFLAGS)

clean:
	rm -f $(PROGS)

.PHONY: all clean
EOF_EX_MK
chmod 0644 "${STAGING_DIR}/examples/Makefile"

# SDK README
cat << EOF_README > "${STAGING_DIR}/README.md"
# bdd C Library SDK (v${VERSION})

High-performance C ABI library for sub-byte bitstream slicing, arbitrary bit-width packing/unpacking, and AI floating point codecs (NVFP4, FP6, FP8, BF16, FP16).

## Directory Structure
- \`include/bdd.h\`: C function prototypes and definitions.
- \`lib/libbdd.so\`: Dynamic shared library.
- \`lib/libbdd.a\`: Static archive library.
- \`lib/pkgconfig/bdd.pc\`: Pkg-config configuration.
- \`examples/\`: Standalone C code examples demonstrating usage.

## Compilation & Linking

### Dynamic Linking
\`\`\`bash
gcc -Iinclude main.c -Llib -lbdd -Wl,-rpath,\$ORIGIN/lib -o main
\`\`\`

### Static Linking
\`\`\`bash
gcc -Iinclude main.c lib/libbdd.a -lpthread -ldl -lm -o main
\`\`\`

### Using pkg-config
\`\`\`bash
export PKG_CONFIG_PATH=\$(pwd)/lib/pkgconfig:\$PKG_CONFIG_PATH
gcc \$(pkg-config --cflags bdd) main.c \$(pkg-config --libs bdd) -o main
\`\`\`
EOF_README
chmod 0644 "${STAGING_DIR}/README.md"

if [ -f "${REPO_ROOT}/LICENSE" ]; then
    install -m 0644 "${REPO_ROOT}/LICENSE" "${STAGING_DIR}/LICENSE"
fi

# Create tarball
tar -czf "${OUTPUT_TAR}" -C "${DIST_DIR}" "${PKG_NAME}"
rm -rf "${STAGING_DIR}"

echo "==> Verifying C Library archive contents:"
tar -ztvf "${OUTPUT_TAR}"

echo -e "\nC Library SDK archive created: ${OUTPUT_TAR}"
