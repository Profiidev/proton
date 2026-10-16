//! Install one project into a profile (`InstallDialog.svelte`)

use gpui_kit::{
  AnyElement, App, AppContext, Context, Entity, FontWeight, InteractiveElement, IntoElement,
  ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Subscription, Task,
  TestSupportExt, Window,
  assets::IconName,
  component::{
    ActiveTheme, Disableable, Icon, Sizable, WindowExt,
    alert::Alert,
    button::{Button, ButtonVariants},
    dialog::{DialogDescription, DialogFooter},
    input::{Input, InputEvent, InputState},
    notification::Notification,
    radio::Radio,
    spinner::Spinner,
  },
  div,
  prelude::FluentBuilder,
  px,
};
use proton_components::widgets::{chip, colors, form::VersionSelect, input_tile, profile::ProfileIcon};
use proton_content::{
  ContentError, ContentExt,
  model::{InstallPlan, PickRef, PlanReason, VersionInfo},
};
use proton_profiles::{ProfilesExt, config::Profile};

use super::{
  ManualDownloads, ProjectRef, content_error, logic, manual::MANUAL_HINT, profile_icon,
  profile_icons, set_target, target,
};
use crate::actions::{toast, toast_error};

enum PlanState {
  Idle,
  Loading,
  Failed { incompatible: bool },
  Ready(InstallPlan),
}

/// Rows of the "Also needed" box: (name, chip text, already installed)
pub fn plan_rows(plan: &InstallPlan) -> Vec<(String, String, bool)> {
  plan
    .already_installed
    .iter()
    .map(|d| (d.name.clone(), "Already installed".to_string(), true))
    .chain(
      plan
        .items
        .iter()
        .filter(|i| i.reason == PlanReason::Dependency)
        .map(|i| {
          (
            i.name.clone(),
            format!("Will install {}", i.version.version_number),
            false,
          )
        }),
    )
    .collect()
}

/// The plan part of the dialog: needed dependencies, optional ones, conflicts
pub fn plan_view(plan: &InstallPlan, cx: &App) -> AnyElement {
  let theme = cx.theme();
  let rows = plan_rows(plan);
  let green = colors::green_500();
  div()
    .flex()
    .flex_col()
    .gap(px(18.))
    .when(!rows.is_empty(), |el| {
      el.child(
        div()
          .id("plan-needed")
          .flex()
          .flex_col()
          .gap_2()
          .px(px(14.))
          .py_3()
          .rounded_xl()
          .border_1()
          .border_color(theme.foreground.opacity(0.1))
          .bg(theme.muted.opacity(0.4))
          .child(
            div()
              .text_xs()
              .font_weight(FontWeight::MEDIUM)
              .text_color(theme.muted_foreground)
              .child("ALSO NEEDED"),
          )
          .children(rows.into_iter().map(|(name, label, installed)| {
            div()
              .flex()
              .items_center()
              .justify_between()
              .gap_2()
              .text_sm()
              .child(div().truncate().child(name))
              .child(if installed {
                chip(label, cx).bg(green.opacity(0.15)).text_color(green)
              } else {
                chip(label, cx)
              })
          }))
          .test_support(),
      )
    })
    .when(!plan.optional.is_empty(), |el| {
      let names: Vec<_> = plan.optional.iter().map(|o| o.name.as_str()).collect();
      el.child(
        div()
          .text_xs()
          .text_color(theme.muted_foreground)
          .child(format!("Optional: {}", names.join(", "))),
      )
    })
    .when(!plan.conflicts.is_empty(), |el| {
      let lines: Vec<String> = plan
        .conflicts
        .iter()
        .map(|c| format!("{}: {}", c.name, c.reason))
        .collect();
      el.child(
        Alert::error("plan-conflicts", lines.join("\n"))
          .title("Conflicts with installed content")
          .icon(IconName::CircleAlert),
      )
    })
    .into_any_element()
}

