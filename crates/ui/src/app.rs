//! Startup glue (port of the tauri `lib.rs` setup and `+layout.svelte`): the startup refresh,
//! service → shell bridges, global toasts, route restore/persist and window state.

pub mod single_instance;

use std::{cell::RefCell, collections::HashMap, rc::Rc, time::Duration};

use gpui_kit::{
  AnyWindowHandle, App, AppContext, Bounds, Entity, Task, TitlebarOptions, Window, WindowBounds,
  WindowDecorations, WindowOptions, component::notification::Notification, point, px, size,
};
use proton_account::AccountsExt;
use proton_config::{OfflineEvent, OfflineExt, SettingsExt, settings::WindowState};
use proton_content::Content;
use proton_profiles::{ProfilesEvent, ProfilesExt};
use proton_utils::entity::WriteChangedExt;
use proton_versions::VersionsExt;
use tracing::error;

use crate::{
  actions::toast,
  router::{Route, Router, RouterExt},
  shell::{Shell, ShellData, ShellDataExt, ShellEvent},
};

/// Start route override, e.g. `PROTON_ROUTE=/settings/java` (screenshot automation)
pub const ROUTE_ENV: &str = "PROTON_ROUTE";
/// Route and window bounds are saved this long after the last change
const PERSIST_DELAY: Duration = Duration::from_millis(500);

/// Call once all services are initialized. Opens the main window right away, the network
/// work runs in the background.
pub fn start(cx: &mut App) -> anyhow::Result<AnyWindowHandle> {
  restore_route(cx);
  persist_route(cx);
  bridge(cx);
  toasts(cx);
  startup(cx);
  cx.on_window_closed(|cx, _| {
    if cx.windows().is_empty() {
      cx.quit();
    }
  })
  .detach();
  open_main_window(cx)
}

/// Runs the last call `PERSIST_DELAY` after it was made, earlier calls are dropped
#[derive(Clone, Default)]
struct Debounce(Rc<RefCell<Option<Task<()>>>>);

impl Debounce {
  fn call(&self, cx: &mut App, f: impl FnOnce(&mut App) + 'static) {
    let task = cx.spawn(async move |cx| {
      cx.background_executor().timer(PERSIST_DELAY).await;
      cx.update(f);
    });
    *self.0.borrow_mut() = Some(task);
  }
}

fn initial_route(cx: &App) -> Route {
  std::env::var(ROUTE_ENV)
    .ok()
    .and_then(|url| Route::from_url(&url))
    .or_else(|| {
      cx.settings()
        .get(cx)
        .url
        .as_deref()
        .and_then(Route::from_url)
    })
    .unwrap_or(Route::Home)
}

fn restore_route(cx: &mut App) {
  let route = initial_route(cx);
  cx.router().update(cx, |r, cx| r.replace(route, cx));
}

fn persist_route(cx: &mut App) {
  let debounce = Debounce::default();
  Router::on_change(cx, move |route, cx| {
    let url = route.to_url();
    debounce.call(cx, |cx| proton_config::update(cx, |s| s.url = Some(url)));
  })
  .detach();
}

/// Copies `f(entity)` into the shell data now and on every change, notifies only on change
fn mirror<T: 'static>(cx: &mut App, entity: Entity<T>, f: impl Fn(&T, &mut ShellData) + 'static) {
  let data = cx.shell_data();
  let apply = move |entity: Entity<T>, cx: &mut App| {
    let mut next = data.read(cx).clone();
    f(entity.read(cx), &mut next);
    data.write_changed(cx, next);
  };
  apply(entity.clone(), cx);
  cx.observe(&entity, apply).detach();
}

fn bridge(cx: &mut App) {
  mirror(cx, cx.offline().offline.clone(), |v, d| d.offline = *v);
  mirror(cx, cx.settings().settings.clone(), |s, d| {
    d.hide_window_controls = s.hide_window_controls
  });
  mirror(cx, cx.profiles().profiles.clone(), |list, d| {
    d.names.profiles = list
      .iter()
      .map(|p| (p.id.clone(), p.name.clone()))
      .collect();
  });
  mirror(cx, cx.profiles().instances.clone(), |list, d| {
    d.names.instances = list
      .iter()
      .map(|i| (i.id.clone(), i.profile_name.clone()))
      .collect();
    d.running = list.len();
  });
  if let Some(content) = cx.try_global::<Content>() {
    mirror(cx, content.updates.clone(), |updates, d| {
      d.content_updates = updates
        .0
        .iter()
        .map(|(id, list)| (id.clone(), list.len()))
        .filter(|(_, n)| *n > 0)
        .collect::<HashMap<_, _>>();
    });
  }

  cx.subscribe(&cx.shell_data(), |_, ShellEvent::Reconnect, cx| {
    let reconnect = cx.offline().try_reconnect();
    cx.spawn(async move |cx| {
      let note = match reconnect.await {
        true => Notification::success("Reconnected successfully"),
        false => Notification::error("Failed to reconnect"),
      };
      cx.update(|cx| toast(note, cx));
    })
    .detach();
  })
  .detach();
}

