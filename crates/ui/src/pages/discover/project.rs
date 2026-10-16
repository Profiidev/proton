//! `/discover/project`: header, description / versions / gallery tabs, compatibility aside

use std::rc::Rc;

use chrono::{DateTime, Utc};
use gpui_kit::{
  AnyElement, App, AppContext, Context, Entity, FontWeight, ImageSource, InteractiveElement,
  IntoElement, ObjectFit, ParentElement, Render, SharedString, StatefulInteractiveElement, Styled,
  StyledImage, Subscription, Task, TestSupportExt, Window,
  assets::IconName,
  component::{
    ActiveTheme, Disableable, Icon, Sizable,
    button::{Button, ButtonVariants},
    scroll::ScrollableElement,
    separator::Separator,
    switch::Switch,
    tab::{Tab, TabBar},
    text::TextView,
  },
  div, img,
  prelude::FluentBuilder,
  px,
};
use proton_components::widgets::{
  chip, colors,
  content::ProjectIcon,
  format::relative_time,
};
use proton_content::{
  ContentExt, InstalledContent,
  model::{ContentKind, DependencyKind, Installed, ProjectDetail, Source, VersionInfo},
};
use proton_profiles::{ProfilesExt, config::Profile};
use proton_versions::{VersionsExt, loader::LoaderType};

