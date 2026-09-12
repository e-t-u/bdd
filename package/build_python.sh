#!/usr/bin/env bash
# build_python.sh: Build Python wheel and source distribution packages for bdd
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
DIST_DIR="${REPO_ROOT}/dist"

VERSION=$(grep -m1 '^version = ' "${REPO_ROOT}/Cargo.toml" | cut -d '"' -f 2)

echo "================================================================="
echo " Building Python Packages (Wheel & Sdist) for bdd v${VERSION}"
echo "================================================================="

mkdir -p "${DIST_DIR}"

# Ensure release shared library exists
if [ ! -f "${REPO_ROOT}/target/release/libbdd.so" ]; then
    echo "==> Building release shared library with cargo..."
    cargo build --release --manifest-path "${REPO_ROOT}/Cargo.toml"
fi

# Stage libbdd.so into python/ directory so it is bundled in the wheel
echo "==> Bundling libbdd.so into python package directory..."
cp -f "${REPO_ROOT}/target/release/libbdd.so" "${REPO_ROOT}/python/libbdd.so"
strip "${REPO_ROOT}/python/libbdd.so" 2>/dev/null || true

cleanup() {
    rm -f "${REPO_ROOT}/python/libbdd.so"
    rm -rf "${REPO_ROOT}/build" "${REPO_ROOT}/bdd.egg-info"
}
trap cleanup EXIT

# Build packages using uv or python3 -m build / pip wheel
if command -v uv >/dev/null 2>&1; then
    echo "==> Building wheel and sdist with uv..."
    uv build --out-dir "${DIST_DIR}"
elif python3 -m build --version >/dev/null 2>&1; then
    echo "==> Building wheel and sdist with python3 -m build..."
    python3 -m build --outdir "${DIST_DIR}" "${REPO_ROOT}"
else
    echo "==> Building wheel with pip..."
    python3 -m pip wheel --no-deps -w "${DIST_DIR}" "${REPO_ROOT}"
fi

# Locate the newly built wheel
WHEEL_FILE=$(find "${DIST_DIR}" -maxdepth 1 -type f -name "bdd-${VERSION}-*.whl" | head -n 1)

if [ -z "${WHEEL_FILE}" ]; then
    echo "Error: Wheel file was not generated." >&2
    exit 1
fi

echo "==> Verifying Wheel package contents:"
python3 -c "import zipfile, sys; z = zipfile.ZipFile(sys.argv[1]); [print('  ', f) for f in z.namelist()]" "${WHEEL_FILE}"

# Test installation in clean isolated virtualenv
echo "==> Verifying pip installation in isolated virtualenv..."
TEMP_VENV=$(mktemp -d -t bdd_test_venv_XXXXXX)
python3 -m venv "${TEMP_VENV}"
"${TEMP_VENV}/bin/pip" install --quiet "${WHEEL_FILE}"
"${TEMP_VENV}/bin/python" -c "
from bdd import Bdd
b = Bdd()
rev = b.reverse_bits(0b1101, 4)
assert rev == 0b1011, f'Expected 0b1011, got {bin(rev)}'
f16 = b.decode_f16(0x3c00)
assert f16 == 1.0, f'Expected 1.0, got {f16}'
print('    [OK] Python ctypes wrapper loaded libbdd.so and executed tests successfully!')
"
rm -rf "${TEMP_VENV}"

echo -e "\nPython packages created in dist/:"
find "${DIST_DIR}" -maxdepth 1 -type f \( -name "bdd-${VERSION}*.whl" -o -name "bdd-${VERSION}*.tar.gz" \) -exec ls -lh {} \;
