use gpui_kit::{
  AppContext, Context, Entity, FontWeight, InteractiveElement, IntoElement, ParentElement, Render,
  StyleRefinement, Styled, Subscription, Window,
  component::{
    button::{Button, ButtonVariants, Toggle, ToggleGroup, ToggleVariants},
    input::{Input, InputEvent, InputState},
    menu::{DropdownMenu, PopupMenuItem},
    switch::Switch,
  },
  div, px,
};
use proton_components::widgets::{
  colors,
  logs::{CONSOLE_ALL, CONSOLE_ERR, CONSOLE_WARN, FollowChanged, LogFilter, LogWindow},
};
use proton_profiles::{ProfilesExt, instance::InstanceInfo};

use super::{confirm_stop, group_by_profile};
use crate::router::{Route, RouterExt};

const MODES: [(&str, &[&str]); 3] = [
  ("All", CONSOLE_ALL),
  ("Warnings", CONSOLE_WARN),
  ("Errors", CONSOLE_ERR),
];

/// `/instances/info/logs?id=`: live console of a running instance. A notify re-renders every
/// ancestor view, so this root stays tiny: new lines re-render only the [`LogWindow`] and this
/// root, never the cached [`ConsoleBar`].
pub struct InstanceLogsPage {
  id: String,
  bar: Entity<ConsoleBar>,
  pub(super) log: Option<Entity<LogWindow>>,
  follow: bool,
  _subs: Vec<Subscription>,
}

impl InstanceLogsPage {
  pub fn new(id: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
    let log = cx
      .profiles()
      .instance_logs(&id, cx)
      .map(|buffer| cx.new(|cx| LogWindow::new(buffer, cx)));
    let bar_id = id.clone();
    let bar_log = log.clone();
    let bar = cx.new(|cx| ConsoleBar::new(bar_id, bar_log, window, cx));
    let mut subs = vec![cx.observe(&bar, |_, _, cx| cx.notify())];
    if let Some(log) = &log {
      subs.push(
        cx.subscribe(log, |this: &mut Self, _, FollowChanged(f), cx| {
          this.follow = *f;
          cx.notify();
        }),
      );
    }
    Self {
      id,
      bar,
      log,
      follow: true,
      _subs: subs,
    }
  }

  #[cfg(test)]
  pub(super) fn bar(&self) -> &Entity<ConsoleBar> {
    &self.bar
  }
}

impl Render for InstanceLogsPage {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let running = self.bar.read(cx).info.is_some();
    let Some(log) = self.log.clone().filter(|_| running) else {
      // the instance exited (or never existed)
      let route = Route::InstanceLogs {
        id: self.id.clone(),
      };
      cx.defer_in(window, move |_, _, cx| {
        if cx.route() == route {
          cx.router()
            .update(cx, |r, cx| r.replace(Route::Instances, cx));
        }
      });
      return div().mt_2().ml_2().child("Loading...").into_any_element();
    };
    let follow_log = log.clone();

    div()
      .id("instance-logs-page")
      .size_full()
      .flex()
      .flex_col()
      .gap_4()
      .px_5()
      .pt_4()
      .pb_6()
      .child(
        self
          .bar
          .clone()
          // cached views take their size from this style: header row + filter row
          .cached(StyleRefinement::default().w_full().h(px(88.)).flex_none()),
      )
      .child(div().flex_1().min_h(px(480.)).child(log))
      .child(
        Switch::new("follow")
          .checked(self.follow)
          .label("Follow new output")
          .on_click(move |checked, _, cx| {
            follow_log.update(cx, |log, cx| log.set_follow(*checked, cx))
          }),
      )
      .into_any_element()
  }
}

/// Title, instance picker, stop button, search and level toggle
pub struct ConsoleBar {
  id: String,
  info: Option<InstanceInfo>,
  /// Instances of the same profile, oldest first
  siblings: Vec<InstanceInfo>,
  log: Option<Entity<LogWindow>>,
  search: Entity<InputState>,
  mode: usize,
  #[cfg(test)]
  pub(super) renders: usize,
  _subs: [Subscription; 2],
}

