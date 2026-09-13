.PHONY: all build release test lint fmt install clean bench docs pdf contrib-pdf man-pdf pres-pdf contrib web web-py deb rpm packages pkgs

all: release

build:
	cargo build

release:
	cargo build --release
	ln -sf target/release/bdd ./bdd

test: release
	cargo test --all-targets --all-features

contrib: release
	$(MAKE) -C contrib test

bench:
	cargo bench

lint:
	cargo clippy --all-targets --all-features -- -D warnings

fmt:
	cargo fmt

check-fmt:
	cargo fmt --check

install:
	cargo install --path .
	ln -sf ~/.cargo/bin/bdd ~/.local/bin/bdd
	mkdir -p ~/.config/bdd
	cp -f presets.json ~/.config/bdd/presets.json

web: release
	cargo run --manifest-path web/Cargo.toml

web-py: release
	python3 web/server.py

pdf:
	node scripts/render_pdf.js README.md README.pdf
	node scripts/render_pdf.js contrib/README.md contrib/README.pdf

contrib-pdf:
	node scripts/render_pdf.js contrib/README.md contrib/README.pdf

man-pdf:
	mkdir -p docs
	groff -man -T ps bdd.1 | ps2pdf - docs/bdd.1.pdf
	groff -man -T html bdd.1 > docs/bdd.1.html

pres-pdf:
	mkdir -p docs
	libreoffice --headless --convert-to pdf docs/Presentation.odp --outdir docs/

docs: pdf man-pdf pres-pdf

deb: release
	./package/build_deb.sh

rpm: release
	./package/build_rpm.sh

c-lib: release
	./package/build_c_lib.sh

python: release
	./package/build_python.sh

packages: release
	./package/build_all.sh

pkgs: packages

clean:
	cargo clean
	rm -f bdd
	rm -rf dist
	$(MAKE) -C contrib clean
