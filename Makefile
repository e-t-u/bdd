.PHONY: all build release test lint fmt install clean bench docs pdf contrib-pdf man-pdf pres-pdf contrib web web-py

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

web: release
	./bdd --serve

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

clean:
	cargo clean
	rm -f bdd
	$(MAKE) -C contrib clean
