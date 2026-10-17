# Service crate patterns

Reference: `crates/config` (offline + settings). `account`, `profiles`, `versions` and `content` use the same shape.

## Layout

- `lib.rs`: a `Global` struct of `Entity<T>` fields, plus a cheap `handle` for tokio code, an
  `XExt` trait on `App` (`cx.offline()`), `init(cx)`, and methods that return futures.
- `listener.rs`: one `cx.spawn` that drains the handle's flume channel into the entities.
- pure modules (`offline.rs`, `settings.rs`, ...): no gpui, plain `#[test]`s.

```rust
#[derive(Clone)]
pub struct Offline { pub offline: Entity<bool>, pub events: Entity<OfflineEvents>, pub handle: OfflineHandle }
impl Global for Offline {}
pub trait OfflineExt { fn offline(&self) -> &Offline; }
impl OfflineExt for App { fn offline(&self) -> &Offline { self.global::<Offline>() } }
```

## Tokio code reporting to gpui state

The tokio side gets a `Clone` handle: `Arc` atomics for instant reads plus a `flume::Sender<Msg>`.
The listener applies each message on the gpui thread with `write_changed` (notifies only when the
value changed) or emits an event. See `offline.rs` (`OfflineHandle`) and `listener.rs`:

```rust
cx.spawn(async move |cx| {
  while let Ok(msg) = rx.recv_async().await {
    match msg {
      OfflineMsg::Offline(v) => offline.write_changed(cx, v),
      OfflineMsg::Reconnected => events.update(cx, |_, cx| cx.emit(OfflineEvent::Reconnected)),
      ..
    }
  }
}).detach();
```

- One entity per independently changing value (fine-grained re-renders). Events that UI shows as
  toasts go on a separate `EventEmitter` entity, so toast subscribers are not woken by state.
- Progress: send ticks through the same channel, keep one entity per row, throttle with
  `future::UpdateLimiter`.
- After a mutation the service refreshes its own entity (the Tauri `update_data` events are gone).

## Calling tokio code from gpui

gpui's executor is not tokio. reqwest, `tokio::fs/net/process` must run on `proton_utils::rt`:

```rust
cx.spawn(async move |this, cx| {
  let value = rt::spawn(async move { fetch().await }).await??; // JoinError, then your error
  this.update(cx, |view, cx| { view.value = value; cx.notify(); })
}).detach_and_log_err(cx); // `use gpui_kit::TaskExt`, or keep the Task in a field
```

Service methods return `impl Future + use<>` that does the `rt::spawn` itself
(`Offline::try_reconnect`). `rt::block_on` only at startup. Tokio tasks never touch gpui state,
they send through the handle.

## Settings and store

- Read: `cx.settings().get(cx)`; observe `cx.settings().settings`.
- Write: `proton_config::update(cx, |s| s.auto_update = true)` saves to the store and notifies.
- Code without a gpui context: `Settings::load()?` (reads `store.json`).
- Store: `proton_store::store().get_or_default(KEY)?`, `.set(KEY, &v)?` (saves atomically),
  `.delete(KEY)?`. Same file and format as the tauri app.
- Paths: `proton_utils::dir::{data_dir, cache_dir, log_dir, download_dir}()?`
  replace `handle.path().app_*_dir()?` one to one.

## Errors and logs

`anyhow` + `proton_utils::log::ErrorLogExt::log_err()` (logs at error, returns the result).
`tracing::{debug, info, warn, error}`, never `log`. User-facing errors become an event or a
returned `Result` the UI turns into a toast.

## Tests

- Pure logic: plain `#[test]` / `#[tokio::test]`.
- gpui: `use gpui_kit::{self as gpui, TestAppContext};` then `#[gpui::test] fn x(cx: &mut TestAppContext)`
  with `cx.run_until_parked()` after updates. Awaiting `rt::spawn` in a test needs
  `cx.executor().allow_parking()` and an `async fn` test (see `utils/src/rt.rs`).
- Anything that saves: call `proton_store::init_at(tempdir.join("store.json"))` before the first
  `store()` use, never write the real data dir (see `config/src/lib.rs` tests).
- Count notifications with `cx.observe(&entity, ..)` to prove "notify only on change".
