#!/usr/bin/env bash
# build_all.sh: Build all Linux distribution packages (DEB for Debian/Ubuntu, RPM for Fedora/DNF)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
DIST_DIR="${REPO_ROOT}/dist"

mkdir -p "${DIST_DIR}"
rm -rf "${DIST_DIR:?}"/*

echo "#################################################################"
echo "# Building All Distribution Packages for bdd"
echo "#################################################################"

echo "==> Step 1: Building optimized release binaries..."
cargo build --release --manifest-path "${REPO_ROOT}/Cargo.toml"

echo -e "\n==> Step 2: Building Debian (.deb) package..."
"${SCRIPT_DIR}/build_deb.sh"

echo -e "\n==> Step 3: Building RPM (.rpm) package for DNF..."
"${SCRIPT_DIR}/build_rpm.sh"

echo -e "\n==> Step 4: Building C Library SDK (.tar.gz)..."
"${SCRIPT_DIR}/build_c_lib.sh"

echo -e "\n==> Step 5: Building Python packages (Wheel & Sdist)..."
"${SCRIPT_DIR}/build_python.sh"

echo -e "\n==> Step 6: Generating SHA256 Checksums..."
cd "${DIST_DIR}"
sha256sum *.deb *.rpm *.tar.gz *.whl > SHA256SUMS

echo -e "\n================================================================="
echo " Packaging Completed Successfully!"
echo " Output directory: ${DIST_DIR}"
echo "================================================================="
ls -lh *.deb *.rpm *.tar.gz *.whl SHA256SUMS

echo -e "\nSHA256 Checksums:"
cat SHA256SUMS
