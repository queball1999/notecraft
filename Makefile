# Shortcuts for common tasks. `make ci` runs the same gates as CI (`cargo xtask ci`); run it
# before you push.
#
# cargo is found in rustup's default folder ($CARGO_HOME/bin, or .cargo/bin in your home
# folder) even when that folder is not on PATH; otherwise `cargo` from PATH is used.
# Override with `make CARGO=/path/to/cargo <target>`.

CARGO_BIN := $(subst \,/,$(or $(CARGO_HOME),$(USERPROFILE),$(HOME)))
ifeq ($(CARGO_HOME),)
CARGO_BIN := $(CARGO_BIN)/.cargo
endif
CARGO_BIN := $(CARGO_BIN)/bin
CARGO ?= $(firstword $(wildcard $(CARGO_BIN)/cargo.exe $(CARGO_BIN)/cargo) cargo)

TRUNK ?= $(firstword $(wildcard $(CARGO_BIN)/trunk.exe $(CARGO_BIN)/trunk) trunk)
WEB_APP := apps/notecraft-web

.PHONY: help run run-release server build release test ci fmt fmt-check clippy check wasm parity text clean

help:
	$(info make run          run the desktop app (debug build))
	$(info make run-release  run the desktop app (release build))
	$(info make server       serve the browser build on localhost (placeholder until Phase 4))
	$(info make build        build the whole workspace (debug))
	$(info make release      build the whole workspace (release))
	$(info make test         run every test)
	$(info make ci           run every CI gate (cargo xtask ci))
	$(info make fmt          format the code)
	$(info make fmt-check    check formatting without changing files)
	$(info make clippy       lint with clippy, warnings as errors)
	$(info make check        type-check the workspace)
	$(info make wasm         check that L0 to L5 build for wasm32)
	$(info make parity       regenerate the parity checklist summary)
	$(info make text         check that every file is plain ASCII)
	$(info make clean        remove build output)

run:
	$(CARGO) run -p notecraft

run-release:
	$(CARGO) run -p notecraft --release

# Placeholder: the browser build ($(WEB_APP)) is created in Phase 4, PR 4.5. That PR finishes
# this target (see build_plan/06-phase-4-advanced.md). Until then it stops with a message.
server:
ifeq ($(wildcard $(WEB_APP)/Cargo.toml),)
	$(error browser build not created yet ($(WEB_APP) arrives in Phase 4, PR 4.5); use `make run` for the desktop app)
else
	$(TRUNK) serve $(WEB_APP)/index.html --open
endif

build:
	$(CARGO) build --workspace

release:
	$(CARGO) build --workspace --release

test:
	$(CARGO) test --workspace

ci:
	$(CARGO) xtask ci

fmt:
	$(CARGO) fmt --all

fmt-check:
	$(CARGO) fmt --all -- --check

clippy:
	$(CARGO) clippy --workspace --all-targets -- -D warnings

check:
	$(CARGO) check --workspace --all-targets

wasm:
	$(CARGO) xtask wasm

parity:
	$(CARGO) xtask parity

text:
	$(CARGO) xtask text

clean:
	$(CARGO) clean
