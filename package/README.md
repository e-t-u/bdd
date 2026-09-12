# bdd Packaging Scripts

Scripts to generate native distribution packages and SDK archives for `bdd`:
- **Debian / Ubuntu / Linux Mint**: `.deb` packages (installed via `dpkg -i` or `apt install`)
- **Fedora / RHEL / CentOS / AlmaLinux**: `.rpm` packages (installed via `dnf install` or `rpm -i`)
- **Cargo Crate (Rust)**: `.crate` package and git source dependency (`bdd = { git = "..." }`)
- **C Library SDK**: Standalone tarball (`bdd-c-*.tar.gz`) with headers, shared & static libraries, pkg-config, examples, and Makefile
- **Python Package (Pip)**: Wheel (`.whl`) and source distribution (`.tar.gz`) with bundled native library

---

## Contents Included in Packages

### Linux System Packages (.deb & .rpm)
Both `.deb` and `.rpm` packages install the complete `bdd` toolchain:
- **CLI Executable**: `/usr/bin/bdd`
- **Shared C Library**: `/usr/lib/libbdd.so` (or `/usr/lib64/libbdd.so` on 64-bit RPM systems)
- **Static C Library**: `/usr/lib/libbdd.a` (or `/usr/lib64/libbdd.a`)
- **pkg-config Configuration**: `/usr/lib/pkgconfig/bdd.pc` (or `/usr/lib64/pkgconfig/bdd.pc`)
- **C Header**: `/usr/include/bdd.h`
- **Manpage**: `/usr/share/man/man1/bdd.1.gz`
- **Documentation**: `/usr/share/doc/bdd/README.md` and `llms.txt`
- **License**: `/usr/share/doc/bdd/copyright` or `/usr/share/licenses/bdd/LICENSE`
- **ldconfig triggers**: Automatically configures the shared dynamic linker cache on install/removal.

### C Library SDK Archive (.tar.gz)
- `include/bdd.h`: Complete C header prototypes
- `lib/libbdd.so`: Dynamic shared library
- `lib/libbdd.a`: Static archive library
- `lib/pkgconfig/bdd.pc`: Pkg-config configuration
- `examples/`: Standalone C examples (`decode_media.c`, `decode_network.c`, `decode_ai_weights.c`) and `Makefile`
- `README.md`: Compilation and linking instructions

### Python Package (.whl & .tar.gz)
- `bdd`: Python module exposing `from bdd import Bdd`
- Bundled `libbdd.so` inside wheel for immediate out-of-the-box operation via ctypes
- Usable directly with `pip install bdd-0.5.0-py3-none-any.whl`

---

## Building Packages

### Via Makefile
```bash
# Build Debian package
make deb

# Build RPM package (for Fedora / DNF)
make rpm

# Build standalone C Library SDK archive
make c-lib

# Build Python Wheel and Sdist packages
make python

# Build all packages and generate SHA256 checksums
make packages
```

### Direct Script Execution
```bash
./package/build_deb.sh      # Debian .deb package
./package/build_rpm.sh      # Fedora/RHEL .rpm package
./package/build_c_lib.sh    # Standalone C Library SDK archive (.tar.gz)
./package/build_python.sh   # Python wheel and sdist (.whl, .tar.gz)
./package/build_all.sh      # Build entire suite and generate dist/SHA256SUMS
```

All generated packages and checksum files are written to `dist/`:
- `dist/bdd_<version>_<arch>.deb`
- `dist/bdd-<version>-1.<dist>.<arch>.rpm`
- `dist/bdd-c-<version>-<arch>.tar.gz`
- `dist/bdd-<version>-py3-none-any.whl`
- `dist/bdd-<version>.tar.gz`
- `dist/SHA256SUMS`

---

## Installation & Usage

### On Fedora / RHEL / CentOS / Rocky (DNF / RPM)
```bash
sudo dnf install ./dist/bdd-0.5.1-1.*.rpm
```

### On Debian / Ubuntu / Mint (APT / DPKG)
```bash
sudo apt install ./dist/bdd_0.5.1_*.deb
```

### Cargo Crate (Rust)
```bash
# Install binary CLI via Cargo from GitHub:
cargo install --git https://github.com/e-t-u/bdd.git --tag v0.5.1

# Or install from downloaded .crate release asset:
cargo install ./dist/bdd-0.5.1.crate
```
In your Rust project's `Cargo.toml`:
```toml
[dependencies]
bdd = { git = "https://github.com/e-t-u/bdd.git", tag = "v0.5.1" }
```

### Python (Pip)
```bash
pip install ./dist/bdd-0.5.1-py3-none-any.whl
```
```python
from bdd import Bdd
b = Bdd()
print(b.decode_f16(0x3c00))  # 1.0
```

### C Library SDK
```bash
tar -xzf dist/bdd-c-0.5.1-linux-x86_64.tar.gz
cd bdd-c-0.5.1-linux-x86_64/examples
make
./decode_network
```
