.PHONY: qefro install install-dev uninstall help

# cargo install puts the `qefro` binary in $(CARGO_HOME)/bin, usually ~/.cargo/bin
CARGO_BIN ?= $(HOME)/.cargo/bin

help:
	@echo "make install      install release qefro to $(CARGO_BIN)"
	@echo "make install-dev  install debug qefro (faster)"
	@echo "make uninstall    remove the installed qefro binary"
	@echo "make qefro        build target/debug/qefro without installing"

qefro:
	cargo build -p qefro-cli

install:
	cargo install --path crates/qefro-cli --locked --force
	@echo "Installed $(CARGO_BIN)/qefro"
	@echo "Ensure $(CARGO_BIN) is on PATH, then run: qefro --help"

install-dev:
	cargo install --path crates/qefro-cli --locked --force --debug
	@echo "Installed debug $(CARGO_BIN)/qefro"
	@echo "Ensure $(CARGO_BIN) is on PATH, then run: qefro --help"

uninstall:
	cargo uninstall qefro-cli
