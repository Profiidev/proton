//! Profile content tabs (mods, resource packs, shaders): `ContentList` + `ContentDropZone`

mod logic;
mod row;
pub mod target;
#[cfg(test)]
mod tests;

use std::{
  collections::{HashMap, HashSet},
  path::PathBuf,
};

use gpui_kit::{
  App, AppContext, Context, Entity, ExternalPaths, FontWeight, InteractiveElement, IntoElement,
  ListSizingBehavior, ParentElement, Render, SharedString, Styled, Subscription, Task,
  TestSupportExt, UniformListScrollHandle, Window,
  assets::IconName,
  component::{
    ActiveTheme, Disableable, Icon, IndexPath, Sizable,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    input::{Input, InputEvent, InputState},
    notification::Notification,
    radio::Radio,
    scroll::Scrollbar,
    select::{SearchableVec, Select, SelectEvent, SelectState},
    spinner::Spinner,
  },
  div,
  prelude::FluentBuilder,
  px, uniform_list,
};
use proton_components::widgets::content::{alert, radio_dot};
use proton_components::widgets::form::open_destroy_dialog;
use proton_components::widgets::form::{filled, icon_label};
use proton_components::widgets::tabs::segmented_tabs;
use proton_content::{
  ContentExt, InstalledContent,
  model::{ContentKind, Installed, Source},
};
use proton_profiles::ProfilesExt;

use self::{
  logic::*,
  row::{ContentRow, ROW_HEIGHT, RowData},
};
use crate::{
  actions::toast,
  router::{self, ProfileTab, Route, RouterExt},
  shell::{PageFactory, ShellDataExt},
};

pub fn register(cx: &mut App) {
  PageFactory::register(
    cx,
    |r| {
      matches!(
        r,
        Route::Profile {
          tab: ProfileTab::Content(_),
          ..
        }
      )
    },
    |route, window, cx| {
      let Route::Profile {
        id,
        tab: ProfileTab::Content(kind),
      } = route
      else {
        unreachable!()
      };
      let (id, kind) = (id.clone(), content_kind(*kind));
      cx.new(|cx| ContentPage::new(id, kind, window, cx)).into()
    },
  );
  feed_update_counts(cx);
}

/// The amber count on the profile's "Content" tab
fn feed_update_counts(cx: &mut App) {
  if !cx.has_global::<proton_content::Content>() {
    return;
  }
  let updates = cx.content().updates.clone();
  cx.observe(&updates, |updates, cx| {
    let counts: HashMap<String, usize> = updates
      .read(cx)
      .0
      .iter()
      .map(|(id, u)| (id.clone(), u.len()))
      .filter(|(_, n)| *n > 0)
      .collect();
    cx.shell_data().update(cx, |data, cx| {
      if data.content_updates != counts {
        data.content_updates = counts;
        cx.notify();
      }
    });
  })
  .detach();
}

/// Everything the page itself draws, the page notifies only when this changes
#[derive(Debug, Clone, Default, PartialEq)]
struct Summary {
  /// file names after search + status filter
  visible: Vec<String>,
  total: usize,
  updates: usize,
  selected: usize,
  selected_updates: bool,
  checking: bool,
  no_shader: bool,
}

pub struct ContentPage {
  profile: String,
  kind: ContentKind,
  installed: Entity<InstalledContent>,
  rows: Vec<Installed>,
  /// file name → newer version number
  updates: HashMap<String, String>,
  row_views: HashMap<String, Entity<ContentRow>>,
  selected: HashSet<String>,
  status: RowStatus,
  checking: bool,
  summary: Summary,
  search: Entity<InputState>,
  status_select: Entity<SelectState<SearchableVec<SharedString>>>,
  scroll: UniformListScrollHandle,
  _subs: [Subscription; 4],
}

