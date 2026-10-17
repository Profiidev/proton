# Every test in the workspace.
test *args:
  cargo nextest run --workspace {{args}}

# Line coverage of the tests, as HTML under target/llvm-cov/html.
coverage:
  cargo llvm-cov nextest --workspace --all-features --html

lint:
  cargo clippy --workspace --all-targets -- -D warnings

fmt:
  cargo fmt --all

# Release packages into target/release (needs cargo-packager), e.g. `just package deb,appimage`
package formats="deb,appimage":
  cargo packager --release -f {{formats}}
