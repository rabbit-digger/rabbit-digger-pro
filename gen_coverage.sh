# Make sure you have cargo-tarpaulin installed: `cargo install cargo-tarpaulin`

cargo tarpaulin --target-dir ./coverage/target --workspace --out Lcov --output-dir coverage --skip-clean
