# bdd Packaging Scripts

Scripts to generate native distribution packages for Linux distributions:
- **Debian / Ubuntu / Linux Mint**: `.deb` packages (installed via `dpkg -i` or `apt install`)
- **Fedora / RHEL / CentOS / AlmaLinux**: `.rpm` packages (installed via `dnf install` or `rpm -i`)

---

## Contents Included in Packages

Both `.deb` and `.rpm` packages install the complete bdd toolchain:
- **CLI Executable**: `/usr/bin/bdd`
- **Shared C Library**: `/usr/lib/libbdd.so` (or `/usr/lib64/libbdd.so` on 64-bit RPM systems)
- **C Header**: `/usr/include/bdd.h`
- **Manpage**: `/usr/share/man/man1/bdd.1.gz`
- **Documentation**: `/usr/share/doc/bdd/README.md` and `llms.txt`
- **License**: `/usr/share/doc/bdd/copyright` or `/usr/share/licenses/bdd/LICENSE`
- **ldconfig triggers**: Automatically configures the shared dynamic linker cache on install/removal.

---

## Prerequisites

- **Debian package**: Requires `dpkg-deb`
- **RPM package**: Requires `rpmbuild`

---

## Building Packages

### Via Makefile
```bash
# Build Debian package
make deb

# Build RPM package (for Fedora / DNF)
make rpm

# Build both packages and generate SHA256 checksums
make packages
```

### Direct Script Execution
```bash
# Build .deb package:
./package/build_deb.sh

# Build .rpm package:
./package/build_rpm.sh

# Build both and generate dist/SHA256SUMS:
./package/build_all.sh
```

All generated packages and checksum files are written to `dist/`:
- `dist/bdd_<version>_<arch>.deb`
- `dist/bdd-<version>-1.<dist>.<arch>.rpm`
- `dist/SHA256SUMS`

---

## Installation

### On Fedora / RHEL / CentOS / Rocky (DNF / RPM)
```bash
sudo dnf install ./dist/bdd-0.4.0-1.*.rpm
```

### On Debian / Ubuntu / Mint (APT / DPKG)
```bash
sudo apt install ./dist/bdd_0.4.0_*.deb
```
