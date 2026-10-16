//! `/instances`, `/instances/info/logs` and the profile Instances / Logs tabs

mod console;
mod list;
mod profile;

use std::{collections::HashMap, time::Duration};

use chrono::{DateTime, Utc};
use gpui_kit::{
  App, AppContext, Context, Entity, Global, IntoElement, ParentElement, Render, SharedString,
  Styled, Subscription, Window,
  component::empty::{Empty, EmptyDescription, EmptyHeader, EmptyTitle},
};
use proton_components::widgets::{
  form::open_destroy_dialog, format::format_duration, instance::InstanceRow,
};
use proton_config::SettingsExt;
use proton_profiles::{ProfilesExt, instance::InstanceInfo};
use proton_utils::entity::WriteChangedExt;

pub use console::InstanceLogsPage;
pub use list::InstancesPage;
pub use profile::{ProfileInstancesPage, ProfileLogsPage};

use crate::{
  actions::toast_error,
  router::{ProfileTab, Route, RouterExt},
  shell::{PageFactory, ShellDataExt},
};

const POLL: Duration = Duration::from_secs(2);

pub fn register(cx: &mut App) {
  PageFactory::register(
    cx,
    |r| matches!(r, Route::Instances),
    |_, _, cx| cx.new(InstancesPage::new).into(),
  );
  PageFactory::register(
    cx,
    |r| matches!(r, Route::InstanceLogs { .. }),
    |r, window, cx| {
      let Route::InstanceLogs { id } = r else {
        unreachable!()
      };
      let id = id.clone();
      cx.new(|cx| InstanceLogsPage::new(id, window, cx)).into()
    },
  );
  PageFactory::register(
    cx,
    |r| {
      matches!(
        r,
        Route::Profile {
          tab: ProfileTab::Instances,
          ..
        }
      )
    },
    |r, _, cx| {
      let id = r.profile_id().unwrap_or_default().to_string();
      cx.new(|cx| ProfileInstancesPage::new(id, cx)).into()
    },
  );
  PageFactory::register(
    cx,
    |r| {
      matches!(
        r,
        Route::Profile {
          tab: ProfileTab::Logs,
          ..
        }
      )
    },
    |r, window, cx| {
      let id = r.profile_id().unwrap_or_default().to_string();
      cx.new(|cx| ProfileLogsPage::new(id, window, cx)).into()
    },
  );

  // sidebar pill
  let instances = cx.profiles().instances.clone();
  let data = cx.shell_data();
  cx.observe(&instances, move |instances, cx| {
    let running = instances.read(cx).len();
    data.update(cx, |d, cx| {
      if d.running != running {
        d.running = running;
        cx.notify();
      }
    });
  })
  .detach();

  start_tick(cx);
}

/// Memory per instance id plus the poll time (so uptimes refresh). Only the rows observe it.
#[derive(Clone, PartialEq)]
pub struct Tick {
  pub memory: HashMap<String, u64>,
  pub now: DateTime<Utc>,
}

struct GlobalTick(Entity<Tick>);

impl Global for GlobalTick {}

fn tick(cx: &App) -> Entity<Tick> {
  cx.global::<GlobalTick>().0.clone()
}

fn shows_instances(route: &Route) -> bool {
  matches!(
    route,
    Route::Instances
      | Route::Profile {
        tab: ProfileTab::Instances,
        ..
      }
  )
}

/// Polls the memory every [`POLL`] while an instances page is shown and a game runs
fn start_tick(cx: &mut App) {
  let entity = cx.new(|_| Tick {
    memory: HashMap::new(),
    now: Utc::now(),
  });
  cx.set_global(GlobalTick(entity.clone()));
  cx.spawn(async move |cx| {
    loop {
      cx.background_executor().timer(POLL).await;
      let poll = cx.update(|cx| {
        let busy = !cx.profiles().instances.read(cx).is_empty();
        (busy && shows_instances(&cx.route())).then(|| cx.profiles().instances_memory())
      });
      let Some(poll) = poll else { continue };
      let memory = poll.await.unwrap_or_default();
      entity.write_changed(
        cx,
        Tick {
          memory,
          now: Utc::now(),
        },
      );
    }
  })
  .detach();
}

