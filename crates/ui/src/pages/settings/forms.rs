//! `GameSettings` / `JavaSettings` forms, shared by the global settings pages and the profile
//! settings tabs.
//!
//! Both are views that own their inputs. The owner passes the current value and an `on_change`
//! that persists it, then pushes later values back with `set_settings`:
//! ```ignore
//! let form = cx.new(|cx| GameSettingsForm::new(settings.clone(), window, cx)
//!   .on_change(|next, _, cx| proton_config::update(cx, |s| s.minecraft.game_settings = next.clone())));
//! // when the source changes (observe the settings or profile entity):
//! form.update(cx, |f, cx| f.set_settings(new_value, window, cx));
//! form.update(cx, |f, cx| f.set_disabled(!use_local, cx));
//! ```
//! `JavaSettingsForm::new` also takes `max_mem` (MB), a memory label fn
//! ([`memory_with_mb`] / [`memory_of_total`]) and the hint text below the slider.

use std::{collections::HashMap, rc::Rc};

use gpui_kit::{
  App, AppContext, Context, ElementId, Entity, InteractiveElement, IntoElement, ParentElement,
  Render, SharedString, Styled, Subscription, Window,
  assets::IconName,
  base::TestSupportExt,
  component::{
    ActiveTheme, Disableable, Icon, Sizable,
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
    notification::NotificationType,
    switch::Switch,
  },
  div,
  prelude::FluentBuilder,
  px,
};
use proton_components::widgets::Handler;
use proton_components::widgets::label;
use proton_components::widgets::slider::{Slider, SliderEvent};
use proton_config::settings::{GameSettings, JvmSettings};

use super::toast;

/// Window size: custom size switch plus width × height
pub struct GameSettingsForm {
  settings: GameSettings,
  disabled: bool,
  width: Entity<InputState>,
  height: Entity<InputState>,
  on_change: Option<Handler<GameSettings>>,
  _subs: Vec<Subscription>,
}

impl GameSettingsForm {
  pub fn new(settings: GameSettings, window: &mut Window, cx: &mut Context<Self>) -> Self {
    let width = cx.new(|cx| {
      InputState::new(window, cx)
        .placeholder("854")
        .default_value(settings.width.to_string())
    });
    let height = cx.new(|cx| {
      InputState::new(window, cx)
        .placeholder("480")
        .default_value(settings.height.to_string())
    });
    let subs = [(&width, true), (&height, false)]
      .map(|(state, is_width)| {
        cx.subscribe_in(state, window, move |this, _, e: &InputEvent, window, cx| {
          if matches!(e, InputEvent::Blur | InputEvent::PressEnter { .. }) {
            this.save_size(is_width, window, cx);
          }
        })
      })
      .into();
    Self {
      settings,
      disabled: false,
      width,
      height,
      on_change: None,
      _subs: subs,
    }
  }

  pub fn on_change(mut self, f: impl Fn(&GameSettings, &mut Window, &mut App) + 'static) -> Self {
    self.on_change = Some(Rc::new(f));
    self
  }

  pub fn settings(&self) -> &GameSettings {
    &self.settings
  }

  /// Shows a value saved elsewhere (or the result of `on_change`)
  pub fn set_settings(
    &mut self,
    settings: GameSettings,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    if settings == self.settings {
      return;
    }
    for (state, value) in [
      (&self.width, settings.width),
      (&self.height, settings.height),
    ] {
      if state.read(cx).value().trim().parse() != Ok(value) {
        state.update(cx, |s, cx| s.set_value(value.to_string(), window, cx));
      }
    }
    self.settings = settings;
    cx.notify();
  }

  pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
    if disabled != self.disabled {
      self.disabled = disabled;
      cx.notify();
    }
  }

  fn change(&mut self, next: GameSettings, window: &mut Window, cx: &mut Context<Self>) {
    if let Some(f) = self.on_change.clone() {
      f(&next, window, cx);
    }
  }

  fn save_size(&mut self, is_width: bool, window: &mut Window, cx: &mut Context<Self>) {
    let state = if is_width { &self.width } else { &self.height };
    let value: usize = state.read(cx).value().trim().parse().unwrap_or(0);
    let mut next = self.settings.clone();
    let slot = if is_width {
      &mut next.width
    } else {
      &mut next.height
    };
    if value > 0 && value != *slot {
      *slot = value;
      self.change(next, window, cx);
    }
  }
}

