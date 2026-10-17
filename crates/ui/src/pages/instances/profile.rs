use chrono::{DateTime, Local, Utc};
use gpui_kit::{
  Anchor, AppContext, Context, Entity, FontWeight, InteractiveElement, IntoElement, ParentElement,
  Render, StatefulInteractiveElement, Styled, Subscription, Task, Window,
  assets::IconName,
  component::{
    ActiveTheme, Icon,
    button::Button,
    input::{Input, InputEvent, InputState},
    menu::{DropdownMenu, PopupMenuItem},
  },
  div,
  prelude::FluentBuilder,
  px,
};
use proton_components::widgets::{
  form::open_destroy_dialog,
  format::format_datetime,
  logs::{LOG_LEVELS, LogBuffer, LogFilter, LogWindow},
};
use proton_profiles::ProfilesExt;

use super::{RowView, empty_state, group_by_profile, rows, spawn_toast};

/// Profile tab "Instances": running instances of this profile as cards
pub struct ProfileInstancesPage {
  profile: String,
  pub(super) rows: Vec<Entity<RowView>>,
  _instances: Subscription,
}

impl ProfileInstancesPage {
  pub fn new(profile: String, cx: &mut Context<Self>) -> Self {
    let instances = cx.profiles().instances.clone();
    let mut this = Self {
      profile,
      rows: Vec::new(),
      _instances: cx.observe(&instances, |this: &mut Self, _, cx| {
        this.sync(cx);
        cx.notify();
      }),
    };
    this.sync(cx);
    this
  }

  fn sync(&mut self, cx: &mut Context<Self>) {
    let list = cx.profiles().instances.read(cx).clone();
    let group = group_by_profile(&list)
      .into_iter()
      .find(|g| g[0].profile_id == self.profile)
      .unwrap_or_default();
    self.rows = rows(
      &group,
      true,
      |_| "Are you sure you want to stop this instance?".into(),
      cx,
    );
  }
}

impl Render for ProfileInstancesPage {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let muted = cx.theme().muted_foreground;
    div()
      .id("profile-instances")
      .size_full()
      .overflow_y_scroll()
      .flex()
      .flex_col()
      .gap_3()
      .child(
        div()
          .child(
            div()
              .text_base()
              .font_weight(FontWeight::MEDIUM)
              .child("Running instances"),
          )
          .child(div().mt_0p5().text_xs().text_color(muted).child(
            "Use Launch another above to start more. Pick a different account to play with two \
             at once.",
          )),
      )
      .children(self.rows.iter().cloned())
      .when(self.rows.is_empty(), |el| {
        el.child(
          empty_state(
            "Not running",
            // the reference balances the two lines (`text-balance`)
            "This profile has no running instance.\nPress Play above to start one.",
          )
          .flex_none(),
        )
      })
      .child(div().text_xs().text_color(muted).child(
        "Worlds are shared between instances, so only open a world in one of them at a time.",
      ))
  }
}

/// Profile tab "Logs": stored runs with search, level filter and run picker
pub struct ProfileLogsPage {
  profile: String,
  runs: Entity<Vec<DateTime<Utc>>>,
  selected: Option<DateTime<Utc>>,
  /// Run shown in the buffer
  loaded: Option<DateTime<Utc>>,
  buffer: Entity<LogBuffer>,
  log: Entity<LogWindow>,
  search: Entity<InputState>,
  levels: Vec<&'static str>,
  _load: Option<Task<()>>,
  _subs: [Subscription; 2],
}

impl ProfileLogsPage {
  pub fn new(profile: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
    let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search logs"));
    let profiles = cx.profiles().clone();
    let runs = profiles.runs(&profile, cx);
    let buffer = cx.new(|_| LogBuffer::new());
    let log = cx.new(|cx| LogWindow::new(buffer.clone(), cx).source(true));
    let subs = [
      cx.observe(&runs, |this: &mut Self, _, cx| {
        this.sync_run(cx);
        cx.notify();
      }),
      cx.subscribe(&search, |this: &mut Self, _, e: &InputEvent, cx| {
        if matches!(e, InputEvent::Change) {
          this.apply_filter(cx);
        }
      }),
    ];
    let mut this = Self {
      profile,
      runs,
      selected: None,
      loaded: None,
      buffer,
      log,
      search,
      levels: Vec::new(),
      _load: None,
      _subs: subs,
    };
    this.sync_run(cx);
    this
  }

  /// The selected run if it still exists, else the newest
  fn run(&self, cx: &Context<Self>) -> Option<DateTime<Utc>> {
    let runs = self.runs.read(cx);
    self
      .selected
      .filter(|s| runs.contains(s))
      .or_else(|| runs.first().copied())
  }