/// Profile tiles, version select, dependency plan, "Install to {profile}"
pub struct InstallDialog {
  project: ProjectRef,
  /// Preselected version (versions table)
  version_id: Option<String>,
  search: Entity<InputState>,
  /// Picked by the user
  chosen: Option<String>,
  /// Profile the versions/plan were loaded for
  loaded_for: Option<String>,
  versions: Vec<VersionInfo>,
  version: Option<VersionInfo>,
  select: Entity<VersionSelect>,
  plan: PlanState,
  manual: Option<Entity<ManualDownloads>>,
  _load: Option<Task<()>>,
  _subs: Vec<Subscription>,
}

impl InstallDialog {
  /// Opens the dialog. `target` preselects a profile (else the remembered target, else the first
  /// compatible one). Use [`InstallDialog::open_version`] to preselect a version.
  pub fn open(project: ProjectRef, target: Option<String>, window: &mut Window, cx: &mut App) {
    Self::open_version(project, target, None, window, cx);
  }

  pub fn open_version(
    project: ProjectRef,
    target: Option<String>,
    version_id: Option<String>,
    window: &mut Window,
    cx: &mut App,
  ) {
    let title: SharedString = format!("Install {}", project.title).into();
    let noun = if project.kind == proton_content::model::ContentKind::Mod {
      "mod"
    } else {
      "pack"
    };
    let description: SharedString = format!(
      "Choose which profile gets this {noun}. Only compatible profiles can be selected."
    )
    .into();
    let view = cx.new(|cx| {
      let mut this = Self::new(project, version_id, window, cx);
      this.chosen = target;
      this
    });
    window.open_dialog(cx, move |dialog, _, _| {
      dialog
        .title(title.clone())
        .w(px(520.))
        .child(
          DialogDescription::new()
            .mb_2()
            .child(description.clone()),
        )
        .child(view.clone())
    });
  }

  fn new(
    project: ProjectRef,
    version_id: Option<String>,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) -> Self {
    let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search profiles"));
    let this = cx.entity().downgrade();
    let select = cx.new(|cx| {
      let mut s = VersionSelect::new("Version", vec![], None, window, cx).on_change(
        move |label, _, cx| {
          let label = label.to_string();
          let _ = this.update(cx, |this, cx| {
            let v = this
              .versions
              .iter()
              .find(|v| logic::version_label(v) == label)
              .cloned();
            this.set_version(v, cx);
          });
        },
      );
      s.set_disabled(true, cx);
      s
    });
    let profiles = cx.profiles().profiles.clone();
    let target = target(cx);
    let icons = profile_icons(cx);
    let subs = vec![
      cx.observe_in(&profiles, window, |this, _, window, cx| this.sync(window, cx)),
      cx.observe_in(&target, window, |this, _, window, cx| this.sync(window, cx)),
      cx.observe(&icons, |_, _, cx| cx.notify()),
      cx.subscribe(&search, |_, _, e: &InputEvent, cx| {
        if matches!(e, InputEvent::Change) {
          cx.notify()
        }
      }),
    ];
    cx.defer_in(window, |this, window, cx| this.sync(window, cx));
    Self {
      project,
      version_id,
      search,
      chosen: None,
      loaded_for: None,
      versions: vec![],
      version: None,
      select,
      plan: PlanState::Idle,
      manual: None,
      _load: None,
      _subs: subs,
    }
  }

  /// (profile, compat) for every profile
  fn tiles(&self, cx: &App) -> Vec<(Profile, logic::Compat)> {
    cx.profiles()
      .profiles
      .read(cx)
      .iter()
      .map(|p| {
        let compat = logic::profile_compat(
          p,
          self.project.kind,
          &self.project.game_versions,
          &self.project.loaders,
        );
        (p.clone(), compat)
      })
      .collect()
  }