impl Render for GameSettingsForm {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let muted = cx.theme().muted_foreground;
    let sizes_disabled = self.disabled || !self.settings.use_custom;
    let use_custom = self.settings.use_custom;
    div()
      .id("game-settings")
      .flex()
      .flex_col()
      .gap(px(6.))
      .child(
        div()
          .flex()
          .items_center()
          .justify_between()
          .gap_2()
          .child(label("Window size"))
          .child(
            div()
              .flex()
              .items_center()
              .gap_2()
              .text_xs()
              .text_color(muted)
              .child("Custom size (1.13+)")
              .child(
                Switch::new("use-custom")
                  .cursor_pointer()
                  .small()
                  .checked(use_custom)
                  .disabled(self.disabled)
                  .when(self.disabled, |b| b.cursor_default())
                  .on_change(cx.listener(|this, checked: &bool, window, cx| {
                    let mut next = this.settings.clone();
                    next.use_custom = *checked;
                    this.change(next, window, cx);
                  })),
              ),
          ),
      )
      .child(
        div()
          .flex()
          .items_center()
          .gap_2()
          .child(
            div()
              .w(px(100.))
              .child(Input::new(&self.width).disabled(sizes_disabled)),
          )
          .child(div().text_color(muted).child("×"))
          .child(
            div()
              .w(px(100.))
              .child(Input::new(&self.height).disabled(sizes_disabled)),
          ),
      )
      .test_support()
  }
}

/// `{x.x} GB ({mb} MB)`, the global Java page
pub fn memory_with_mb(mb: u64) -> String {
  format!("{:.1} GB ({mb} MB)", mb as f64 / 1024.)
}

/// `{x.x} GB of {total} GB`, the profile Java tab
pub fn memory_of_total(mb: u64, total_mb: u64) -> String {
  format!(
    "{:.1} GB of {:.1} GB",
    mb as f64 / 1024.,
    total_mb as f64 / 1024.
  )
}

/// Splits a java argument string on whitespace
pub fn parse_args(args: &str) -> Vec<String> {
  args.split_whitespace().map(String::from).collect()
}

/// Env vars sorted by key
pub fn sorted_env(env: &HashMap<String, String>) -> Vec<(&String, &String)> {
  let mut list: Vec<_> = env.iter().collect();
  list.sort();
  list
}