impl ConsoleBar {
  fn new(
    id: String,
    log: Option<Entity<LogWindow>>,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) -> Self {
    let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search console"));
    let instances = cx.profiles().instances.clone();
    let subs = [
      cx.observe(&instances, |this: &mut Self, _, cx| {
        this.sync(cx);
        cx.notify();
      }),
      cx.subscribe(&search, |this: &mut Self, _, e: &InputEvent, cx| {
        if matches!(e, InputEvent::Change) {
          this.apply_filter(cx);
        }
      }),
    ];
    let mut this = Self {
      id,
      info: None,
      siblings: Vec::new(),
      log,
      search,
      mode: 0,
      #[cfg(test)]
      renders: 0,
      _subs: subs,
    };
    this.sync(cx);
    this
  }

  fn sync(&mut self, cx: &mut Context<Self>) {
    let list = cx.profiles().instances.read(cx).clone();
    self.info = list.iter().find(|i| i.id == self.id).cloned();
    self.siblings = self
      .info
      .as_ref()
      .and_then(|info| {
        group_by_profile(&list)
          .into_iter()
          .find(|g| g[0].profile_id == info.profile_id)
      })
      .unwrap_or_default();
  }

  fn apply_filter(&mut self, cx: &mut Context<Self>) {
    let filter = LogFilter::new(&self.search.read(cx).value(), MODES[self.mode].1);
    if let Some(log) = &self.log {
      log.update(cx, |log, cx| log.set_filter(filter, cx));
    }
  }

  fn set_mode(&mut self, mode: usize, cx: &mut Context<Self>) {
    if mode != self.mode {
      self.mode = mode;
      self.apply_filter(cx);
      cx.notify();
    }
  }
}

fn sibling_label(siblings: &[InstanceInfo], ix: usize) -> String {
  match siblings.get(ix).and_then(|s| s.account_name.as_deref()) {
    Some(account) if !account.is_empty() => format!("Instance {} · {account}", ix + 1),
    _ => format!("Instance {}", ix + 1),
  }
}

impl Render for ConsoleBar {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    #[cfg(test)]
    {
      self.renders += 1;
    }
    let Some(info) = self.info.clone() else {
      return div().into_any_element();
    };
    let current = self
      .siblings
      .iter()
      .position(|s| s.id == info.id)
      .unwrap_or(0);
    let siblings = self.siblings.clone();
    let stop_text = format!(
      "Are you sure you want to stop the instance of profile \"{}\"?",
      info.profile_name
    );
    let stop_info = info.clone();

    let header = div()
      .flex()
      .flex_wrap()
      .items_center()
      .gap_3()
      .child(
        div()
          .size_2()
          .flex_none()
          .rounded_full()
          .bg(colors::green_500()),
      )
      .child(
        div()
          .flex_1()
          .text_xl()
          .font_weight(FontWeight::SEMIBOLD)
          .child(format!("{} console", info.profile_name)),
      )
      .child(
        Button::new("instance-select")
          .outline()
          .w(px(220.))
          .justify_between()
          .label(sibling_label(&siblings, current))
          .dropdown_caret(true)
          .dropdown_menu(move |mut menu, _, _| {
            for (ix, sibling) in siblings.iter().enumerate() {
              let id = sibling.id.clone();
              menu = menu.item(
                PopupMenuItem::new(sibling_label(&siblings, ix))
                  .checked(ix == current)
                  .on_click(move |_, _, cx| cx.navigate(Route::InstanceLogs { id: id.clone() })),
              );
            }
            menu.min_w(px(220.))
          }),
      )
      .child(
        Button::new("stop")
          .danger()
          .label("Stop")
          .on_click(move |_, window, cx| {
            confirm_stop(&stop_info, stop_text.clone().into(), window, cx)
          }),
      );

    let modes = ToggleGroup::new("mode")
      .outline()
      .segmented()
      .children(
        MODES
          .iter()
          .enumerate()
          .map(|(ix, (label, _))| Toggle::new(ix).label(*label).checked(ix == self.mode)),
      )
      .on_click(cx.listener(|this, checked: &Vec<bool>, _, cx| {
        // single select that can not be cleared
        if let Some(ix) = (0..checked.len()).find(|&ix| checked[ix] && ix != this.mode) {
          this.set_mode(ix, cx);
        }
      }));

    div()
      .flex()
      .flex_col()
      .gap_4()
      .child(header)
      .child(
        div()
          .flex()
          .flex_wrap()
          .gap_2()
          .child(
            div()
              .id("search")
              .flex_grow(1.)
              .flex_basis(px(220.))
              .child(Input::new(&self.search)),
          )
          .child(modes),
      )
      .into_any_element()
  }
}
