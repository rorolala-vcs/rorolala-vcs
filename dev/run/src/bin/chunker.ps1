# Migrated to `cargo dev chunker`; this is the strangler shim kept while both entry points exist. The
# logic lives in `dev/build` now — see `__PLAN_BUILD_CI.md` in the working tree.
& cargo run --quiet --manifest-path dev/build/Cargo.toml --bin rola-build -- chunker @args
exit $LASTEXITCODE
