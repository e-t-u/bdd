CC ?= gcc
CFLAGS ?= -Wall -Wextra -O2 -I../include
LDFLAGS ?= -L../target/release -Wl,-rpath,'$$ORIGIN/../../target/release'
LDLIBS ?= -lbdd -lm

C_BINS = c/decode_media c/decode_ai_weights

.PHONY: all samples c-bins test clean clean-data

all: samples c-bins

samples:
	@echo "==> Generating sample binary files..."
	python3 python/generate_samples.py

c-bins: $(C_BINS)

c/decode_media: c/decode_media.c ../include/bdd.h
	$(CC) $(CFLAGS) $< $(LDFLAGS) $(LDLIBS) -o $@

c/decode_ai_weights: c/decode_ai_weights.c ../include/bdd.h
	$(CC) $(CFLAGS) $< $(LDFLAGS) $(LDLIBS) -o $@

test: all
	@echo "==> Testing Shell Scripts..."
	./shell/run_all_shell_examples.sh
	@echo "==> Testing Python Programs..."
	python3 python/decode_media.py
	python3 python/decode_ai_weights.py
	@echo "==> Testing C Programs..."
	./c/decode_media
	./c/decode_ai_weights
	@echo "==> All contrib demonstrations passed successfully!"

clean:
	rm -f $(C_BINS)

clean-data:
	rm -f data/*