/// One instance row / card; re-renders on its own when the [`Tick`] changes
pub struct RowView {
  info: InstanceInfo,
  number: usize,
  card: bool,
  /// Text of the stop confirmation
  stop_text: SharedString,
  _tick: Subscription,
}

impl RowView {
  fn new(
    info: InstanceInfo,
    number: usize,
    card: bool,
    stop_text: SharedString,
    cx: &mut Context<Self>,
  ) -> Self {
    Self {
      info,
      number,
      card,
      stop_text,
      _tick: cx.observe(&tick(cx), |_, _, cx| cx.notify()),
    }
  }
}

impl Render for RowView {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let tick = tick(cx);
    let tick = tick.read(cx);
    let uptime = format_duration((Utc::now() - self.info.launched_at).num_milliseconds());
    let memory = tick.memory.get(&self.info.id).copied();
    let total = cx.settings().get(cx).system_max_mem;
    let info = self.info.clone();
    let id = info.id.clone();
    let stop_text = self.stop_text.clone();

    InstanceRow::new(SharedString::from(info.id.clone()), self.number, uptime)
      .account(info.account_name.clone().map(Into::into))
      .memory(memory, Some(total))
      .card(self.card)
      .on_console(move |_, cx| cx.navigate(Route::InstanceLogs { id: id.clone() }))
      .on_stop(move |window, cx| confirm_stop(&info, stop_text.clone(), window, cx))
  }
}

/// Row views for `group` (one profile, oldest first), numbered from 1
fn rows(
  group: &[InstanceInfo],
  card: bool,
  stop_text: impl Fn(&InstanceInfo) -> String,
  cx: &mut App,
) -> Vec<Entity<RowView>> {
  group
    .iter()
    .enumerate()
    .map(|(ix, info)| {
      let text = stop_text(info).into();
      cx.new(|cx| RowView::new(info.clone(), ix + 1, card, text, cx))
    })
    .collect()
}

fn confirm_stop(info: &InstanceInfo, text: SharedString, window: &mut Window, cx: &mut App) {
  let (profile, id) = (info.profile_id.clone(), info.id.clone());
  open_destroy_dialog(window, cx, "Stop Instance", text, "Stop", move |_, cx| {
    let task = cx.profiles().stop_instance(profile.clone(), id.clone());
    spawn_toast("Failed to stop instance", task, cx);
  });
}

/// Awaits `task`, an error becomes a toast
fn spawn_toast(
  message: &'static str,
  task: impl Future<Output = anyhow::Result<()>> + 'static,
  cx: &mut App,
) {
  cx.spawn(async move |cx| {
    if let Err(e) = task.await {
      cx.update(|cx| toast_error(message, &e, cx));
    }
  })
  .detach();
}

/// Grouped by profile, groups ordered by their first launch, each group oldest first
pub fn group_by_profile(instances: &[InstanceInfo]) -> Vec<Vec<InstanceInfo>> {
  let mut sorted = instances.to_vec();
  sorted.sort_by_key(|i| i.launched_at);
  let mut groups: Vec<Vec<InstanceInfo>> = Vec::new();
  for info in sorted {
    match groups
      .iter_mut()
      .find(|g| g[0].profile_id == info.profile_id)
    {
      Some(group) => group.push(info),
      None => groups.push(vec![info]),
    }
  }
  groups
}

/// `3 games running across 2 profiles.`
pub fn running_summary(games: usize, profiles: usize) -> String {
  let plural = |n: usize, word: &str| format!("{n} {word}{}", if n == 1 { "" } else { "s" });
  format!(
    "{} running across {}.",
    plural(games, "game"),
    plural(profiles, "profile")
  )
}

/// Dashed empty state
fn empty_state(title: &'static str, description: &'static str) -> Empty {
  Empty::new().border_1().header(
    EmptyHeader::new()
      .title(EmptyTitle::new().child(title))
      .description(EmptyDescription::new().child(description)),
  )
}

#[cfg(test)]
mod tests;