  /// Picked, else remembered target, else first compatible
  fn profile(&self, cx: &App) -> Option<Profile> {
    let ok: Vec<Profile> = self
      .tiles(cx)
      .into_iter()
      .filter(|(_, c)| c.ok)
      .map(|(p, _)| p)
      .collect();
    let remembered = cx
      .try_global::<super::GlobalTarget>()
      .and_then(|t| t.0.read(cx).clone());
    ok.iter()
      .find(|p| Some(&p.id) == self.chosen.as_ref())
      .or_else(|| ok.iter().find(|p| Some(&p.id) == remembered.as_ref()))
      .or(ok.first())
      .cloned()
  }

  /// Reloads versions when the profile changed
  fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let profile = self.profile(cx).map(|p| p.id);
    if profile == self.loaded_for {
      cx.notify();
      return;
    }
    self.loaded_for = profile.clone();
    self.versions.clear();
    self.set_version(None, cx);
    let Some(profile) = profile else {
      return;
    };
    let load = cx
      .content()
      .versions(self.project.source, self.project.id.clone(), Some(profile));
    self._load = Some(cx.spawn_in(window, async move |this, cx| {
      let versions = load.await.unwrap_or_default();
      let _ = this.update_in(cx, |this, window, cx| {
        let version = logic::default_version(&versions, this.version_id.as_deref()).cloned();
        let labels = versions
          .iter()
          .map(|v| logic::version_label(v).into())
          .collect();
        let value = version.as_ref().map(|v| logic::version_label(v).into());
        let empty = versions.is_empty();
        this.versions = versions;
        this.select.update(cx, |s, cx| {
          s.set_options(labels, value, window, cx);
          s.set_disabled(empty, cx);
        });
        this.set_version(version, cx);
      });
    }));
  }

  fn set_version(&mut self, version: Option<VersionInfo>, cx: &mut Context<Self>) {
    self.version = version;
    self.manual = None;
    let (Some(profile), Some(version)) = (self.loaded_for.clone(), self.version.clone()) else {
      self.plan = PlanState::Idle;
      cx.notify();
      return;
    };
    self.plan = PlanState::Loading;
    let plan = cx.content().plan_install(
      profile,
      vec![PickRef {
        source: self.project.source,
        project_id: self.project.id.clone(),
        version_id: Some(version.id),
      }],
    );
    self._load = Some(cx.spawn(async move |this, cx| {
      let res = plan.await;
      let _ = this.update(cx, |this, cx| this.set_plan(res, cx));
    }));
    cx.notify();
  }

  /// Shows a plan (or its error)
  pub fn set_plan(&mut self, plan: anyhow::Result<InstallPlan>, cx: &mut Context<Self>) {
    self.plan = match plan {
      Ok(plan) => {
        self.manual = (!plan.manual.is_empty()).then(|| {
          let view = cx.new(|cx| ManualDownloads::new(plan.manual.clone(), cx));
          cx.observe(&view, |_, _, cx| cx.notify()).detach();
          view
        });
        PlanState::Ready(plan)
      }
      Err(e) => PlanState::Failed {
        incompatible: matches!(content_error(&e), Some(ContentError::Incompatible)),
      },
    };
    cx.notify();
  }

  fn can_install(&self, cx: &App) -> bool {
    matches!(&self.plan, PlanState::Ready(p) if p.conflicts.is_empty())
      && self.manual.as_ref().is_none_or(|m| m.read(cx).done())
  }

  fn install(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let PlanState::Ready(plan) = &self.plan else {
      return;
    };
    let Some(profile) = self.profile(cx) else {
      return;
    };
    let mut plan = plan.clone();
    let found = self
      .manual
      .as_ref()
      .map(|m| m.read(cx).found().clone())
      .unwrap_or_default();
    let project = self.project.title.clone();
    window.close_dialog(cx);
    set_target(cx, Some(profile.id.clone()));
    let manual = std::mem::take(&mut plan.manual);
    cx.spawn(async move |_, cx| {
      // restricted files come from the download folders
      for file in manual {
        let Some(path) = found.get(&file.file_id).cloned() else {
          continue;
        };
        let name = file.file_name.clone();
        let task = cx.update(|cx| cx.content().clone().adopt_file(&profile.id, file, path, cx));
        if let Err(e) = task.await {
          cx.update(|cx| {
            toast_error(&format!("Failed to add {name} to {}", profile.name), &e, cx)
          });
          return;
        }
      }
      if plan.items.is_empty() {
        cx.update(|cx| {
          toast(
            Notification::success(format!("Installed {project} in {}", profile.name)),
            cx,
          )
        });
        return;
      }
      let what = match plan.items.as_slice() {
        [one] => one.name.clone(),
        items => format!("{} items", items.len()),
      };
      let title = format!("{}: install", profile.name);
      let task = cx.update(|cx| cx.content().clone().install(plan, title, cx));
      let res = task.await;
      cx.update(|cx| match res {
        Ok(true) => toast(
          Notification::success(format!("Installed {what} in {}", profile.name)),
          cx,
        ),
        Ok(false) => toast(Notification::warning("Download canceled"), cx),
        Err(e) => toast_error(
          &format!("Failed to install content in {}", profile.name),
          &e,
          cx,
        ),
      });
    })
    .detach();
  }
}