use crate::{
  content::{
    InstallDialog, ModpackInstallDialog, ProjectRef, TargetPicker, logic, target, target_picker,
  },
  router::{Kind, Route, RouterExt},
  shell::ShellDataExt,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum TabKind {
  Description,
  Versions,
  Gallery,
}

pub struct ProjectPage {
  id: String,
  kind: ContentKind,
  source: Source,
  detail: Option<Rc<ProjectDetail>>,
  failed: bool,
  tab: TabKind,
  show_all: bool,
  /// Versions for the target, None while loading
  compat_versions: Option<Vec<VersionInfo>>,
  /// All versions, loaded when asked for (and for packs)
  all_versions: Option<Vec<VersionInfo>>,
  /// Required dependencies of the newest compatible version: (id, name)
  needs: Vec<(String, String)>,
  /// Vanilla versions newest first, to sort game versions
  known: Vec<String>,
  picker: Option<Entity<TargetPicker>>,
  versions_for: Option<String>,
  /// Observed installed content per profile id
  watched: std::collections::HashMap<String, Subscription>,
  _tasks: Vec<Task<()>>,
  _versions: Option<Task<()>>,
  _subs: Vec<Subscription>,
}

impl ProjectPage {
  pub fn new(id: String, kind: Kind, source: crate::router::Source, window: &mut Window, cx: &mut Context<Self>) -> Self {
    let kind = logic::content_kind(kind);
    let source = logic::content_source(source);
    let picker = (kind != ContentKind::Modpack).then(|| {
      // nothing is compatible until the project loaded
      let filter: target_picker::ProfileFilter = Rc::new(|_, _| false);
      cx.new(|cx| TargetPicker::new(filter, window, cx))
    });
    let profiles = cx.profiles().profiles.clone();
    let target = target(cx);
    let updates = cx.content().updates.clone();
    let subs = vec![
      cx.observe(&profiles, |this, _, cx| this.load_versions(cx)),
      cx.observe(&target, |this, _, cx| this.load_versions(cx)),
      cx.observe(&updates, |_, _, cx| cx.notify()),
    ];

    let mut tasks = vec![];
    let load = cx.content().project(source, id.clone());
    tasks.push(cx.spawn(async move |this, cx| {
      let res = load.await;
      let _ = this.update(cx, |this, cx| match res {
        Ok(detail) => this.set_detail(detail, cx),
        Err(_) => {
          this.failed = true;
          cx.notify();
        }
      });
    }));
    let known = cx.versions().clone().version_list(LoaderType::Vanilla, cx);
    tasks.push(cx.spawn(async move |this, cx| {
      if let Ok(list) = known.await {
        let _ = this.update(cx, |this, cx| {
          this.known = list;
          cx.notify();
        });
      }
    }));
    let mut this = Self {
      id,
      kind,
      source,
      detail: None,
      failed: false,
      tab: TabKind::Description,
      show_all: false,
      compat_versions: None,
      all_versions: None,
      needs: vec![],
      known: vec![],
      picker,
      versions_for: None,
      watched: Default::default(),
      _tasks: tasks,
      _versions: None,
      _subs: subs,
    };
    if kind == ContentKind::Modpack {
      this.load_all(cx);
    }
    this
  }

  /// Shows a loaded project (also used by tests)
  pub fn set_detail(&mut self, detail: ProjectDetail, cx: &mut Context<Self>) {
    let name = detail.summary.name.clone();
    let id = self.id.clone();
    cx.shell_data().update(cx, |d, cx| {
      if d.names.projects.get(&id) != Some(&name) {
        d.names.projects.insert(id, name);
        cx.notify();
      }
    });
    let summary = detail.summary.clone();
    if let Some(picker) = &self.picker {
      let filter: target_picker::ProfileFilter =
        Rc::new(move |p, _| logic::summary_compat(p, &summary).ok);
      picker.update(cx, |p, cx| p.set_filter(filter, cx));
    }
    self.detail = Some(Rc::new(detail));
    self.load_versions(cx);
    cx.notify();
  }

  fn filter(&self) -> Option<target_picker::ProfileFilter> {
    let summary = self.detail.as_ref()?.summary.clone();
    Some(Rc::new(move |p, _| logic::summary_compat(p, &summary).ok))
  }

  fn target(&self, cx: &mut App) -> Option<Profile> {
    if self.kind == ContentKind::Modpack {
      return None;
    }
    target_picker::candidates(&self.filter()?, cx).1
  }

  /// Versions for the current target, reloaded when it changes
  fn load_versions(&mut self, cx: &mut Context<Self>) {
    let target = self.target(cx).map(|p| p.id);
    if target == self.versions_for {
      cx.notify();
      return;
    }
    self.versions_for = target.clone();
    self.compat_versions = None;
    self.needs.clear();
    let Some(profile) = target else {
      cx.notify();
      return;
    };
    let load = cx
      .content()
      .versions(self.source, self.id.clone(), Some(profile));
    let content = cx.content().clone();
    let source = self.source;
    self._versions = Some(cx.spawn(async move |this, cx| {
      let versions = load.await.unwrap_or_default();
      let required: Vec<String> = versions
        .first()
        .map(|v| {
          v.dependencies
            .iter()
            .filter(|d| d.kind == DependencyKind::Required)
            .map(|d| d.project_id.clone())
            .collect()
        })
        .unwrap_or_default();
      let _ = this.update(cx, |this, cx| {
        this.compat_versions = Some(versions);
        cx.notify();
      });
      let mut needs = vec![];
      for id in required {
        let name = content
          .project(source, id.clone())
          .await
          .map(|d| d.summary.name)
          .unwrap_or_else(|_| id.clone());
        needs.push((id, name));
      }
      let _ = this.update(cx, |this, cx| {
        this.needs = needs;
        cx.notify();
      });
    }));
    cx.notify();
  }

  fn load_all(&mut self, cx: &mut Context<Self>) {
    if self.all_versions.is_some() {
      return;
    }
    let load = cx.content().versions(self.source, self.id.clone(), None);
    self._tasks.push(cx.spawn(async move |this, cx| {
      let versions = load.await.unwrap_or_default();
      let _ = this.update(cx, |this, cx| {
        this.all_versions = Some(versions);
        cx.notify();
      });
    }));
  }

  /// Installed content of a profile, re-renders when it changes
  fn installed(&mut self, profile: &str, cx: &mut Context<Self>) -> Entity<InstalledContent> {
    let entity = cx.content().clone().installed(profile, cx);
    if !self.watched.contains_key(profile) {
      let sub = cx.observe(&entity, |_, _, cx| cx.notify());
      self.watched.insert(profile.to_string(), sub);
    }
    entity
  }

  fn install(&self, version: Option<String>, window: &mut Window, cx: &mut App) {
    let Some(detail) = &self.detail else {
      return;
    };
    let project = ProjectRef::from(&detail.summary);
    if self.kind == ContentKind::Modpack {
      ModpackInstallDialog::open(project, window, cx);
    } else {
      let target = self.target(cx).map(|p| p.id);
      InstallDialog::open_version(project, target, version, window, cx);
    }
  }
}

/// The project description, http(s) links open in the browser
pub fn description_view(body: &str) -> TextView {
  TextView::html("description-text", body.to_string())
    .selectable(true)
    .on_link_click(|url, _, _, cx| {
      if url.starts_with("http") {
        cx.open_url(url);
      }
    })
}

fn kv(key: impl Into<SharedString>, value: impl Into<SharedString>, cx: &App) -> impl IntoElement {
  div()
    .flex()
    .items_center()
    .justify_between()
    .gap_3()
    .child(
      div()
        .flex_none()
        .text_color(cx.theme().muted_foreground)
        .child(key.into()),
    )
    .child(div().truncate().text_right().child(value.into()))
}

fn card(cx: &App) -> gpui_kit::Div {
  let theme = cx.theme();
  div()
    .flex()
    .flex_col()
    .gap_3()
    .p_4()
    .rounded_xl()
    .border_1()
    .border_color(theme.foreground.opacity(0.1))
    .bg(theme.popover)
    .text_sm()
}

fn green_chip(text: &'static str, cx: &App) -> impl IntoElement {
  let green = colors::green_500();
  chip(text, cx).bg(green.opacity(0.15)).text_color(green)
}

fn empty(title: &str, text: String, cx: &App) -> AnyElement {
  div()
    .flex()
    .flex_col()
    .items_center()
    .gap_1()
    .p_6()
    .rounded_xl()
    .border_1()
    .border_dashed()
    .border_color(cx.theme().border)
    .child(div().font_weight(FontWeight::MEDIUM).child(title.to_string()))
    .child(
      div()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(text),
    )
    .into_any_element()
}

impl ProjectPage {
  fn versions_tab(
    &self,
    target: Option<&Profile>,
    entry: Option<&Installed>,
    cx: &mut Context<Self>,
  ) -> AnyElement {
    let is_pack = self.kind == ContentKind::Modpack;
    let filtered = !is_pack && !self.show_all;
    let versions = if filtered {
      self.compat_versions.as_ref()
    } else {
      self.all_versions.as_ref()
    };
    let theme = cx.theme();
    let muted = theme.muted_foreground;
    let border = theme.border;
    let now = Utc::now();
    let body = match versions {
      None => div().into_any_element(),
      Some(list) if list.is_empty() => empty(
        "No versions",
        match (filtered, target) {
          (true, Some(t)) => format!(
            "None made for {} {}. Show all versions to see the rest.",
            logic::loader_label(t.loader),
            t.version
          ),
          _ => "This project has no files to install.".into(),
        },
        cx,
      ),
      Some(list) => {
        let col = |w: f32| div().w(px(w)).flex_none().truncate();
        let head = div()
          .flex()
          .items_center()
          .gap_3()
          .h_10()
          .px_3()
          .border_b_1()
          .border_color(border)
          .text_color(muted)
          .font_weight(FontWeight::MEDIUM)
          .child(div().flex_1().min_w_0().child("Version"))
          .child(col(70.).child("Channel"))
          .child(col(150.).child("Game version"))
          .child(col(110.).child("Loaders"))
          .child(col(100.).child("Published"))
          .child(col(80.).text_right().child("Action"));
        let rows = list.iter().enumerate().map(|(ix, v)| {
          let installed = entry.is_some_and(|e| e.version_id.as_deref() == Some(&v.id));
          let id = v.id.clone();
          let channel = serde_json::to_value(v.channel)
            .ok()
            .and_then(|c| c.as_str().map(logic::tag_label))
            .unwrap_or_default();
          let loaders: Vec<String> = v.loaders.iter().map(|l| logic::tag_label(l)).collect();
          let published = v
            .published_at
            .parse::<DateTime<Utc>>()
            .map(|t| relative_time(t, now))
            .unwrap_or_default();
          div()
            .flex()
            .items_center()
            .gap_3()
            .min_h_10()
            .px_3()
            .py_1()
            .when(ix + 1 < list.len(), |el| el.border_b_1().border_color(border))
            .child(
              div()
                .flex_1()
                .min_w_0()
                .truncate()
                .font_weight(FontWeight::MEDIUM)
                .child(v.version_number.clone()),
            )
            .child(col(70.).child(channel))
            .child(col(150.).child(logic::short_list(
              &logic::sort_versions(&v.game_versions, &self.known),
              2,
            )))
            .child(col(110.).child(loaders.join(", ")))
            .child(col(100.).child(published))
            .child(div().w(px(80.)).flex().justify_end().child(if installed {
              green_chip("Installed", cx).into_any_element()
            } else {
              Button::new(("install-version", ix))
                .outline()
                .small()
                .label("Install")
                .on_click(cx.listener(move |this, _, window, cx| {
                  this.install(Some(id.clone()), window, cx)
                }))
                .into_any_element()
            }))
        });
        div()
          .rounded_xl()
          .border_1()
          .border_color(theme.foreground.opacity(0.1))
          .bg(theme.popover)
          .overflow_hidden()
          .text_sm()
          .child(head)
          .children(rows)
          .into_any_element()
      }
    };
    div()
      .flex()
      .flex_col()
      .gap_3()
      .pt_2()
      .when_some(target.filter(|_| !is_pack), |el, t| {
        el.child(
          Switch::new("show-all")
            .checked(self.show_all)
            .label(format!(
              "Show all versions, not only those for {}",
              t.name
            ))
            .on_click(cx.listener(|this, on: &bool, _, cx| {
              this.show_all = *on;
              if *on {
                this.load_all(cx);
              }
              cx.notify();
            })),
        )
      })
      .child(body)
      .into_any_element()
  }

  fn gallery_tab(&self, detail: &ProjectDetail, cx: &App) -> AnyElement {
    if detail.gallery.is_empty() {
      return div()
        .pt_3()
        .child(empty(
          "No screenshots",
          "This project has no gallery images.".into(),
          cx,
        ))
        .into_any_element();
    }
    let muted = cx.theme().muted_foreground;
    let bg = cx.theme().muted;
    div()
      .pt_3()
      .flex()
      .flex_wrap()
      .gap_2()
      .children(detail.gallery.iter().map(|image| {
        div()
          .flex()
          .flex_col()
          .gap_1()
          .w(px(240.))
          .flex_grow(1.)
          .child(
            img(ImageSource::from(image.url.clone()))
              .w_full()
              .h(px(135.))
              .rounded_lg()
              .bg(bg)
              .object_fit(ObjectFit::Cover),
          )
          .when_some(image.title.clone(), |el, t| {
            el.child(div().truncate().text_xs().text_color(muted).child(t))
          })
      }))
      .into_any_element()
  }

  fn aside(
    &mut self,
    detail: &ProjectDetail,
    target: Option<&Profile>,
    cx: &mut Context<Self>,
  ) -> impl IntoElement {
    let summary = &detail.summary;
    let url = summary.url.clone();
    let source = summary.source;
    // mods of the target, for the "Installed" chips of the dependencies
    let installed_mods: Vec<String> = target
      .map(|t| {
        let e = self.installed(&t.id, cx);
        e.read(cx)
          .get(ContentKind::Mod)
          .iter()
          .filter_map(|i| i.project_id.clone())
          .collect()
      })
      .unwrap_or_default();

    let muted = cx.theme().muted_foreground;
    let mut compat = card(cx).child(div().font_weight(FontWeight::MEDIUM).child("Compatibility"));
    if !summary.game_versions.is_empty() {
      compat = compat.child(kv(
        "Game version",
        logic::short_list(&logic::sort_versions(&summary.game_versions, &self.known), 3),
        cx,
      ));
    }
    if !summary.loaders.is_empty() {
      let loaders: Vec<String> = summary.loaders.iter().map(|l| logic::tag_label(l)).collect();
      compat = compat.child(kv("Loaders", loaders.join(", "), cx));
    }
    if let Some(runs) = logic::runs_on(summary.client_side.as_deref(), summary.server_side.as_deref())
    {
      compat = compat.child(kv("Runs on", runs, cx));
    }
    compat = compat.child(
      div()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .child(div().text_color(muted).child("Source"))
        .child(
          div()
            .id("source-link")
            .flex()
            .items_center()
            .gap_1()
            .cursor_pointer()
            .hover(|s| s.underline())
            .child(logic::source_label(summary.source))
            .child(Icon::new(IconName::ExternalLink).size(px(14.)))
            .on_click(move |_, _, cx| cx.open_url(&url)),
        ),
    );
    if !self.needs.is_empty() {
      compat = compat
        .child(Separator::horizontal())
        .child(div().font_weight(FontWeight::MEDIUM).child("Needs"))
        .children(self.needs.iter().enumerate().map(|(ix, (id, name))| {
          let route = Route::Project {
            id: id.clone(),
            kind: Kind::Mod,
            source: logic::route_source(source),
          };
          div()
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .child(
              div()
                .id(("need", ix))
                .truncate()
                .cursor_pointer()
                .hover(|s| s.underline())
                .child(name.clone())
                .on_click(move |_, _, cx| cx.navigate(route.clone())),
            )
            .when(installed_mods.contains(id), |el| {
              el.child(green_chip("Installed", cx))
            })
        }));
    }

    // other profiles with this project
    let others: Vec<(String, String)> = if self.kind == ContentKind::Modpack {
      vec![]
    } else {
      let profiles: Vec<Profile> = cx
        .profiles()
        .profiles
        .read(cx)
        .iter()
        .filter(|p| Some(&p.id) != target.map(|t| &t.id))
        .cloned()
        .collect();
      profiles
        .into_iter()
        .filter_map(|p| {
          let e = self.installed(&p.id, cx);
          let found = e
            .read(cx)
            .get(self.kind)
            .iter()
            .find(|i| i.project_id.as_deref() == Some(&self.id))
            .map(|i| i.version_number.clone().unwrap_or_default());
          found.map(|v| (p.name, v))
        })
        .collect()
    };

    div()
      .id("project-aside")
      .flex()
      .flex_col()
      .flex_none()
      .w(px(260.))
      .max_h_full()
      .gap_3()
      .overflow_y_scroll()
      .child(compat)
      .when(!others.is_empty(), |el| {
        el.child(
          card(cx)
            .child(
              div()
                .font_weight(FontWeight::MEDIUM)
                .child("Also installed from this page"),
            )
            .children(others.into_iter().map(|(name, v)| kv(name, v, cx))),
        )
      })
  }
}

impl Render for ProjectPage {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let root = div().size_full().flex().flex_col().gap_4().px_5().pt_4().pb_6();
    let Some(detail) = self.detail.clone() else {
      return root.when(self.failed, |el| {
        el.child(empty(
          "Project not found",
          "It may have been removed, or the source is not reachable right now.".into(),
          cx,
        ))
      });
    };
    let summary = &detail.summary;
    let is_pack = self.kind == ContentKind::Modpack;
    let target = self.target(cx);
    let entry: Option<Installed> = target.as_ref().and_then(|t| {
      let e = self.installed(&t.id, cx);
      e.read(cx)
        .get(self.kind)
        .iter()
        .find(|i| i.project_id.as_deref() == Some(&self.id))
        .cloned()
    });
    let update = target.as_ref().and_then(|t| {
      cx.content()
        .updates
        .read(cx)
        .0
        .get(&t.id)
        .and_then(|u| u.iter().find(|u| u.project_id == self.id).cloned())
    });
    let theme = cx.theme();
    let muted = theme.muted_foreground;
    let amber = colors::amber_500();
    let tags = logic::dedup(
      summary
        .categories
        .iter()
        .chain(&summary.loaders)
        .map(|t| logic::tag_label(t)),
    );

    let button = match (&update, &entry) {
      (Some(u), _) => {
        let id = u.latest.id.clone();
        Button::new("install")
          .primary()
          .label(format!("Update to {}", u.latest.version_number))
          .on_click(cx.listener(move |this, _, window, cx| {
            this.install(Some(id.clone()), window, cx)
          }))
      }
      (None, Some(_)) => Button::new("install")
        .secondary()
        .disabled(true)
        .icon(IconName::Check)
        .label("Installed"),
      (None, None) => Button::new("install")
        .primary()
        .label("Install")
        .disabled(!is_pack && target.is_none())
        .on_click(cx.listener(|this, _, window, cx| this.install(None, window, cx))),
    };

    let header = div()
      .flex()
      .flex_none()
      .flex_wrap()
      .items_center()
      .gap_4()
      .child(
        ProjectIcon::new(summary.icon_url.clone().map(ImageSource::from))
          .size(px(88.))
          .radius(px(12.)),
      )
      .child(
        div()
          .flex()
          .flex_col()
          .flex_1()
          .min_w(px(320.))
          .gap(px(6.))
          .child(
            div()
              .flex()
              .items_center()
              .gap_2()
              .child(
                div()
                  .truncate()
                  .text_xl()
                  .font_weight(FontWeight::SEMIBOLD)
                  .child(summary.name.clone()),
              )
              .when(update.is_some(), |el| {
                el.child(
                  chip("Update available", cx)
                    .bg(amber.opacity(0.15))
                    .text_color(amber),
                )
              }),
          )
          .child(
            div()
              .max_w(px(560.))
              .text_sm()
              .text_color(muted)
              .child(summary.description.clone()),
          )
          .child(
            div()
              .flex()
              .flex_wrap()
              .gap(px(6.))
              .children(tags.into_iter().map(|t| chip(t, cx))),
          ),
      )
      .child(
        div()
          .flex()
          .flex_col()
          .w(px(220.))
          .gap_2()
          .children(self.picker.clone())
          .child(button.w_full())
          .when_some(entry.as_ref().zip(target.as_ref()), |el, (e, t)| {
            el.child(
              div()
                .text_center()
                .text_xs()
                .text_color(muted)
                .child(format!(
                  "Installed in {}: {}",
                  t.name,
                  e.version_number.clone().unwrap_or(e.file_name.clone())
                )),
            )
          }),
      );

    let tab_ix = match self.tab {
      TabKind::Description => 0,
      TabKind::Versions => 1,
      TabKind::Gallery => 2,
    };
    let content = match self.tab {
      TabKind::Description => div()
        .id("description")
        .pt_3()
        .max_w(px(640.))
        .child(description_view(&detail.body))
        .test_support()
        .into_any_element(),
      TabKind::Versions => self.versions_tab(target.as_ref(), entry.as_ref(), cx),
      TabKind::Gallery => self.gallery_tab(&detail, cx),
    };

    root.child(header).child(
      div()
        .flex()
        .flex_1()
        .min_h_0()
        .items_start()
        .gap_5()
        .child(
          div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .h_full()
            .child(
              div().flex_none().child(
                TabBar::new("project-tabs")
                  .segmented()
                  .selected_index(tab_ix)
                  .child(Tab::new().label("Description"))
                  .child(Tab::new().label("Versions"))
                  .child(Tab::new().label("Gallery"))
                  .on_click(cx.listener(|this, ix: &usize, _, cx| {
                    this.tab = match ix {
                      1 => TabKind::Versions,
                      2 => TabKind::Gallery,
                      _ => TabKind::Description,
                    };
                    cx.notify();
                  })),
              ),
            )
            .child(
              div()
                .id("tab-content")
                .flex_1()
                .min_h_0()
                .overflow_y_scrollbar()
                .child(content),
            ),
        )
        .child(self.aside(&detail, target.as_ref(), cx)),
    )
  }
}

