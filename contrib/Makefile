CC ?= gcc
CFLAGS ?= -Wall -Wextra -O2 -I../include
LDFLAGS ?= -L../target/release -Wl,-rpath,'$$ORIGIN/../../target/release'
LDLIBS ?= -lbdd -lm

C_BINS = c/decode_media c/decode_ai_weights c/decode_network

.PHONY: all samples c-bins rust-bins test pdf clean clean-data

all: samples c-bins rust-bins

rust-bins:
	cargo build --release -p bdd-netlink
	cp -f ../target/release/bdd-netlink ./bdd-netlink

samples:
	@echo "==> Generating sample binary files..."
	python3 python/generate_samples.py

c-bins: $(C_BINS)

c/decode_media: c/decode_media.c ../include/bdd.h
	$(CC) $(CFLAGS) $< $(LDFLAGS) $(LDLIBS) -o $@

c/decode_ai_weights: c/decode_ai_weights.c ../include/bdd.h
	$(CC) $(CFLAGS) $< $(LDFLAGS) $(LDLIBS) -o $@

c/decode_network: c/decode_network.c ../include/bdd.h
	$(CC) $(CFLAGS) $< $(LDFLAGS) $(LDLIBS) -o $@

pdf:
	node ../scripts/render_pdf.js README.md README.pdf

test: all
	@echo "==> Testing Shell Scripts..."
	./shell/run_all_shell_examples.sh
	@echo "==> Testing Python Programs..."
	python3 python/decode_media.py
	python3 python/decode_ai_weights.py
	python3 python/decode_network.py
	python3 python/stream_memory_keys.py --source demo --units 128
	@echo "==> Testing JSON Translators (C and Rust)..."
	../target/release/bdd --explain-pattern "sync:8u,pid:13u" --output-json | python3 python/json_to_c.py --name TestPacket | grep -q "MpegTsHeader\|TestPacket"
	../target/release/bdd --explain-pattern "sync:8u,pid:13u" --output-json | python3 python/json_to_rust.py --name TestPacket | grep -q "pub struct TestPacket"
	@echo "==> Testing C Programs..."
	./c/decode_media
	./c/decode_ai_weights
	./c/decode_network
	@echo "==> All contrib demonstrations passed successfully!"

clean:
	rm -f $(C_BINS) bdd-netlink README.pdf

clean-data:
	rm -f data/*