impl Render for InstallDialog {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let theme = cx.theme();
    let muted = theme.muted_foreground;
    let tiles = self.tiles(cx);
    let profile = self.profile(cx);
    let needle = self.search.read(cx).value().trim().to_lowercase();
    let total = tiles.len();
    let shown: Vec<_> = tiles
      .into_iter()
      .filter(|(p, _)| p.name.to_lowercase().contains(&needle))
      .collect();
    let can_install = self.can_install(cx);
    let selected = profile.as_ref().map(|p| p.id.clone());

    let list = div()
      .id("install-profiles")
      .flex()
      .flex_col()
      .gap_2()
      .max_h(px(288.))
      .overflow_y_scroll()
      .when(shown.is_empty(), |el| {
        el.child(div().text_sm().text_color(muted).child(if total == 0 {
          "No profiles yet."
        } else {
          "No matching profiles."
        }))
      })
      .children(shown.into_iter().map(|(p, compat)| {
        let checked = selected.as_ref() == Some(&p.id);
        let note = logic::profile_note(&p, &compat);
        let id = p.id.clone();
        let icon = profile_icon(&p.id, cx);
        input_tile(cx)
          .id(SharedString::from(format!("profile-{}", p.id)))
          .flex()
          .items_center()
          .gap_3()
          .px_3()
          .py(px(10.))
          .when(!compat.ok, |el| el.opacity(0.5))
          .when(compat.ok, |el| {
            el.cursor_pointer()
              .on_click(cx.listener(move |this, _, window, cx| {
                this.chosen = Some(id.clone());
                this.sync(window, cx);
              }))
          })
          .child(
            Radio::new(SharedString::from(format!("radio-{}", p.id)))
              .checked(checked)
              .disabled(!compat.ok),
          )
          .child(
            ProfileIcon::new(icon)
              .size(px(36.))
              .fallback_size(px(20.)),
          )
          .child(
            div()
              .flex()
              .flex_col()
              .flex_1()
              .min_w_0()
              .child(
                div()
                  .truncate()
                  .text_sm()
                  .font_weight(FontWeight::MEDIUM)
                  .child(p.name.clone()),
              )
              .child(div().text_xs().text_color(muted).child(note)),
          )
      }));