#[cfg(test)]
pub(crate) mod tests {
  use std::{path::PathBuf, sync::LazyLock};

  use gpui_kit::{self as gpui, TestAppContext, test::TestWindowExt};
  use proton_content::model::{GalleryImage, ProjectLinks};

  use super::*;

  static DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    let dir = tempfile::tempdir().unwrap().keep();
    proton_store::init_at(dir.join("store.json"));
    dir
  });

  /// Theme, shell, config, versions (no manifests), profiles in a temp dir, content
  pub fn init_services(cx: &mut TestAppContext) {
    LazyLock::force(&DIR);
    cx.update(|cx| {
      gpui_kit::init(cx);
      proton_components::assets::load(cx).unwrap();
      crate::init(cx);
      proton_config::init(cx);
      let (handle, _) = proton_versions::event::DownloadsHandle::new();
      let versions = proton_versions::Versions {
        versions: cx.new(|_| Default::default()),
        downloads: cx.new(|_| Default::default()),
        handle,
      };
      cx.set_global(versions);
      proton_profiles::init_at(cx, tempfile::tempdir().unwrap().keep()).unwrap();
      proton_content::init(cx);
    });
  }

  fn detail() -> ProjectDetail {
    let mut summary = crate::content::logic::tests::summary("AANobbMI");
    summary.name = "Sodium".into();
    ProjectDetail {
      summary,
      body: proton_content::render::markdown(
        "# Sodium\n\nA **fast** renderer, see [the wiki](https://example.com).",
      ),
      gallery: vec![GalleryImage {
        url: "https://example.com/a.png".into(),
        title: Some("Shot".into()),
      }],
      links: ProjectLinks::default(),
    }
  }

  #[gpui::test]
  fn renders_the_markdown_description(cx: &mut TestAppContext) {
    init_services(cx);
    let (page, cx) = cx.add_window_view(|window, cx| {
      ProjectPage::new(
        "AANobbMI".into(),
        Kind::Mod,
        crate::router::Source::Modrinth,
        window,
        cx,
      )
    });
    page.update(cx, |p, cx| p.set_detail(detail(), cx));
    cx.run_until_parked();
    cx.update(|window, cx| {
      window.render_frame(cx);
      assert!(window.try_find("description").is_some());
      // the breadcrumb learned the title
      assert_eq!(
        cx.shell_data().read(cx).names.projects["AANobbMI"],
        "Sodium"
      );
    });
    page.update(cx, |p, cx| {
      p.tab = TabKind::Gallery;
      cx.notify();
    });
    cx.update(|window, cx| window.render_frame(cx));
  }
}