impl ContentPage {
  pub fn new(
    profile: String,
    kind: ContentKind,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) -> Self {
    let installed = cx.content().clone().installed(&profile, cx);
    let updates = cx.content().updates.clone();
    let noun = word(kind);
    let search =
      cx.new(|cx| InputState::new(window, cx).placeholder(format!("Search installed {noun}s")));
    let labels = RowStatus::ALL.map(|(_, l)| SharedString::from(l)).to_vec();
    let status_select = cx.new(|cx| {
      SelectState::new(
        SearchableVec::new(labels),
        Some(IndexPath::default()),
        window,
        cx,
      )
      .searchable(false)
    });
    let subs = [
      cx.observe(&installed, |this, _, cx| this.sync(cx)),
      cx.observe(&updates, |this, _, cx| this.sync(cx)),
      cx.subscribe(&search, |this, _, e: &InputEvent, cx| {
        if matches!(e, InputEvent::Change) {
          this.sync(cx)
        }
      }),
      cx.subscribe(
        &status_select,
        |this, _, e: &SelectEvent<SearchableVec<SharedString>>, cx| {
          let SelectEvent::Confirm(Some(label)) = e else {
            return;
          };
          if let Some((s, _)) = RowStatus::ALL.iter().find(|(_, l)| *l == label.as_ref()) {
            this.status = *s;
            this.sync(cx);
          }
        },
      ),
    ];
    let mut this = Self {
      profile,
      kind,
      installed,
      rows: vec![],
      updates: HashMap::new(),
      row_views: HashMap::new(),
      selected: HashSet::new(),
      status: RowStatus::All,
      checking: false,
      summary: Summary::default(),
      search,
      status_select,
      scroll: UniformListScrollHandle::new(),
      _subs: subs,
    };
    this.sync(cx);
    this
  }

  /// Pushes the service state into the row entities (each notifies only on change) and notifies
  /// the page only when its own part changed
  fn sync(&mut self, cx: &mut Context<Self>) {
    let mut rows = self.installed.read(cx).get(self.kind).to_vec();
    sort_rows(self.kind, &mut rows);
    self.updates = cx
      .content()
      .updates
      .read(cx)
      .0
      .get(&self.profile)
      .into_iter()
      .flatten()
      .filter(|u| u.kind == self.kind)
      .map(|u| (u.file_name.clone(), u.latest.version_number.clone()))
      .collect();
    self
      .selected
      .retain(|f| rows.iter().any(|r| r.file_name == *f));

    let page = cx.weak_entity();
    let mut views = HashMap::with_capacity(rows.len());
    for entry in &rows {
      let file = entry.file_name.clone();
      let data = RowData {
        entry: entry.clone(),
        update: self.updates.get(&file).cloned(),
        selected: self.selected.contains(&file),
      };
      let view = match self.row_views.remove(&file) {
        Some(view) => {
          view.update(cx, |row, cx| row.set(data, cx));
          view
        }
        None => cx.new(|_| ContentRow::new(data, page.clone())),
      };
      views.insert(file, view);
    }
    self.row_views = views;

    let query = self.search.read(cx).value();
    let update_set: HashSet<String> = self.updates.keys().cloned().collect();
    let summary = Summary {
      visible: filter_rows(&rows, &query, self.status, &update_set)
        .into_iter()
        .map(|r| r.file_name.clone())
        .collect(),
      total: rows.len(),
      updates: self.updates.len(),
      selected: self.selected.len(),
      selected_updates: self.selected.iter().any(|f| self.updates.contains_key(f)),
      checking: self.checking,
      no_shader: !rows.iter().any(|r| r.active == Some(true)),
    };
    self.rows = rows;
    if summary != self.summary {
      self.summary = summary;
      cx.notify();
    }
  }

  fn entries(&self, files: &[String]) -> Vec<Installed> {
    self
      .rows
      .iter()
      .filter(|r| files.contains(&r.file_name))
      .cloned()
      .collect()
  }

  /// Toasts a failed command, then resyncs rows that were changed optimistically
  fn finish(&self, task: Task<anyhow::Result<()>>, cx: &mut Context<Self>) -> Task<bool> {
    cx.spawn(async move |this, cx| {
      let res = task.await;
      if let Err(e) = &res {
        let msg = error_message(e);
        let _ = this.update(cx, |this, cx| {
          toast(Notification::error(msg), cx);
          this.sync(cx);
        });
      }
      res.is_ok()
    })
  }

  pub fn toggle(&mut self, file: String, on: bool, _: &mut Window, cx: &mut Context<Self>) {
    self.set_enabled(vec![file], on, cx);
  }

