default : test

fmt:
  cargo fmt --all

clippy:
  cargo clippy --all --all-targets --all-features

# Runs all tests
test:
  cargo test

# Runs tests for the library
test-lib:
  cd bxmlrs && cargo test

# Runs tests for the binary
test-bin:
  cd bxmlrs-bin && cargo test

# Runs bin with provided argument
run ARGUMENT:
  cd bxmlrs-bin && cargo run -- --file={{ARGUMENT}}
