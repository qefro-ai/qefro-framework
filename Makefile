.PHONY: qefro install install-dev uninstall help check postgres docs docs-serve

# cargo install puts the `qefro` binary in $(CARGO_HOME)/bin, usually ~/.cargo/bin
CARGO_BIN ?= $(HOME)/.cargo/bin
DATABASE_URL ?= postgres://qefro:qefro@127.0.0.1:5432/qefro

help:
	@echo "make install      install release qefro to $(CARGO_BIN)"
	@echo "make install-dev  install debug qefro (faster)"
	@echo "make uninstall    remove the installed qefro binary"
	@echo "make qefro        build target/debug/qefro without installing"
	@echo "make postgres     create/verify the qefro role and database"
	@echo "make check        workspace tests + frontend tests (requires Postgres)"
	@echo "make docs         build the MkDocs site into ./site"
	@echo "make docs-serve   serve the docs locally on http://127.0.0.1:8000"

qefro:
	cargo build -p qefro-cli

postgres:
	./scripts/setup-postgres.sh

check:
	./scripts/setup-postgres.sh --check
	DATABASE_URL=$(DATABASE_URL) cargo test --workspace -- --test-threads=1
	cd frontend && npm test

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

docs:
	python3 -m venv .venv-docs
	.venv-docs/bin/pip install -q -r requirements-docs.txt
	.venv-docs/bin/mkdocs build --strict

docs-serve:
	python3 -m venv .venv-docs
	.venv-docs/bin/pip install -q -r requirements-docs.txt
	.venv-docs/bin/mkdocs serve
