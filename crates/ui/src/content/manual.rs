//! Restricted CurseForge files the user downloads in the browser (`ManualDownloads.svelte`)

use std::{collections::HashMap, path::PathBuf, rc::Rc, time::Duration};

use gpui_kit::{
  App, AppContext, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, Render,
  StatefulInteractiveElement, Styled, Task, Window,
  assets::IconName,
  component::{
    ActiveTheme, Disableable, Icon, Sizable, WindowExt,
    button::{Button, ButtonVariants},
    dialog::DialogFooter,
    spinner::Spinner,
  },
  div,
  prelude::FluentBuilder,
  px,
};
use proton_components::widgets::colors;
use proton_config::SettingsExt;
use proton_content::{ContentExt, model::ManualFile};

/// ponytail: polls every 2s like the web app, a fs watcher if this ever shows up in profiles
const POLL: Duration = Duration::from_secs(2);

pub const MANUAL_HINT: &str = "CurseForge only allows these files to be downloaded from its website. Download each one, it is picked up automatically.";

type OnDone = Rc<dyn Fn(HashMap<String, PathBuf>, &mut Window, &mut App)>;

/// File list with Download buttons; finds each file in the download folders by polling.
/// Notifies when a file is found, read [`ManualDownloads::found`].
pub struct ManualDownloads {
  files: Vec<ManualFile>,
  found: HashMap<String, PathBuf>,
  /// Set when shown as its own dialog
  on_done: Option<OnDone>,
  _poll: Option<Task<()>>,
}

impl ManualDownloads {
  pub fn new(files: Vec<ManualFile>, cx: &mut Context<Self>) -> Self {
    let mut this = Self {
      files,
      found: HashMap::new(),
      on_done: None,
      _poll: None,
    };
    this.poll(cx);
    this
  }

  /// Standalone dialog: waits for `files`, then "Continue" calls `on_done` with file id → path
  pub fn open(
    files: Vec<ManualFile>,
    on_done: impl Fn(HashMap<String, PathBuf>, &mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
  ) {
    let view = cx.new(|cx| {
      let mut this = Self::new(files, cx);
      this.on_done = Some(Rc::new(on_done));
      this
    });
    window.open_dialog(cx, move |dialog, _, _| {
      dialog
        .title("Manual downloads")
        .w(px(520.))
        .child(view.clone())
    });
  }

  /// File id → downloaded file
  pub fn found(&self) -> &HashMap<String, PathBuf> {
    &self.found
  }

  /// Every file was found
  pub fn done(&self) -> bool {
    self
      .files
      .iter()
      .all(|f| self.found.contains_key(&f.file_id))
  }

  fn poll(&mut self, cx: &mut Context<Self>) {
    if self.files.is_empty() {
      return;
    }
    self._poll = Some(cx.spawn(async move |this, cx| {
      loop {
        let Ok(Some(look)) = this.update(cx, |this, _| {
          let missing: Vec<ManualFile> = this
            .files
            .iter()
            .filter(|f| !this.found.contains_key(&f.file_id))
            .cloned()
            .collect();
          (!missing.is_empty()).then_some(missing)
        }) else {
          return;
        };
        let task = cx.update(|cx| cx.content().find_manual(look, cx));
        let found = task.await;
        if !found.is_empty()
          && this
            .update(cx, |this, cx| {
              this.found.extend(found);
              cx.notify();
            })
            .is_err()
        {
          return;
        }
        cx.background_executor().timer(POLL).await;
      }
    }));
  }
}

impl Render for ManualDownloads {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let theme = cx.theme();
    let (muted, border, bg) = (
      theme.muted_foreground,
      theme.border,
      theme.popover.opacity(0.5),
    );
    let dirs = cx.settings().get(cx).download_dirs();
    let hint = if dirs.is_empty() {
      "No download folders set, add one in Settings, Minecraft.".to_string()
    } else {
      let dirs: Vec<_> = dirs.iter().map(|d| d.display().to_string()).collect();
      format!(
        "Watching {}. Change the folders in Settings, Minecraft.",
        dirs.join(", ")
      )
    };
    let green = colors::green_500();
    let done = self.done();
    let on_done = self.on_done.clone();
    let found_paths = self.found.clone();
    let count = self.files.len();
    div()
      .flex()
      .flex_col()
      .gap(px(6.))
      .when(on_done.is_some(), |el| {
        el.child(div().text_sm().child(MANUAL_HINT))
      })
      .child(div().text_xs().text_color(muted).child(hint))
      .child(
        div()
          .id("manual-files")
          .flex()
          .flex_col()
          .max_h(px(256.))
          .overflow_y_scroll()
          .rounded(px(14.))
          .border_1()
          .border_color(border)
          .bg(bg)
          .children(self.files.iter().enumerate().map(|(ix, file)| {
            let found = self.found.contains_key(&file.file_id);
            let url = file.download_url.clone();
            div()
              .flex()
              .flex_wrap()
              .items_center()
              .gap_3()
              .px_3()
              .py(px(10.))
              .when(ix + 1 < count, |el| el.border_b_1().border_color(border))
              .child(
                div()
                  .flex_1()
                  .min_w_0()
                  .truncate()
                  .text_sm()
                  .font_weight(FontWeight::MEDIUM)
                  .child(file.file_name.clone()),
              )
              .map(|el| {
                if found {
                  el.child(
                    div()
                      .flex()
                      .items_center()
                      .gap_1()
                      .h_5()
                      .px_2()
                      .rounded_full()
                      .bg(green.opacity(0.15))
                      .text_color(green)
                      .text_xs()
                      .child(Icon::new(IconName::Check).size(px(12.)))
                      .child("Found"),
                  )
                } else {
                  el.child(
                    div()
                      .flex()
                      .items_center()
                      .gap_1()
                      .text_xs()
                      .text_color(muted)
                      .child(Spinner::new().with_size(px(12.)))
                      .child("Waiting"),
                  )
                  .child(
                    Button::new(("download", ix))
                      .cursor_pointer()
                      .outline()
                      .small()
                      .icon(IconName::Download)
                      .label("Download")
                      .on_click(move |_, _, cx| cx.open_url(&url)),
                  )
                }
              })
          })),
      )
      .when_some(on_done, |el, on_done| {
        el.child(
          DialogFooter::new()
            .mt_2()
            .child(
              Button::new("cancel")
                .cursor_pointer()
                .outline()
                .label("Cancel")
                .on_click(|_, window, cx| window.close_dialog(cx)),
            )
            .child(
              Button::new("continue")
                .cursor_pointer()
                .primary()
                .label("Continue")
                .disabled(!done)
                .when(!done, |b| b.cursor_default())
                .on_click(move |_, window, cx| {
                  window.close_dialog(cx);
                  on_done(found_paths.clone(), window, cx);
                }),
            ),
        )
      })
  }
}

