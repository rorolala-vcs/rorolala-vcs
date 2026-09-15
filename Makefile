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
TARGET_DIR ?= .cargo/temp

# Directory `export` lays the hand-off artifacts out in.
BUILD_DIR ?= build

# The only profile this Makefile builds. Everything it produces is release, so
# nothing that ships can carry debug assertions or debug info.
RELEASE_FLAG := --release

# Where cargo writes the release artifacts.
RELEASE_DIR := $(TARGET_DIR)/release

# The generated C header, written by the root build script while `lib` builds.
HEADER := $(RELEASE_DIR)/ffi_bindings/rorolala_ffi.h

# `export` deletes its destination before copying, so refuse a path that would take
# something else with it.
ifneq ($(filter /%,$(BUILD_DIR)),)
  $(error BUILD_DIR must be a relative path, got `$(BUILD_DIR)')
endif
ifeq ($(strip $(BUILD_DIR)),)
  $(error BUILD_DIR must not be empty)
endif

# Platform spellings. Cargo names a Unix shared library with a `lib` prefix and a
# Windows one with a `.dll` suffix, so the exported names are normalised to one
# shape per platform:
#
#   | cargo artifact              | exported as                              |
#   | --------------------------- | ---------------------------------------- |
#   | librorolala_ffi.so          | $(BUILD_DIR)/lib/rorolala.so             |
#   | librorolala_ffi.dylib       | $(BUILD_DIR)/lib/rorolala.dylib          |
#   | rorolala_ffi.dll            | $(BUILD_DIR)/lib/rorolala.dll            |
#   | librorolala_ffi.a           | $(BUILD_DIR)/lib/rorolala.a              |
#   | rorolala_ffi.lib            | $(BUILD_DIR)/lib/rorolala.lib            |
#   | ffi_bindings/rorolala_ffi.h | $(BUILD_DIR)/lib/rorolala.h              |
#   |                             | $(BUILD_DIR)/lib/rorolala.hpp            |
#   | rola[.exe]                  | $(BUILD_DIR)/bin/rola[.exe]              |
#
# The header is C and C++ at once — its declarations sit in an `extern "C"` block —
# so `.hpp` is the same file under the name a C++ project includes.
ifeq ($(OS),Windows_NT)
  EXE_SUFFIX    := .exe
  SHARED_SUFFIX := dll
  STATIC_SUFFIX := lib
  CARGO_SHARED  := rorolala_ffi.$(SHARED_SUFFIX)
  CARGO_STATIC  := rorolala_ffi.$(STATIC_SUFFIX)
else
  EXE_SUFFIX    :=
  SHARED_SUFFIX := $(if $(filter Darwin,$(shell uname -s)),dylib,so)
  STATIC_SUFFIX := a
  CARGO_SHARED  := librorolala_ffi.$(SHARED_SUFFIX)
  CARGO_STATIC  := librorolala_ffi.$(STATIC_SUFFIX)
endif

.PHONY: all check lib bin build export clippy doc doc-open fmt test clean

# Default target: the code compiles and every lint passes.
all: check build

# Type-checks the workspace, tests and examples included. No artifact comes out of
# this, so it stays on the profile cargo defaults to.
check:
	$(CARGO) check --workspace --all-targets --all-features

# Builds the C ABI artifact — a release cdylib and staticlib — and, as a side effect
# of the root build script, the C header that describes it.
lib:
	$(CARGO) build $(RELEASE_FLAG) --all-features -p rorolala-ffi

# Builds the release command line program.
bin:
	$(CARGO) build $(RELEASE_FLAG) --all-features -p rola

# Everything that ships: the library and the program.
build: lib bin

# Lays the build out for hand-off under $(BUILD_DIR), under the names a consumer
# links and includes against.
export: build
	rm -rf $(BUILD_DIR)/lib $(BUILD_DIR)/bin
	mkdir -p $(BUILD_DIR)/lib $(BUILD_DIR)/bin
	cp $(RELEASE_DIR)/$(CARGO_SHARED) $(BUILD_DIR)/lib/rorolala.$(SHARED_SUFFIX)
	cp $(RELEASE_DIR)/$(CARGO_STATIC) $(BUILD_DIR)/lib/rorolala.$(STATIC_SUFFIX)
	cp $(HEADER) $(BUILD_DIR)/lib/rorolala.h
	cp $(HEADER) $(BUILD_DIR)/lib/rorolala.hpp
	cp $(RELEASE_DIR)/rola$(EXE_SUFFIX) $(BUILD_DIR)/bin/rola$(EXE_SUFFIX)

# Runs clippy over the workspace; warnings are errors.
clippy:
	$(CARGO) clippy --workspace --all-targets --all-features -- -D warnings

# Builds the API documentation for every crate in the workspace.
doc:
	$(CARGO) doc --workspace --no-deps

# Same, then opens it in a browser.
doc-open:
	$(CARGO) doc --workspace --no-deps --open

# Formats every crate in the workspace in place.
fmt:
	$(CARGO) fmt --all

# Runs the test suites. Deliberately left on cargo's default profile: release
# builds turn `debug_assert!` off, which is the opposite of what a test run wants.
test:
	$(CARGO) test --workspace --all-features

# Removes cargo's build output. $(BUILD_DIR) is left alone: it holds an export, not
# a build.
clean:
	$(CARGO) clean