  /// Loads the shown run when it changed
  fn sync_run(&mut self, cx: &mut Context<Self>) {
    let run = self.run(cx);
    if run == self.loaded {
      return;
    }
    self.loaded = run;
    let Some(run) = run else {
      self._load = None;
      self.buffer.update(cx, |b, cx| b.clear(cx));
      return;
    };
    let task = cx.profiles().logs(self.profile.clone(), run);
    self._load = Some(cx.spawn(async move |this, cx| {
      let lines = task.await;
      let _ = this.update(cx, |this, cx| match lines {
        Ok(lines) => this
          .buffer
          .update(cx, |b, cx| b.set(lines.iter().map(String::as_str), cx)),
        Err(e) => crate::actions::toast_error("Failed to load the logs", &e, cx),
      });
    }));
  }

  pub(crate) fn apply_filter(&mut self, cx: &mut Context<Self>) {
    let filter = LogFilter::new(&self.search.read(cx).value(), &self.levels);
    self.log.update(cx, |log, cx| log.set_filter(filter, cx));
  }

  pub(crate) fn toggle_level(&mut self, level: &'static str, cx: &mut Context<Self>) {
    let on = !self.levels.contains(&level);
    let levels = &self.levels;
    // canonical order
    self.levels = LOG_LEVELS
      .into_iter()
      .filter(|l| (*l == level && on) || (*l != level && levels.contains(l)))
      .collect();
    self.apply_filter(cx);
    cx.notify();
  }

  fn select(&mut self, run: DateTime<Utc>, cx: &mut Context<Self>) {
    self.selected = Some(run);
    self.sync_run(cx);
    cx.notify();
  }

  #[cfg(test)]
  pub(crate) fn log(&self) -> &Entity<LogWindow> {
    &self.log
  }

  #[cfg(test)]
  pub(crate) fn search(&self) -> &Entity<InputState> {
    &self.search
  }
}

fn format_run(run: DateTime<Utc>) -> String {
  format_datetime(&run.with_timezone(&Local))
}

impl Render for ProfileLogsPage {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let muted = cx.theme().muted_foreground;
    let danger = cx.theme().danger;
    let run = self.run(cx);
    let runs = self.runs.read(cx).clone();
    let levels_label = if self.levels.is_empty() {
      "All levels".to_string()
    } else {
      self.levels.join(", ")
    };
    let run_label = run.map_or("No runs".to_string(), |r| format!("Run: {}", format_run(r)));
    let page = cx.entity().downgrade();
    let level_page = page.clone();

    let levels = Button::new("levels")
      .cursor_pointer()
      .outline()
      .label(levels_label)
      .dropdown_caret(true)
      .dropdown_menu_with_anchor(Anchor::TopRight, move |mut menu, _, cx| {
        let selected = level_page
          .upgrade()
          .map(|p| p.read(cx).levels.clone())
          .unwrap_or_default();
        for level in LOG_LEVELS {
          let page = level_page.clone();
          menu = menu.item(
            PopupMenuItem::new(level)
              .checked(selected.contains(&level))
              .on_click(move |_, _, cx| {
                let _ = page.update(cx, |p, cx| p.toggle_level(level, cx));
              }),
          );
        }
        menu
      });

    let run_menu = Button::new("runs")
      .cursor_pointer()
      .outline()
      .label(run_label)
      .dropdown_caret(true)
      .dropdown_menu_with_anchor(Anchor::TopRight, move |mut menu, _, _| {
        for r in &runs {
          let (page, r) = (page.clone(), *r);
          menu = menu.item(
            PopupMenuItem::new(format_run(r))
              .checked(Some(r) == run)
              .on_click(move |_, _, cx| {
                let _ = page.update(cx, |p, cx| p.select(r, cx));
              }),
          );
        }
        if !runs.is_empty() {
          menu = menu.separator();
        }
        let page = page.clone();
        menu
          .item(
            PopupMenuItem::element(move |_, _| div().text_color(danger).child("Clear runs"))
              .icon(Icon::new(IconName::Trash).text_color(danger))
              .on_click(move |_, window, cx| {
                let Some(profile) = page.upgrade().map(|p| p.read(cx).profile.clone()) else {
                  return;
                };
                open_destroy_dialog(
                  window,
                  cx,
                  "Clear Logs",
                  "Are you sure you want to clear the logs of this profile?",
                  "Clear",
                  move |_, cx| {
                    let task = cx.profiles().clear_logs(profile.clone());
                    spawn_toast("Failed to clear the logs", task, cx);
                  },
                );
              }),
          )
          .max_h(px(320.))
          .scrollable(true)
      });

    div()
      .id("profile-logs")
      .size_full()
      .flex()
      .flex_col()
      .gap(px(10.))
      .child(
        div()
          .flex()
          .flex_wrap()
          .gap_2()
          .child(div().id("search").flex_grow(1.).flex_basis(px(220.)).child(
            Input::new(&self.search).prefix(Icon::new(IconName::Search).size_4().text_color(muted)),
          ))
          .child(levels)
          .child(run_menu),
      )
      .child(div().flex_1().min_h_0().child(self.log.clone()))
  }
}
