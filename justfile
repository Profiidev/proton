# Every test in the workspace.
test *args:
  cargo nextest run --workspace {{args}}

lint:
  cargo clippy --workspace --all-targets -- -D warnings

fmt:
  cargo fmt --all

# Screenshot every route of the Tauri app into fixtures/reference (Hyprland), optional slug filter
reference *filter:
  fixtures/scripts/reference.sh {{filter}}

# Screenshot every route of the gpui app into fixtures/gpui (own headless sway), optional slug filter
screenshots *filter:
  fixtures/scripts/screenshots.sh {{filter}}

# Side-by-side montages + RMSE report of fixtures/reference vs fixtures/gpui into fixtures/compare
compare *filter:
  fixtures/scripts/compare.sh {{filter}}
