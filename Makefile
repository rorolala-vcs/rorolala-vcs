# Rorolala — build and hand-off.
#
# `make` on its own runs `all`; `make export` is the target that produces the files
# meant to leave the repository. Every artifact is built for release — there is no
# way to hand out a debug binary.

### Toolchain and layout

# Cargo command line. Override it to pass extra flags, e.g.
# `make export CARGO="cargo --offline"`.
CARGO ?= cargo

# Cargo's target directory, matching `.cargo/config.toml`.
TARGET_DIR ?= .cache/rs-target

# .NET's output directory, matching `Directory.Build.props`. It is under the same `.cache` as
# cargo's, so the two toolchains' output sits in one place.
CS_TARGET_DIR ?= .cache/cs-target

# Directory `export` lays the hand-off artifacts out in.
BUILD_DIR ?= build

# The only profile this Makefile builds. Everything it produces is release, so
# nothing that ships can carry debug assertions or debug info.
RELEASE_FLAG := --release

# Where cargo writes the release artifacts.
RELEASE_DIR := $(TARGET_DIR)/release

# The generated C header, written by the root build script while `lib` builds.
HEADER := $(RELEASE_DIR)/ffi_bindings/rorolala_ffi.h

# Completion scripts. While the programs compile, mingling's `gen_program!()` writes
# one per supported shell into this directory — deliberately not profile-specific,
# so the same files serve every build.
COMPLETION_DIR ?= $(TARGET_DIR)/mingling

# Shells mingling generates a completion script for.
COMPLETION_SHELLS := sh zsh fish ps1

# The command line programs. Each is a workspace member whose package name is also
# its binary name, so one list drives the build, the export, and the completion
# scripts — mingling names those `<program>_comp.<shell>` and they are exported as
# `<program>-completion.<shell>`. The scripts call their command by name and never
# refer to their own file name, so the exported name is free.
PROGRAMS := rola rola-daemon