  fn set_enabled(&mut self, files: Vec<String>, on: bool, cx: &mut Context<Self>) {
    // optimistic, the refresh after the command confirms it without another notify
    for file in &files {
      if let Some(view) = self.row_views.get(file) {
        view.update(cx, |row, cx| {
          let mut data = row.data.clone();
          data.entry.enabled = on;
          row.set(data, cx);
        });
      }
    }
    let (profile, kind) = (self.profile.clone(), self.kind);
    cx.spawn(async move |this, cx| {
      for file in files {
        let task = cx.update(|cx| {
          cx.content()
            .clone()
            .set_enabled(&profile, kind, file, on, cx)
        });
        let Ok(done) = this.update(cx, |this, cx| this.finish(task, cx)) else {
          return;
        };
        if !done.await {
          return;
        }
      }
    })
    .detach();
  }

  pub fn select(&mut self, file: String, on: bool, _: &mut Window, cx: &mut Context<Self>) {
    if on {
      self.selected.insert(file);
    } else {
      self.selected.remove(&file);
    }
    self.sync(cx);
  }

  fn select_all(&mut self, on: bool, cx: &mut Context<Self>) {
    self.selected = if on {
      self.summary.visible.iter().cloned().collect()
    } else {
      HashSet::new()
    };
    self.sync(cx);
  }

  pub fn set_shader(&mut self, file: Option<String>, _: &mut Window, cx: &mut Context<Self>) {
    let task = cx
      .content()
      .clone()
      .set_active_shader(&self.profile, file, cx);
    self.finish(task, cx).detach();
  }

  pub fn reorder(&mut self, from: String, to: String, _: &mut Window, cx: &mut Context<Self>) {
    let order: Vec<String> = self
      .rows
      .iter()
      .filter(|r| r.enabled)
      .map(|r| r.file_name.clone())
      .collect();
    let pos = |f: &String| order.iter().position(|o| o == f);
    if let (Some(from), Some(to)) = (pos(&from), pos(&to))
      && from != to
    {
      let files = move_item(&order, from, to);
      let task = cx.content().clone().set_order(&self.profile, files, cx);
      self.finish(task, cx).detach();
    }
  }

  pub fn ask_remove(&mut self, files: Vec<String>, window: &mut Window, cx: &mut Context<Self>) {
    let entries = self.entries(&files);
    let Some(text) = remove_warning(&entries, &self.rows) else {
      self.remove(files, cx);
      return;
    };
    let noun = word(self.kind);
    let title = if files.len() == 1 {
      format!("Remove {noun}")
    } else {
      format!("Remove {noun}s")
    };
    let page = cx.weak_entity();
    open_destroy_dialog(window, cx, title, text, "Remove", move |_, cx| {
      let files = files.clone();
      let _ = page.update(cx, |this, cx| this.remove(files, cx));
    });
  }

  fn remove(&mut self, files: Vec<String>, cx: &mut Context<Self>) {
    for f in &files {
      self.selected.remove(f);
    }
    let task = cx
      .content()
      .clone()
      .remove(&self.profile, self.kind, files, cx);
    self.finish(task, cx).detach();
    self.sync(cx);
  }

  /// `None` updates everything with an update. Progress shows in the downloads panel.
  pub fn update(&mut self, files: Option<Vec<String>>, cx: &mut Context<Self>) {
    let name = self.profile_name(cx);
    let task = cx
      .content()
      .clone()
      .update(&self.profile, files, format!("{name}: update"), cx);
    cx.spawn(async move |_, cx| {
      let note = match task.await {
        Ok(()) => Notification::success(format!("Updated content in {name}")),
        Err(_) => Notification::error(format!("Failed to update content in {name}")),
      };
      cx.update(|cx| toast(note, cx));
    })
    .detach();
  }

  fn check_updates(&mut self, cx: &mut Context<Self>) {
    let task = cx.content().check_updates(Some(self.profile.clone()));
    self.checking = true;
    self.sync(cx);
    cx.spawn(async move |this, cx| {
      let res = task.await;
      let _ = this.update(cx, |this, cx| {
        this.checking = false;
        if let Err(e) = res {
          toast(Notification::error(error_message(&e)), cx);
        }
        this.sync(cx);
      });
    })
    .detach();
  }

  fn add(&mut self, cx: &mut Context<Self>) {
    target::set(cx, self.profile.clone());
    cx.navigate(Route::Discover(route_kind(self.kind)));
  }

