//! Profile menu actions: duplicate (`DuplicateDialog.svelte`) and export as `.mrpack`

use gpui_kit::{
  App, AppContext, Context, Entity, FontWeight, IntoElement, ParentElement, Render, Styled,
  Subscription, Window,
  component::{
    Disableable, WindowExt,
    button::{Button, ButtonVariants},
    dialog::{DialogDescription, DialogFooter},
    input::{Input, InputEvent, InputState},
    notification::Notification,
  },
  div,
  prelude::FluentBuilder,
  px,
};
use proton_content::ContentExt;
use proton_profiles::ProfilesExt;

use crate::{
  actions::{toast, toast_error},
  router::{ProfileTab, QuickPlayTab, Route, RouterExt},
};

fn profile_name(id: &str, cx: &App) -> Option<String> {
  cx.profiles()
    .profiles
    .read(cx)
    .iter()
    .find(|p| p.id == id)
    .map(|p| p.name.clone())
}

struct Duplicate {
  profile: String,
  original: String,
  name: Entity<InputState>,
  busy: bool,
  _subs: [Subscription; 1],
}

impl Duplicate {
  fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let name = self.name.read(cx).value().trim().to_string();
    if self.busy || name.is_empty() {
      return;
    }
    self.busy = true;
    cx.notify();
    let task = cx.content().duplicate_profile(self.profile.clone(), name);
    let original = self.original.clone();
    cx.spawn_in(window, async move |this, cx| {
      let res = task.await;
      let _ = this.update_in(cx, |this, window, cx| {
        this.busy = false;
        match res {
          Ok(id) => {
            window.close_dialog(cx);
            // the service refreshed the profile list before returning
            cx.navigate(Route::Profile {
              id,
              tab: ProfileTab::QuickPlay(QuickPlayTab::Singleplayer),
            });
          }
          Err(e) => toast_error(&format!("Failed to duplicate profile {original}"), &e, cx),
        }
        cx.notify();
      });
    })
    .detach();
  }
}

impl Render for Duplicate {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let blank = self.name.read(cx).value().trim().is_empty();
    div()
      .flex()
      .flex_col()
      .gap_5()
      .child(
        div()
          .flex()
          .flex_col()
          .gap(px(6.))
          .child(
            div()
              .text_sm()
              .font_weight(FontWeight::MEDIUM)
              .child("Name"),
          )
          .child(Input::new(&self.name)),
      )
      .child(
        DialogFooter::new()
          .child(
            Button::new("cancel")
              .cursor_pointer()
              .outline()
              .label("Cancel")
              .on_click(|_, window, cx| window.close_dialog(cx)),
          )
          .child(
            Button::new("duplicate")
              .cursor_pointer()
              .primary()
              .label("Duplicate")
              .loading(self.busy)
              .disabled(self.busy || blank)
              .when(self.busy || blank, |b| b.cursor_default())
              .on_click(cx.listener(|this, _, window, cx| this.submit(window, cx))),
          ),
      )
  }
}

/// "Duplicate profile" dialog, opens the copy when done
pub fn open_duplicate_dialog(profile: String, window: &mut Window, cx: &mut App) {
  let Some(original) = profile_name(&profile, cx) else {
    return;
  };
  let view = cx.new(|cx| {
    let name = cx.new(|cx| InputState::new(window, cx).default_value(format!("{original} (copy)")));
    let sub = cx.subscribe_in(
      &name,
      window,
      |this: &mut Duplicate, _, e, window, cx| match e {
        InputEvent::PressEnter { .. } => this.submit(window, cx),
        InputEvent::Change => cx.notify(),
        _ => {}
      },
    );
    Duplicate {
      profile,
      original,
      name,
      busy: false,
      _subs: [sub],
    }
  });
  window.open_dialog(cx, move |dialog, _, _| {
    dialog
      .title("Duplicate profile")
      .w(px(480.))
      .child(
        DialogDescription::new()
          .mb_2()
          .child("Copies the profile with its content, settings and worlds."),
      )
      .child(view.clone())
  });
}

