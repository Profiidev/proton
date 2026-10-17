# Notes for agents

Read `ARCHITECTURE.md` first (crate map, startup, "where do I change X", gotchas), then
`crates/utils/PATTERNS.md` before touching a service crate.

## Commands

```bash
just test                 # cargo nextest run --workspace (one process per test)
cargo nextest run -p proton_ui pages::home   # a subset
just lint                 # cargo clippy --workspace --all-targets -- -D warnings
just fmt                  # cargo fmt --all
oxfmt <files>             # json/md/yaml, from the repo root (devenv has it, CI checks it)
just package deb,appimage # cargo-packager bundles
```

Running the app (`cargo run`) needs the gpui runtime libs on `LD_LIBRARY_PATH`: use the devenv
shell (`direnv allow` / `devenv shell`) or `nix build` (`result/bin/proton`).

## Conventions

- rustfmt with 2-space indent (`rustfmt.toml`). Never prettier; non-Rust files use oxfmt
  (`.oxfmtrc.json`).
- Errors: `anyhow` + `proton_utils::log::ErrorLogExt::log_err()`; logs via `tracing`, never
  `log`. User-facing errors become a toast (`ui::actions::toast_error`).
- Comments: short lowercase "why" comments, `///` doc comments of one line or two, like the
  existing code. No restating what the code does.
- Services follow the Global + `Entity` + `XExt` + handle + `listener.rs` + flume shape. tokio
  work goes through `proton_utils::rt`; tokio tasks never touch entities.
- Notify only on change: `write_changed`, compare before `cx.notify()`.
- Page roots are `size_full()`, transparent and scroll themselves; keep them shallow and put
  live parts into child entities. Unbounded data (logs, search results, content lists) uses
  virtual lists (`uniform_list`/`list`).
- No new abstractions, configs or crates for one use (YAGNI). Reuse widgets in
  `proton_components` and helpers in `proton_utils` before writing new ones.
- Keep old-data compatibility: identifier `io.profidev.proton`, `store.json` keys and format,
  old route URLs, updater key. New fields get `#[serde(default)]`.
- Deliberate shortcuts get a `ponytail:` comment naming the ceiling and the upgrade path.
- Tests that save call `proton_store::init_at` first; UI tests use `init_fixture`. Never write
  the real data dir.