fn toasts(cx: &mut App) {
  cx.subscribe(&cx.profiles().events.clone(), |_, event, cx| {
    if let ProfilesEvent::InstanceCrashed { profile_name } = event {
      let text = format!("An instance of profile {profile_name} has crashed.");
      toast(Notification::warning(text), cx);
    }
  })
  .detach();
  cx.subscribe(&cx.offline().events.clone(), |_, event, cx| match event {
    OfflineEvent::ManifestRefreshError => {
      toast(Notification::error("Failed to refresh manifests."), cx)
    }
    // the startup refresh could not run, catch up now (old `spawn_refresh`)
    OfflineEvent::Reconnected => refresh_manifests(cx).detach(),
  })
  .detach();
}

/// Refreshes the manifests and reports the outcome to the offline handle
fn refresh_manifests(cx: &mut App) -> Task<()> {
  let task = cx.versions().clone().refresh_manifests(cx);
  let handle = cx.offline().handle.clone();
  cx.spawn(async move |_| match task.await {
    Ok(()) => handle.set_init(),
    Err(e) => {
      error!("Failed to refresh manifests: {e:#}");
      handle.manifest_refresh_failed();
    }
  })
}

/// Online check → manifest refresh → content updates loop; accounts refresh in parallel
fn startup(cx: &mut App) {
  let online = cx.offline().try_reconnect();
  cx.spawn(async move |cx| {
    if online.await {
      cx.update(crate::updater::check_on_start);
      cx.update(refresh_manifests).await;
    } else {
      cx.update(|cx| {
        // ponytail: the old toast stayed 10s, gpui-kit notifications have one fixed duration
        let text = "You are currently offline, some features may not work";
        toast(Notification::warning(text), cx);
        // the old startup reported the skipped refresh as an error too
        cx.offline().handle.manifest_refresh_failed();
      });
    }
    cx.update(|cx| {
      if let Some(content) = cx.try_global::<Content>() {
        content.start_updates_loop();
      }
    });
  })
  .detach();

  let accounts = cx.accounts().refresh_all();
  cx.spawn(async move |cx| {
    if accounts.await.is_err() {
      cx.update(|cx| toast(Notification::error("Failed to refresh Accounts"), cx));
    }
  })
  .detach();
}

const DEFAULT_SIZE: (f32, f32) = (1300., 800.);
const MIN_SIZE: (f32, f32) = (600., 400.);

/// The saved bounds if they still touch a display, else centered at the saved size
fn restored_bounds(cx: &App) -> WindowBounds {
  let saved = cx.settings().get(cx).window;
  let (w, h) = saved.map_or(DEFAULT_SIZE, |s| (s.width, s.height));
  let size = size(px(w.max(MIN_SIZE.0)), px(h.max(MIN_SIZE.1)));
  let bounds = saved
    .map(|s| Bounds::new(point(px(s.x), px(s.y)), size))
    .filter(|b| cx.displays().iter().any(|d| d.bounds().intersects(b)))
    .unwrap_or_else(|| Bounds::centered(None, size, cx));
  match saved {
    Some(s) if s.maximized => WindowBounds::Maximized(bounds),
    _ => WindowBounds::Windowed(bounds),
  }
}

fn window_state(window: &Window) -> WindowState {
  let (b, maximized) = match window.window_bounds() {
    WindowBounds::Windowed(b) => (b, false),
    WindowBounds::Maximized(b) | WindowBounds::Fullscreen(b) => (b, true),
  };
  WindowState {
    x: b.origin.x.into(),
    y: b.origin.y.into(),
    width: b.size.width.into(),
    height: b.size.height.into(),
    // like the tauri app: tiling compositors (e.g. Hyprland) report tiled windows as
    // maximized, restoring that would force a maximized layout
    maximized: maximized && !cfg!(target_os = "linux"),
  }
}

/// Saves the window state debounced on resize/move and once more on quit
fn track_window(window: &mut Window, cx: &mut gpui_kit::Context<Shell>) {
  let last = Rc::new(RefCell::new(None::<WindowState>));
  let debounce = Debounce::default();
  let seen = last.clone();
  cx.observe_window_bounds(window, move |_, window, cx| {
    let state = window_state(window);
    *seen.borrow_mut() = Some(state);
    debounce.call(cx, move |cx| {
      proton_config::update(cx, |s| s.window = Some(state))
    });
  })
  .detach();
  cx.on_app_quit(move |_, cx| {
    if let Some(state) = *last.borrow() {
      proton_config::update(cx, |s| s.window = Some(state));
    }
    async {}
  })
  .detach();
}

fn open_main_window(cx: &mut App) -> anyhow::Result<AnyWindowHandle> {
  let options = WindowOptions {
    window_bounds: Some(restored_bounds(cx)),
    titlebar: Some(TitlebarOptions {
      title: Some("Proton".into()),
      appears_transparent: true,
      traffic_light_position: None,
    }),
    window_decorations: Some(WindowDecorations::Client),
    window_min_size: Some(size(px(MIN_SIZE.0), px(MIN_SIZE.1))),
    app_id: Some("io.profidev.proton".into()),
    ..Default::default()
  };
  let (handle, _) = proton_components::assets::open_window(options, cx, |window, cx| {
    cx.new(|cx| {
      track_window(window, cx);
      Shell::new(window, cx)
    })
  })?;
  Ok(handle)
}

#[cfg(test)]
mod tests;