  pub fn open_project(&mut self, source: Source, id: String, cx: &mut Context<Self>) {
    let source = match source {
      Source::Modrinth => router::Source::Modrinth,
      Source::CurseForge => router::Source::CurseForge,
    };
    cx.navigate(Route::Project {
      id,
      kind: route_kind(self.kind),
      source,
    });
  }

  pub fn show_folder(&mut self, cx: &mut Context<Self>) {
    let task = cx.profiles().open_path(self.profile.clone());
    cx.spawn(async move |_, cx| {
      if let Err(e) = task.await {
        cx.update(|cx| crate::actions::toast_error("Failed to open the folder", &e, cx));
      }
    })
    .detach();
  }

  /// Dropped files: wrong extensions are reported, the rest is added one by one
  pub fn add_paths(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
    let (files, rejected): (Vec<_>, Vec<_>) =
      paths.into_iter().partition(|p| accepts(self.kind, p));
    if !rejected.is_empty() {
      let ext = extension(self.kind);
      toast(
        Notification::warning(format!("Only {ext} files can be added here")),
        cx,
      );
    }
    let profile = self.profile.clone();
    cx.spawn(async move |this, cx| {
      for path in files {
        let task = cx.update(|cx| cx.content().clone().add_file(&profile, path, cx));
        let Ok(done) = this.update(cx, |this, cx| this.finish(task, cx)) else {
          return;
        };
        done.await;
      }
    })
    .detach();
  }

  fn profile_name(&self, cx: &App) -> String {
    cx.profiles()
      .profiles
      .read(cx)
      .iter()
      .find(|p| p.id == self.profile)
      .map(|p| p.name.clone())
      .unwrap_or_default()
  }

  fn tabs(&self, cx: &App) -> impl IntoElement {
    let profile = self.profile.clone();
    let selected = LIST_KINDS.iter().position(|k| *k == self.kind).unwrap_or(0);
    div()
      .flex()
      .child(segmented_tabs(
        "content-tabs",
        ["Mods", "Resource packs", "Shaders"],
        selected,
        move |ix, _, cx| {
          cx.navigate(Route::Profile {
            id: profile.clone(),
            tab: ProfileTab::Content(route_kind(LIST_KINDS[*ix])),
          })
        },
        cx,
      ))
      .into_any_element()
  }

  fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
    let noun = word(self.kind);
    div()
      .flex()
      .items_center()
      .gap_2()
      .child(
        div().flex_1().min_w_0().child(
          Input::new(&self.search)
            .prefix(Icon::new(IconName::Search).text_color(cx.theme().muted_foreground))
            .aria_label(format!("Search installed {noun}s")),
        ),
      )
      .when(self.kind != ContentKind::Shader, |el| {
        el.child(
          div().w(px(144.)).flex_none().child(
            Select::new(&self.status_select)
              .cursor_pointer()
              .id("status")
              .accessibility_label("Status"),
          ),
        )
      })
      .child(
        Button::new("check-updates")
          .cursor_pointer()
          .outline()
          .map(|b| {
            if self.checking {
              // the kit draws its spinner in the icon slot
              b.icon(IconName::RefreshCw)
                .loading(true)
                .disabled(true)
                .cursor_default()
                .label("Check for updates")
            } else {
              icon_label(b, IconName::RefreshCw, "Check for updates")
            }
          })
          .on_click(cx.listener(|this, _, _, cx| this.check_updates(cx))),
      )
      .child(
        filled(icon_label(
          Button::new("add").cursor_pointer().primary(),
          IconName::Plus,
          format!("Add {noun}s"),
        ))
        .on_click(cx.listener(|this, _, _, cx| this.add(cx))),
      )
  }

  fn update_alert(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
    let n = self.summary.updates;
    if n == 0 {
      return None;
    }
    let (loader, version) = cx
      .profiles()
      .profiles
      .read(cx)
      .iter()
      .find(|p| p.id == self.profile)
      .map(|p| (format!("{:?}", p.loader), p.version.clone()))
      .unwrap_or_default();
    let title = format!("{} can be updated", plural(n, word(self.kind)));
    let text = format!("Newer versions compatible with {loader} {version} are available.");
    // `Alert.Action`: absolute top-2 right-2, the alert keeps pr-18 free
    Some(
      alert(Some(IconName::CircleArrowUp), title, text, None, cx)
        .relative()
        .pr(px(72.))
        .child(
          div().absolute().top_2().right_2().child(
            Button::new("update-all")
              .cursor_pointer()
              .primary()
              .small()
              .h_7()
              .label("Update all")
              .on_click(cx.listener(|this, _, _, cx| this.update(None, cx))),
          ),
        ),
    )
  }

  fn selection_bar(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
    let n = self.summary.selected;
    if n == 0 {
      return None;
    }
    let toggles = self.kind != ContentKind::Shader;
    Some(
      div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap_2()
        .child(
          div()
            .flex_1()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child(format!("{n} selected")),
        )
        .when(self.summary.selected_updates, |el| {
          el.child(
            Button::new("bulk-update")
              .cursor_pointer()
              .outline()
              .label("Update")
              .on_click(cx.listener(|this, _, _, cx| {
                let files = this
                  .selected
                  .iter()
                  .filter(|f| this.updates.contains_key(*f))
                  .cloned()
                  .collect();
                this.update(Some(files), cx)
              })),
          )
        })
        .when(toggles, |el| {
          el.child(
            Button::new("bulk-enable")
              .cursor_pointer()
              .outline()
              .label("Enable")
              .on_click(cx.listener(|this, _, _, cx| {
                let files = this.selected.iter().cloned().collect();
                this.set_enabled(files, true, cx)
              })),
          )
          .child(
            Button::new("bulk-disable")
              .cursor_pointer()
              .outline()
              .label("Disable")
              .on_click(cx.listener(|this, _, _, cx| {
                let files = this.selected.iter().cloned().collect();
                this.set_enabled(files, false, cx)
              })),
          )
        })
        .child(
          Button::new("bulk-remove")
            .cursor_pointer()
            .outline()
            .text_color(cx.theme().danger)
            .map(|b| icon_label(b, IconName::Trash, "Remove"))
            .on_click(cx.listener(|this, _, window, cx| {
              let files = this.selected.iter().cloned().collect();
              this.ask_remove(files, window, cx)
            })),
        ),
    )
  }

  fn list(&self, cx: &mut Context<Self>) -> impl IntoElement {
    let theme = cx.theme().clone();
    let noun = word(self.kind);
    let visible = &self.summary.visible;
    let all = !visible.is_empty() && visible.iter().all(|f| self.selected.contains(f));
    let header_text = |text: &'static str| {
      div()
        .text_xs()
        .line_height(px(16.))
        .text_color(theme.muted_foreground)
        .child(text)
    };
    let header = div()
      .h_9()
      .flex_none()
      .flex()
      .items_center()
      .gap_3()
      .px_3()
      // gpui clips rectangular, so backgrounds round themselves inside the 14px card
      .rounded_t(px(13.))
      .bg(theme.muted.opacity(0.4))
      .when(self.kind == ContentKind::ResourcePack, |el| {
        el.child(div().size_4().flex_none())
      })
      .child(
        Checkbox::new("select-all")
          .cursor_pointer()
          .checked(all)
          .accessibility_label(format!("Select all {noun}s"))
          .on_click(cx.listener(|this, on: &bool, _, cx| this.select_all(*on, cx))),
      )
      .child(
        div()
          .flex_1()
          .pl(px(52.))
          .text_xs()
          .line_height(px(16.))
          .text_color(theme.muted_foreground)
          .child(plural(visible.len(), noun)),
      )
      .child(header_text("Version").w(px(144.)).flex_none())
      .child(header_text("Source").w(px(104.)).flex_none())
      .child(
        header_text(if self.kind == ContentKind::Shader {
          "Active"
        } else {
          "Enabled"
        })
        .w_32()
        .flex_none()
        .text_right(),
      )
      .child(div().size_7().flex_none());

    let body = if !self.installed.read(cx).loaded {
      div()
        .py_6()
        .border_t_1()
        .border_color(theme.border)
        .flex()
        .justify_center()
        .child(Spinner::new())
        .into_any_element()
    } else if visible.is_empty() {
      let text = if self.summary.total == 0 {
        format!("No {noun}s installed yet")
      } else {
        format!("No {noun}s match the filters")
      };
      div()
        .px_3()
        .py_6()
        .border_t_1()
        .border_color(theme.border)
        .text_center()
        .text_sm()
        .text_color(theme.muted_foreground)
        .child(text)
        .into_any_element()
    } else {
      let row_style = gpui_kit::StyleRefinement::default()
        .w_full()
        .h(px(ROW_HEIGHT));
      let hover = theme.accent.opacity(0.4);
      let round_last = self.kind != ContentKind::Shader;
      div()
        .relative()
        .flex()
        .flex_col()
        .min_h_0()
        .flex_shrink(1.)
        .child(
          uniform_list(
            "content-rows",
            visible.len(),
            cx.processor(move |this, range: std::ops::Range<usize>, _, _| {
              let last = this.summary.visible.len() - 1;
              range
                .filter_map(|ix| Some((ix, this.row_views.get(this.summary.visible.get(ix)?)?)))
                .map(|(ix, view)| {
                  // hover lives out here so the last row can round it into the card corners
                  // ponytail: a hovered row cut off mid-scroll still pokes past the corners
                  div()
                    .id(ix)
                    .w_full()
                    .h(px(ROW_HEIGHT))
                    .hover(|s| s.bg(hover))
                    .when(round_last && ix == last, |el| el.rounded_b(px(13.)))
                    .child(view.clone().cached(row_style.clone()))
                    .into_any_element()
                })
                .collect()
            }),
          )
          .with_sizing_behavior(ListSizingBehavior::Infer)
          .track_scroll(&self.scroll)
          .w_full(),
        )
        .child(Scrollbar::new(&self.scroll))
        .into_any_element()
    };

    let none_row = (self.kind == ContentKind::Shader).then(|| {
      div()
        .id("shader-none")
        .h(px(ROW_HEIGHT))
        .flex_none()
        .flex()
        .items_center()
        .gap_3()
        .px_3()
        .border_t_1()
        .border_color(theme.border)
        .child(div().size_4().flex_none())
        .child(
          div()
            .size_10()
            .flex_none()
            .rounded(px(8.))
            .border_1()
            .border_dashed()
            .border_color(theme.border),
        )
        .child(
          div()
            .flex_1()
            .font_weight(FontWeight::MEDIUM)
            .child("None (shaders off)"),
        )
        .child(
          div()
            .w_32()
            .flex_none()
            .flex()
            .justify_end()
            .child(radio_dot(
              Radio::new("toggle")
                .cursor_pointer()
                .checked(self.summary.no_shader)
                .accessibility_label("Turn shaders off")
                .on_click(cx.listener(|this, _, window, cx| this.set_shader(None, window, cx))),
              self.summary.no_shader,
              cx,
            )),
        )
        .child(div().size_7().flex_none())
    });

    div()
      .flex()
      .flex_col()
      .min_h_0()
      .flex_shrink(1.)
      .overflow_hidden()
      .rounded(px(14.))
      .border_1()
      .border_color(theme.border)
      .bg(theme.popover.opacity(0.5))
      .child(header)
      .child(body)
      .children(none_row)
  }

  fn drop_zone(&self, cx: &mut Context<Self>) -> impl IntoElement {
    let theme = cx.theme();
    let primary = theme.primary;
    let fg = theme.foreground;
    div()
      .id("drop-zone")
      .h_16()
      .w_full()
      .flex_none()
      .flex()
      .items_center()
      .justify_center()
      .gap_2()
      .rounded(px(14.))
      .border_1()
      .border_dashed()
      .border_color(theme.border)
      .text_sm()
      .text_color(theme.muted_foreground)
      .group_drag_over::<ExternalPaths>("content-page", move |s| {
        s.border_color(primary)
          .bg(primary.opacity(0.1))
          .text_color(fg)
      })
      .child(Icon::new(IconName::Upload).size_5())
      .child(format!(
        "Drop {} files here to add a {} manually",
        extension(self.kind),
        word(self.kind)
      ))
      .test_support()
  }
}

impl Render for ContentPage {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    div()
      .id("content-page")
      .group("content-page")
      .size_full()
      .flex()
      .flex_col()
      .gap_3()
      .on_drop(
        cx.listener(|this, paths: &ExternalPaths, _, cx| {
          this.add_paths(paths.paths().to_vec(), cx)
        }),
      )
      .child(self.tabs(cx))
      .child(self.toolbar(cx))
      .children(self.update_alert(cx))
      .children(self.selection_bar(cx))
      .child(self.list(cx))
      .child(self.drop_zone(cx))
  }
}