    let plan = match &self.plan {
      PlanState::Idle => div().into_any_element(),
      PlanState::Loading => div()
        .flex()
        .items_center()
        .gap_2()
        .text_sm()
        .text_color(muted)
        .child(Spinner::new().small())
        .child("Checking dependencies")
        .into_any_element(),
      PlanState::Failed { incompatible } => {
        let (title, text) = if *incompatible {
          (
            "Not compatible with this profile",
            "This version can not run with the loader or Minecraft version of the profile.",
          )
        } else {
          (
            "Could not resolve dependencies",
            "Check your connection and try again.",
          )
        };
        Alert::error("plan-error", text)
          .title(title)
          .icon(IconName::CircleAlert)
          .into_any_element()
      }
      PlanState::Ready(plan) => plan_view(plan, cx),
    };

    let target_name = profile
      .as_ref()
      .map(|p| p.name.clone())
      .unwrap_or_else(|| "profile".into());

    div()
      .flex()
      .flex_col()
      .gap(px(18.))
      .child(Input::new(&self.search))
      .child(list)
      .child(
        div()
          .flex()
          .flex_col()
          .gap(px(6.))
          .child(
            div()
              .text_sm()
              .font_weight(FontWeight::MEDIUM)
              .child("Version"),
          )
          .child(self.select.clone()),
      )
      .child(plan)
      .when_some(self.manual.clone(), |el, manual| {
        el.child(
          div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(div().text_sm().child(MANUAL_HINT))
            .child(manual),
        )
      })
      .child(
        DialogFooter::new()
          .child(
            Button::new("cancel")
              .outline()
              .label("Cancel")
              .on_click(|_, window, cx| window.close_dialog(cx)),
          )
          .child(
            Button::new("install")
              .primary()
              .icon(Icon::new(IconName::Download))
              .label(format!("Install to {target_name}"))
              .disabled(!can_install)
              .on_click(cx.listener(|this, _, window, cx| this.install(window, cx))),
          ),
      )
  }
}

#[cfg(test)]
mod tests {
  use gpui_kit::{self as gpui, TestAppContext, test::TestWindowExt};
  use proton_components::test_view;
  use proton_content::model::{
    Channel, Conflict, ContentKind, OptionalRef, PlanItem, ProjectRef as Dep, Source,
  };

  use super::*;
  use crate::content::logic::tests::version;

  fn fixture() -> InstallPlan {
    InstallPlan {
      profile: "p".into(),
      items: vec![
        PlanItem {
          name: "Iris".into(),
          icon_url: None,
          kind: ContentKind::Mod,
          version: version("1", Channel::Release),
          reason: PlanReason::Requested,
          dependency_of: None,
          replaces: None,
        },
        PlanItem {
          name: "Sodium".into(),
          icon_url: None,
          kind: ContentKind::Mod,
          version: version("2", Channel::Release),
          reason: PlanReason::Dependency,
          dependency_of: Some("iris".into()),
          replaces: None,
        },
      ],
      already_installed: vec![Dep {
        project_id: "fabric-api".into(),
        name: "Fabric API".into(),
        dependency_of: None,
      }],
      optional: vec![OptionalRef {
        source: Source::Modrinth,
        project_id: "mm".into(),
        name: "Mod Menu".into(),
      }],
      conflicts: vec![Conflict {
        project_id: "of".into(),
        name: "OptiFine".into(),
        reason: "incompatible".into(),
      }],
      manual: vec![],
      total_bytes: 0,
    }
  }

  #[gpui::test]
  fn renders_a_fixture_plan(cx: &mut TestAppContext) {
    let plan = fixture();
    assert_eq!(
      plan_rows(&plan),
      [
        ("Fabric API".into(), "Already installed".into(), true),
        ("Sodium".into(), "Will install v2".into(), false),
      ]
    );
    let (handle, _) = test_view::open(cx, move |_, cx| {
      div().w(px(520.)).child(plan_view(&plan, cx)).into_any_element()
    });
    cx.update_window(handle, |_, window, cx| {
      window.render_frame(cx);
      assert!(window.try_find("plan-needed").is_some());
    })
    .unwrap();
  }
}
