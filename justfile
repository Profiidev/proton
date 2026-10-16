# Every test in the workspace.
test *args:
  cargo nextest run --workspace {{args}}

lint:
  cargo clippy --workspace --all-targets -- -D warnings

fmt:
  cargo fmt --all