#[cfg(test)]
mod tests {
  use std::cell::RefCell;

  use gpui_kit::{self as gpui, AnyWindowHandle, TestAppContext, test::TestWindowExt};
  use proton_content::model::Source;

  use super::*;
  use crate::pages::profiles::tests::{init_fixture, open, poll};

  struct Empty;

  impl Render for Empty {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
      div().size_full()
    }
  }

  fn file(id: &str, bytes: &[u8]) -> ManualFile {
    ManualFile {
      source: Source::CurseForge,
      project_id: "p".into(),
      file_id: id.into(),
      file_name: format!("{id}.jar"),
      sha1: proton_utils::file::bytes_hash(bytes),
      size: bytes.len() as u64,
      download_url: "https://example.com".into(),
    }
  }

  /// The fixture with `dir` as the only download folder, and an empty window
  fn setup(cx: &mut TestAppContext, dir: PathBuf) -> AnyWindowHandle {
    init_fixture(cx);
    cx.update(|cx| proton_config::update(cx, |s| s.download_dirs = Some(vec![dir])));
    open(cx, |_, _| Empty).0
  }

  #[gpui::test]
  fn no_files_is_done_without_polling(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let handle = setup(cx, dir.path().into());
    cx.update_window(handle, |_, _, cx| {
      let view = cx.new(|cx| ManualDownloads::new(vec![], cx));
      assert!(view.read(cx).done());
      assert!(view.read(cx)._poll.is_none());
    })
    .unwrap();
  }

  #[gpui::test]
  fn continue_waits_for_every_file_then_hands_over_the_paths(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let handle = setup(cx, dir.path().into());
    // already there, under any name
    std::fs::write(dir.path().join("renamed.jar"), b"first").unwrap();
    // a browser download in progress hashes right but is skipped
    std::fs::write(dir.path().join("second.jar.part"), b"second").unwrap();
    let files = vec![file("a", b"first"), file("b", b"second")];
    let done: Rc<RefCell<Option<HashMap<String, PathBuf>>>> = Rc::default();
    let seen = done.clone();
    cx.update_window(handle, |_, w, cx| {
      ManualDownloads::open(
        files,
        move |found, _, _| *seen.borrow_mut() = Some(found),
        w,
        cx,
      )
    })
    .unwrap();
    poll(cx, |cx| {
      cx.windows()[0]
        .update(cx, |_, w, cx| {
          w.render_frame(cx);
          w.try_find(("download", 0usize)).is_none()
        })
        .unwrap()
    });
    cx.update_window(handle, |_, w, cx| {
      w.render_frame(cx);
      assert!(w.try_find(("download", 1usize)).is_some());
      // disabled while a file is missing
      w.click("continue", cx);
    })
    .unwrap();
    assert!(done.borrow().is_none());

    std::fs::write(dir.path().join("second.jar"), b"second").unwrap();
    for _ in 0..50 {
      cx.executor().advance_clock(POLL);
      std::thread::sleep(std::time::Duration::from_millis(20));
      cx.run_until_parked();
      let found = cx
        .update_window(handle, |_, w, cx| {
          w.render_frame(cx);
          w.try_find(("download", 1usize)).is_none()
        })
        .unwrap();
      if found {
        break;
      }
    }
    cx.update_window(handle, |_, w, cx| {
      w.render_frame(cx);
      w.click("continue", cx);
    })
    .unwrap();
    poll(cx, |_| done.borrow().is_some());
    let found = done.borrow().clone().unwrap();
    assert_eq!(found["a"], dir.path().join("renamed.jar"));
    assert_eq!(found["b"], dir.path().join("second.jar"));
  }
}