/// Asks where to save, then exports the profile as `.mrpack` as a downloads-panel job
pub fn export_mrpack(profile: String, _: &mut Window, cx: &mut App) {
  let Some(name) = profile_name(&profile, cx) else {
    return;
  };
  let dir = proton_utils::dir::download_dir()
    .ok()
    .or_else(std::env::home_dir)
    .unwrap_or_default();
  let path = cx.prompt_for_new_path(&dir, Some(&format!("{name}.mrpack")));
  cx.spawn(async move |cx| {
    let Ok(Ok(Some(path))) = path.await else {
      return;
    };
    let task = cx.update(|cx| {
      cx.content().export_mrpack(
        profile,
        path,
        vec!["config".into(), "options.txt".into()],
        format!("{name}: export"),
      )
    });
    let res = task.await;
    cx.update(|cx| match res {
      Ok(()) => toast(Notification::success(format!("Exported {name}")), cx),
      Err(e) => toast_error(&format!("Failed to export {name}"), &e, cx),
    });
  })
  .detach();
}

#[cfg(test)]
mod tests {
  use gpui_kit::{
    self as gpui, AnyWindowHandle, TestAppContext, component::WindowExt, test::TestWindowExt,
  };

  use super::*;
  use crate::{
    pages::profiles::tests::{init_fixture, open, poll},
    router::RouterExt,
  };

  const VANILLA: &str = "11111111-1111-4111-8111-111111111111";

  struct Empty;

  impl Render for Empty {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
      div().size_full()
    }
  }

  fn window(cx: &mut TestAppContext) -> AnyWindowHandle {
    init_fixture(cx);
    open(cx, |_, _| Empty).0
  }

  fn toasts(cx: &mut TestAppContext, handle: AnyWindowHandle) -> usize {
    cx.update_window(handle, |_, window, cx| window.notifications(cx).len())
      .unwrap()
  }

  #[gpui::test]
  fn unknown_profiles_open_nothing(cx: &mut TestAppContext) {
    let handle = window(cx);
    cx.update_window(handle, |_, w, cx| {
      open_duplicate_dialog("gone".into(), w, cx);
      export_mrpack("gone".into(), w, cx);
      assert!(!w.has_active_dialog(cx));
    })
    .unwrap();
    assert!(!cx.did_prompt_for_new_path());
  }

  #[gpui::test]
  fn duplicate_copies_and_opens_the_copy(cx: &mut TestAppContext) {
    let handle = window(cx);
    cx.update_window(handle, |_, w, cx| {
      open_duplicate_dialog(VANILLA.into(), w, cx);
      w.render_frame(cx);
      assert!(w.has_active_dialog(cx));
      w.within("dialog").click("duplicate", cx);
    })
    .unwrap();
    poll(cx, |cx| {
      cx.profiles()
        .profiles
        .read(cx)
        .iter()
        .any(|p| p.name == "Vanilla Survival (copy)")
    });
    let copy = cx.update(|cx| {
      cx.profiles()
        .profiles
        .read(cx)
        .iter()
        .find(|p| p.name == "Vanilla Survival (copy)")
        .unwrap()
        .id
        .clone()
    });
    assert_ne!(copy, VANILLA);
    poll(cx, |cx| {
      cx.route()
        == Route::Profile {
          id: copy.clone(),
          tab: ProfileTab::QuickPlay(QuickPlayTab::Singleplayer),
        }
    });
    cx.update_window(handle, |_, w, cx| {
      w.render_frame(cx);
      assert!(!w.has_active_dialog(cx));
    })
    .unwrap();
    assert_eq!(toasts(cx, handle), 0);
  }

  #[gpui::test]
  fn export_writes_the_mrpack_and_a_cancel_does_nothing(cx: &mut TestAppContext) {
    let handle = window(cx);
    let dir = tempfile::tempdir().unwrap();
    cx.update_window(handle, |_, w, cx| export_mrpack(VANILLA.into(), w, cx))
      .unwrap();
    cx.run_until_parked();
    cx.simulate_new_path_selection(|_| None);
    cx.run_until_parked();
    assert_eq!(toasts(cx, handle), 0);

    let path = dir.path().join("Vanilla Survival.mrpack");
    cx.update_window(handle, |_, w, cx| export_mrpack(VANILLA.into(), w, cx))
      .unwrap();
    cx.run_until_parked();
    let target = path.clone();
    cx.simulate_new_path_selection(move |_| Some(target));
    poll(cx, |_| path.exists());
    // the success toast
    let handle2 = handle;
    poll(cx, move |cx| {
      handle2
        .update(cx, |_, w, cx| w.notifications(cx).len())
        .unwrap()
        == 1
    });
  }
}