/// The env map with `key=value` added, or the error toast text
pub fn add_env_var(
  env: &HashMap<String, String>,
  key: &str,
  value: &str,
) -> Result<HashMap<String, String>, String> {
  let (key, value) = (key.trim(), value.trim());
  if key.is_empty() || value.is_empty() {
    return Err("Both key and value must be provided".into());
  }
  if key.contains('=') {
    return Err(r#"Environment variable key cannot contain "=""#.into());
  }
  if env.contains_key(key) {
    return Err(format!(r#"Environment variable "{key}" already exists"#));
  }
  let mut env = env.clone();
  env.insert(key.into(), value.into());
  Ok(env)
}

type MemoryLabel = Rc<dyn Fn(u64) -> String>;

/// Memory slider, java arguments, environment variables
pub struct JavaSettingsForm {
  settings: JvmSettings,
  disabled: bool,
  /// live slider value while dragging, saved on release
  mem: u64,
  memory_label: MemoryLabel,
  memory_hint: SharedString,
  slider: Entity<Slider>,
  args: Entity<InputState>,
  key: Entity<InputState>,
  value: Entity<InputState>,
  on_change: Option<Handler<JvmSettings>>,
  _subs: Vec<Subscription>,
}

impl JavaSettingsForm {
  pub fn new(
    settings: JvmSettings,
    max_mem: u64,
    memory_label: impl Fn(u64) -> String + 'static,
    memory_hint: impl Into<SharedString>,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) -> Self {
    let mem = settings.mem_max as u64;
    let slider = cx.new(|cx| Slider::new(1024., max_mem.max(1024) as f32, 256., mem as f32, cx));
    let args = cx.new(|cx| {
      InputState::new(window, cx)
        .placeholder("-XX:+UseG1GC")
        .default_value(settings.args.join(" "))
    });
    let key = cx.new(|cx| InputState::new(window, cx).placeholder("KEY"));
    let value = cx.new(|cx| InputState::new(window, cx).placeholder("value"));
    let subs = vec![
      cx.subscribe_in(
        &slider,
        window,
        |this, _, e: &SliderEvent, window, cx| match e {
          SliderEvent::Change(v) => {
            this.mem = *v as u64;
            cx.notify();
          }
          SliderEvent::Release(v) => {
            let mem = *v as usize;
            if mem != this.settings.mem_max {
              let mut next = this.settings.clone();
              next.mem_max = mem;
              this.change(next, window, cx);
            }
          }
        },
      ),
      cx.subscribe_in(&args, window, |this, args, e: &InputEvent, window, cx| {
        if matches!(e, InputEvent::Blur | InputEvent::PressEnter { .. }) {
          let parsed = parse_args(&args.read(cx).value());
          if parsed != this.settings.args {
            let mut next = this.settings.clone();
            next.args = parsed;
            this.change(next, window, cx);
          }
        }
      }),
      cx.subscribe_in(&key, window, Self::submit_on_enter),
      cx.subscribe_in(&value, window, Self::submit_on_enter),
    ];
    Self {
      settings,
      disabled: false,
      mem,
      memory_label: Rc::new(memory_label),
      memory_hint: memory_hint.into(),
      slider,
      args,
      key,
      value,
      on_change: None,
      _subs: subs,
    }
  }

  pub fn on_change(mut self, f: impl Fn(&JvmSettings, &mut Window, &mut App) + 'static) -> Self {
    self.on_change = Some(Rc::new(f));
    self
  }

  pub fn settings(&self) -> &JvmSettings {
    &self.settings
  }

  /// Shows a value saved elsewhere (or the result of `on_change`)
  pub fn set_settings(
    &mut self,
    settings: JvmSettings,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    if settings == self.settings {
      return;
    }
    if settings.mem_max != self.settings.mem_max {
      self.mem = settings.mem_max as u64;
      let mem = self.mem as f32;
      self.slider.update(cx, |s, cx| s.set_value(mem, cx));
    }
    if parse_args(&self.args.read(cx).value()) != settings.args {
      let joined = settings.args.join(" ");
      self
        .args
        .update(cx, |s, cx| s.set_value(joined, window, cx));
    }
    self.settings = settings;
    cx.notify();
  }

  pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
    if disabled != self.disabled {
      self.disabled = disabled;
      self.slider.update(cx, |s, cx| s.set_disabled(disabled, cx));
      self.slider.update(cx, |s, cx| s.set_disabled(disabled, cx));
      cx.notify();
    }
  }

  fn change(&mut self, next: JvmSettings, window: &mut Window, cx: &mut Context<Self>) {
    if let Some(f) = self.on_change.clone() {
      f(&next, window, cx);
    }
  }

  fn submit_on_enter(
    &mut self,
    _: &Entity<InputState>,
    e: &InputEvent,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    if matches!(e, InputEvent::PressEnter { .. }) {
      self.add_env(window, cx);
    }
  }

  fn add_env(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if self.disabled {
      return;
    }
    let key = self.key.read(cx).value();
    let value = self.value.read(cx).value();
    match add_env_var(&self.settings.env_vars, &key, &value) {
      Ok(env_vars) => {
        let mut next = self.settings.clone();
        next.env_vars = env_vars;
        self.change(next, window, cx);
        for state in [&self.key, &self.value] {
          state.update(cx, |s, cx| s.set_value("", window, cx));
        }
      }
      Err(message) => toast(window, cx, NotificationType::Error, message),
    }
  }

  fn remove_env(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
    let mut next = self.settings.clone();
    next.env_vars.remove(key);
    self.change(next, window, cx);
  }
}

impl Render for JavaSettingsForm {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let theme = cx.theme();
    let muted = theme.muted_foreground;
    let disabled = self.disabled;
    let env = sorted_env(&self.settings.env_vars);
    let env_list = (!env.is_empty()).then(|| {
      div()
        .flex()
        .flex_col()
        .overflow_hidden()
        .rounded(px(14.))
        .border_1()
        .border_color(theme.border)
        .bg(theme.popover.opacity(0.5))
        .children(env.into_iter().enumerate().map(|(ix, (key, value))| {
          let remove = key.clone();
          div()
            .flex()
            .items_center()
            .gap_3()
            .px_3()
            .py(px(10.))
            .when(ix > 0, |d| d.border_t_1().border_color(theme.border))
            .child(
              div()
                .flex_1()
                .min_w_0()
                .font_family("monospace")
                .text_sm()
                .child(format!("{key}={value}")),
            )
            .child(
              Button::new(ElementId::Name(format!("remove-env-{key}").into()))
                .cursor_pointer()
                .ghost()
                .small()
                .icon(Icon::new(IconName::Trash).text_color(theme.danger))
                .tooltip(format!("Remove {key}"))
                .disabled(disabled)
                .when(disabled, |b| b.cursor_default())
                .on_click(
                  cx.listener(move |this, _, window, cx| this.remove_env(&remove, window, cx)),
                ),
            )
        }))
    });

    div()
      .id("java-settings")
      .flex()
      .flex_col()
      .gap_4()
      .child(
        div()
          .flex()
          .flex_col()
          .gap(px(6.))
          .child(
            div()
              .flex()
              .items_center()
              .justify_between()
              .gap_2()
              .child(label("Memory"))
              .child(div().text_xs().child((self.memory_label)(self.mem))),
          )
          .child(
            // zero height: the label, track and hint spacing of the reference comes from the
            // gaps alone, the 4px track overhangs centered on the line
            div()
              .flex()
              .items_center()
              .h(px(0.))
              .child(self.slider.clone()),
          )
          .child(
            div()
              .text_xs()
              .text_color(muted)
              .child(self.memory_hint.clone()),
          ),
      )
      .child(
        div()
          .flex()
          .flex_col()
          .gap(px(6.))
          .child(label("Java arguments"))
          .child(Input::new(&self.args).disabled(disabled)),
      )
      .child(
        div()
          .flex()
          .flex_col()
          .gap(px(6.))
          .child(label("Environment variables"))
          .children(env_list)
          .child(
            div()
              .flex()
              .gap_2()
              .child(
                div()
                  .flex_1()
                  .child(Input::new(&self.key).disabled(disabled)),
              )
              .child(
                div()
                  .flex_1()
                  .child(Input::new(&self.value).disabled(disabled)),
              )
              .child(
                Button::new("add-env")
                  .cursor_pointer()
                  .outline()
                  .label("Add")
                  .disabled(disabled)
                  .when(disabled, |b| b.cursor_default())
                  .on_click(cx.listener(|this, _, window, cx| this.add_env(window, cx))),
              ),
          ),
      )
      .test_support()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn env_vars_validate() {
    let env = HashMap::from([("A".to_string(), "1".to_string())]);
    assert_eq!(
      add_env_var(&env, " ", "x").unwrap_err(),
      "Both key and value must be provided"
    );
    assert_eq!(
      add_env_var(&env, "B", " ").unwrap_err(),
      "Both key and value must be provided"
    );
    assert_eq!(
      add_env_var(&env, "A=B", "x").unwrap_err(),
      r#"Environment variable key cannot contain "=""#
    );
    assert_eq!(
      add_env_var(&env, "A", "x").unwrap_err(),
      r#"Environment variable "A" already exists"#
    );
    let next = add_env_var(&env, " B ", " 2 ").unwrap();
    assert_eq!(next.get("B").map(String::as_str), Some("2"));
    assert_eq!(
      sorted_env(&next),
      [
        (&"A".to_string(), &"1".to_string()),
        (&"B".to_string(), &"2".to_string())
      ]
    );
  }

  #[test]
  fn args_and_labels() {
    assert_eq!(
      parse_args("  -Xss1M   -XX:+UseG1GC "),
      ["-Xss1M", "-XX:+UseG1GC"]
    );
    assert!(parse_args("   ").is_empty());
    assert_eq!(memory_with_mb(4096), "4.0 GB (4096 MB)");
    assert_eq!(memory_of_total(6144, 14016), "6.0 GB of 13.7 GB");
  }
}
