# Migrated to `cargo ci doc-open`; this is the strangler shim kept while both entry points exist. The
# logic lives in `dev/ci` now — see `__PLAN_BUILD_CI.md` in the working tree.
& cargo run --quiet --manifest-path dev/ci/Cargo.toml --bin rola-ci -- doc-open @args
exit $LASTEXITCODE