# `export` and `clean` delete their destination with `rm -rf`, so refuse a path that
# would take something else with it.
ifneq ($(filter /%,$(BUILD_DIR)),)
  $(error BUILD_DIR must be a relative path, got `$(BUILD_DIR)')
endif
ifeq ($(strip $(BUILD_DIR)),)
  $(error BUILD_DIR must not be empty)
endif
ifneq ($(filter /%,$(TARGET_DIR)),)
  $(error TARGET_DIR must be a relative path, got `$(TARGET_DIR)')
endif
ifeq ($(strip $(TARGET_DIR)),)
  $(error TARGET_DIR must not be empty)
endif
ifneq ($(filter /%,$(CS_TARGET_DIR)),)
  $(error CS_TARGET_DIR must be a relative path, got `$(CS_TARGET_DIR)')
endif
ifeq ($(strip $(CS_TARGET_DIR)),)
  $(error CS_TARGET_DIR must not be empty)
endif

# Platform spellings. Cargo names a Unix shared library with a `lib` prefix and a
# Windows one with a `.dll` suffix, so the exported names are normalised to one
# shape per platform:
#
#   | cargo artifact               | exported as                              |
#   | ---------------------------- | ---------------------------------------- |
#   | librorolala.so               | $(BUILD_DIR)/lib/rorolala.so             |
#   | librorolala.dylib            | $(BUILD_DIR)/lib/rorolala.dylib          |
#   | rorolala.dll                 | $(BUILD_DIR)/lib/rorolala.dll            |
#   | rorolala.dll.lib             | $(BUILD_DIR)/lib/rorolala.dll.lib        |
#   | librorolala.a                | $(BUILD_DIR)/lib/rorolala.a              |
#   | rorolala.lib                 | $(BUILD_DIR)/lib/rorolala.lib            |
#   | ffi_bindings/rorolala_ffi.h  | $(BUILD_DIR)/lib/rorolala.h              |
#   |                              | $(BUILD_DIR)/lib/rorolala.hpp            |
#   | rola[.exe]                   | $(BUILD_DIR)/bin/rola[.exe]              |
#   | mingling/<program>_comp.*    | $(BUILD_DIR)/bin/<program>-completion.*  |
#
# The header is C and C++ at once — its declarations sit in an `extern "C"` block —
# so `.hpp` is the same file under the name a C++ project includes. The completion
# scripts are the four shells mingling generates: sh, zsh, fish and ps1.
#
# The import library is the one artifact only Windows has: a DLL is linked through it,
# and the static library of the same crate needs nothing beside it, which is why the
# two `.lib` files are both handed over and named apart.
ifeq ($(OS),Windows_NT)
  EXE_SUFFIX    := .exe
  SHARED_SUFFIX := dll
  STATIC_SUFFIX := lib
  CARGO_SHARED  := rorolala.$(SHARED_SUFFIX)
  CARGO_STATIC  := rorolala.$(STATIC_SUFFIX)
  # A DLL is linked through the import library that names what it exports, and cargo
  # writes that one under the DLL's own name — `rorolala.dll.lib` — so that it does not
  # take the static library's. It records the DLL it belongs to, so it is only handed
  # over beside a DLL of the name it reaches for, which is why the library and not this
  # Makefile is what names the artifact.
  CARGO_IMPORT  := rorolala.$(SHARED_SUFFIX).$(STATIC_SUFFIX)
  IMPORT_SUFFIX := $(SHARED_SUFFIX).$(STATIC_SUFFIX)
else
  EXE_SUFFIX    :=
  SHARED_SUFFIX := $(if $(filter Darwin,$(shell uname -s)),dylib,so)
  STATIC_SUFFIX := a
  CARGO_SHARED  := librorolala.$(SHARED_SUFFIX)
  CARGO_STATIC  := librorolala.$(STATIC_SUFFIX)
  # Nothing to hand over: a linker is given the shared library itself.
  CARGO_IMPORT  :=
  IMPORT_SUFFIX :=
endif

.PHONY: all check lib bin build export clippy doc doc-open fmt fmt-check test integration-test cargo-clean clean

# Default target: the full gate.
all: check build

# The full gate: the formatting of every crate, the test suites, a build of every
# crate, clippy, the documentation, then the integration suites. Composed from the
# targets below rather than repeating their commands.
#
# `fmt-check` comes first because it is instant and it is the one that is forgotten:
# `fmt` rewrites the sources in place, so nothing else here would notice a crate that
# was left unformatted.
check: fmt-check test build clippy doc integration-test

# Builds the C ABI artifact — a release cdylib and staticlib — and, as a side effect
# of the root build script, the C header that describes it.
lib:
	$(CARGO) build $(RELEASE_FLAG) --all-features -p rorolala-ffi

# Builds the release command line programs. Compiling them is also what refreshes
# the completion scripts in $(COMPLETION_DIR).
bin:
	$(CARGO) build $(RELEASE_FLAG) --all-features $(addprefix -p ,$(PROGRAMS))

# Everything that ships: the library and the programs.
build: lib bin

# Lays the build out for hand-off under $(BUILD_DIR), under the names a consumer
# links, includes and sources.
export: build
	rm -rf $(BUILD_DIR)
	mkdir -p $(BUILD_DIR)/lib $(BUILD_DIR)/bin
	cp $(RELEASE_DIR)/$(CARGO_SHARED) $(BUILD_DIR)/lib/rorolala.$(SHARED_SUFFIX)
	cp $(RELEASE_DIR)/$(CARGO_STATIC) $(BUILD_DIR)/lib/rorolala.$(STATIC_SUFFIX)
ifneq ($(CARGO_IMPORT),)
	cp $(RELEASE_DIR)/$(CARGO_IMPORT) $(BUILD_DIR)/lib/rorolala.$(IMPORT_SUFFIX)
endif
	cp $(HEADER) $(BUILD_DIR)/lib/rorolala.h
	cp $(HEADER) $(BUILD_DIR)/lib/rorolala.hpp
	set -e; for program in $(PROGRAMS); do \
		cp $(RELEASE_DIR)/$${program}$(EXE_SUFFIX) $(BUILD_DIR)/bin/$${program}$(EXE_SUFFIX); \
		for shell in $(COMPLETION_SHELLS); do \
			cp $(COMPLETION_DIR)/$${program}_comp.$${shell} \
				$(BUILD_DIR)/bin/$${program}-completion.$${shell}; \
		done; \
	done

# Runs clippy over the workspace; warnings are errors.
clippy:
	$(CARGO) clippy --workspace --all-targets --all-features -- -D warnings

# Builds the API documentation for every crate in the workspace.
doc:
	$(CARGO) doc --workspace --no-deps --all-features

# Same, then opens it in a browser.
doc-open:
	$(CARGO) doc --workspace --no-deps --all-features --open

# Formats every crate in the workspace in place.
fmt:
	$(CARGO) fmt --all

# Verifies that every crate is formatted, without rewriting anything.
fmt-check:
	$(CARGO) fmt --all -- --check

# Runs the test suites. Deliberately left on cargo's default profile: release
# builds turn `debug_assert!` off, which is the opposite of what a test run wants.
test:
	$(CARGO) test --workspace --all-features

# Runs the integration suites under `tests/`. Each is a program of its own — deliberately
# not a member of the workspace — that runs the built programs and checks what they said,
# so what is checked is the program a caller meets rather than the parts it is made of.
#
# A suite is a program rather than a test crate, because what it does is what a caller does:
# it is run, and it complains — by ending non-zero — when what came back was not what it
# expected. That is also what lets one do what a test cannot: serve a Vault, reach it, and
# stop it again.
#
# The suites are a workspace of their own, so they are run from beside it: where a suite is
# run from is what picks its target directory out of `tests/.cargo/config.toml`, which is how
# running them leaves nothing inside the workspace they test. The programs they run are the
# release ones this Makefile builds, named to them rather than looked for.
#
# They run one at a time, so a suite that fails is named rather than lost among the output of
# the others — and the ones after it still run, so one run reports every suite that failed.
integration-test: bin
	set -e; \
	cd tests; \
	failed=; \
	for suite in */; do \
		[ -f "$${suite}Cargo.toml" ] || continue; \
		echo "==> $${suite}"; \
		if ! ROLA_BIN_DIR="$(CURDIR)/$(RELEASE_DIR)" $(CARGO) run --manifest-path "$${suite}Cargo.toml"; then \
			failed="$$failed $${suite}"; \
		fi; \
	done; \
	if [ -n "$$failed" ]; then echo "==> failed:$$failed"; exit 1; fi

# Removes cargo's build output. $(BUILD_DIR) survives: it holds an export, not a
# build.
cargo-clean:
	$(CARGO) clean

# Removes everything the build ever produced: cargo's output, whatever else
# $(TARGET_DIR) holds (the generated header, mingling's completion scripts), .NET's
# output, and the exported $(BUILD_DIR).
clean: cargo-clean
	rm -rf $(TARGET_DIR) $(CS_TARGET_DIR) $(BUILD_DIR)
