#!/usr/bin/env bash
# build_rpm.sh: Build RPM package (.rpm) for bdd (Fedora, RHEL, CentOS, AlmaLinux)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
DIST_DIR="${REPO_ROOT}/dist"
RPM_TOPDIR="${DIST_DIR}/rpmbuild"

VERSION=$(grep -m1 '^version = ' "${REPO_ROOT}/Cargo.toml" | cut -d '"' -f 2)
ARCH_RAW=$(uname -m)

echo "================================================================="
echo " Building RPM Package for DNF: bdd-${VERSION}-1.${ARCH_RAW}.rpm"
echo "================================================================="

if ! command -v rpmbuild >/dev/null 2>&1; then
    echo "Error: 'rpmbuild' is required but not installed." >&2
    exit 1
fi

# Ensure release binaries exist
if [ ! -f "${REPO_ROOT}/target/release/bdd" ] || [ ! -f "${REPO_ROOT}/target/release/libbdd.so" ]; then
    echo "==> Building release binaries with cargo..."
    cargo build --release --manifest-path "${REPO_ROOT}/Cargo.toml"
fi

# Setup rpmbuild working directories
rm -rf "${RPM_TOPDIR}"
mkdir -p "${RPM_TOPDIR}"/{BUILD,RPMS,SOURCES,SPECS,SRPMS}

# Copy payload into SOURCES
install -m 0755 "${REPO_ROOT}/target/release/bdd" "${RPM_TOPDIR}/SOURCES/bdd"
strip "${RPM_TOPDIR}/SOURCES/bdd" 2>/dev/null || true

install -m 0755 "${REPO_ROOT}/target/release/libbdd.so" "${RPM_TOPDIR}/SOURCES/libbdd.so"
strip "${RPM_TOPDIR}/SOURCES/libbdd.so" 2>/dev/null || true

install -m 0644 "${REPO_ROOT}/include/bdd.h" "${RPM_TOPDIR}/SOURCES/bdd.h"

if [ -f "${REPO_ROOT}/bdd.1" ]; then
    gzip -9cn "${REPO_ROOT}/bdd.1" > "${RPM_TOPDIR}/SOURCES/bdd.1.gz"
fi

install -m 0644 "${REPO_ROOT}/README.md" "${RPM_TOPDIR}/SOURCES/README.md"
if [ -f "${REPO_ROOT}/llms.txt" ]; then
    install -m 0644 "${REPO_ROOT}/llms.txt" "${RPM_TOPDIR}/SOURCES/llms.txt"
fi
if [ -f "${REPO_ROOT}/LICENSE" ]; then
    install -m 0644 "${REPO_ROOT}/LICENSE" "${RPM_TOPDIR}/SOURCES/LICENSE"
fi

# Generate RPM spec file
SPEC_FILE="${RPM_TOPDIR}/SPECS/bdd.spec"
cat << _EOF_SPEC_ > "${SPEC_FILE}"
Name:           bdd
Version:        ${VERSION}
Release:        1%{?dist}
Summary:        Sub-byte bitstream slicer, binary structure prober, and transcoder
License:        GPL-3.0-only
URL:            https://github.com/e-t-u/bdd

%define _build_id_links none
%undefine _missing_build_ids_terminate_build

%description
bdd is a high-performance command-line tool and C-ABI shared library to
interpret, manipulate, merge, transcode, and create arbitrary bit streams,
sub-byte AI model weights (NVFP4, FP6, FP8, BF16), and network/multimedia protocols.

%install
rm -rf %{buildroot}
mkdir -p %{buildroot}%{_bindir}
mkdir -p %{buildroot}%{_libdir}
mkdir -p %{buildroot}%{_includedir}
mkdir -p %{buildroot}%{_mandir}/man1
mkdir -p %{buildroot}%{_docdir}/%{name}
mkdir -p %{buildroot}%{_licensedir}/%{name}

install -m 0755 %{_sourcedir}/bdd %{buildroot}%{_bindir}/bdd
install -m 0755 %{_sourcedir}/libbdd.so %{buildroot}%{_libdir}/libbdd.so
install -m 0644 %{_sourcedir}/bdd.h %{buildroot}%{_includedir}/bdd.h
if [ -f %{_sourcedir}/bdd.1.gz ]; then
    install -m 0644 %{_sourcedir}/bdd.1.gz %{buildroot}%{_mandir}/man1/bdd.1.gz
fi
install -m 0644 %{_sourcedir}/README.md %{buildroot}%{_docdir}/%{name}/README.md
if [ -f %{_sourcedir}/llms.txt ]; then
    install -m 0644 %{_sourcedir}/llms.txt %{buildroot}%{_docdir}/%{name}/llms.txt
fi
if [ -f %{_sourcedir}/LICENSE ]; then
    install -m 0644 %{_sourcedir}/LICENSE %{buildroot}%{_licensedir}/%{name}/LICENSE
fi

%post -p /sbin/ldconfig
%postun -p /sbin/ldconfig

%files
%license %{_licensedir}/%{name}/LICENSE
%{_bindir}/bdd
%{_libdir}/libbdd.so
%{_includedir}/bdd.h
%{_mandir}/man1/bdd.1*
%doc %{_docdir}/%{name}/README.md
%doc %{_docdir}/%{name}/llms.txt

%changelog
* Sat Sep 12 2026 Esa Turtiainen <eturtiainen@gmail.com> - ${VERSION}-1
- Release version ${VERSION} with network presets, sub-byte slicing, AI float conversions, and C-ABI library.
_EOF_SPEC_

# Execute rpmbuild
rpmbuild --define "_topdir ${RPM_TOPDIR}" -bb "${SPEC_FILE}"

# Move generated RPMs directly to dist/
find "${RPM_TOPDIR}/RPMS" -type f -name "*.rpm" -exec cp {} "${DIST_DIR}/" \;
rm -rf "${RPM_TOPDIR}"

BUILT_RPM=$(find "${DIST_DIR}" -maxdepth 1 -type f -name "bdd-${VERSION}-*.rpm" | head -n 1)

echo "==> Verifying RPM package information:"
rpm -qpi "${BUILT_RPM}"

echo "==> Verifying RPM package contents:"
rpm -qpl "${BUILT_RPM}"

echo -e "\nRPM package created: ${BUILT_RPM}"
