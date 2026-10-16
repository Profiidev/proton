//! Modpack install / version change (`ModpackInstallDialog.svelte`)

use gpui_kit::{
  App, AppContext, Context, Entity, FontWeight, ImageSource, IntoElement, ParentElement, Render,
  SharedString, Styled, Subscription, Task, Window,
  assets::IconName,
  component::{
    ActiveTheme, Disableable, Sizable, WindowExt,
    alert::Alert,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    dialog::{DialogDescription, DialogFooter},
    input::{Input, InputEvent, InputState},
    notification::Notification,
    spinner::Spinner,
  },
  div,
  prelude::FluentBuilder,
  px,
};
use proton_components::widgets::{chip, content::ProjectIcon, form::VersionSelect};
use proton_config::SettingsExt;
use proton_content::{
  ContentError, ContentExt,
  model::{PackPlan, PackSource, Source, VersionInfo},
};
use proton_profiles::{ProfilesExt, config::Profile};

use super::{ManualDownloads, ProjectRef, content_error, logic};
use crate::actions::{launch, toast, toast_error};

/// The pack whose versions are listed
#[derive(Clone)]
struct Pack {
  source: Source,
  id: String,
  name: String,
  icon_url: Option<String>,
}

enum Remote {
  Loading,
  Failed { invalid: bool },
  Ready(Box<PackPlan>),
}

pub struct ModpackInstallDialog {
  pack: Option<Pack>,
  /// Change mode: the modpack profile
  profile: Option<Profile>,
  /// Given plan (imported file)
  given: Option<PackPlan>,
  remote: Remote,
  versions: Vec<VersionInfo>,
  version: Option<VersionInfo>,
  select: Entity<VersionSelect>,
  name: Entity<InputState>,
  /// Last name filled in from the plan, the field follows the plan until the user typed
  auto_name: String,
  start: bool,
  manual: Option<Entity<ManualDownloads>>,
  _load: Option<Task<()>>,
  _plan: Option<Task<()>>,
  _subs: Vec<Subscription>,
}

impl ModpackInstallDialog {
  /// Install mode for a project from Discover / the create dialog
  pub fn open(project: ProjectRef, window: &mut Window, cx: &mut App) {
    let pack = Pack {
      source: project.source,
      id: project.id,
      name: project.title,
      icon_url: project.icon_url,
    };
    let title = format!("Install {}", pack.name);
    Self::show(title, Some(pack), None, None, window, cx);
  }

  /// Install mode for an imported file's plan
  pub fn open_plan(plan: PackPlan, window: &mut Window, cx: &mut App) {
    let title = format!("Install {}", plan.name);
    Self::show(title, None, None, Some(plan), window, cx);
  }

  /// Change mode: switches the modpack profile `profile` to another pack version
  pub fn open_change(profile: &str, window: &mut Window, cx: &mut App) {
    let Some(profile) = cx
      .profiles()
      .profiles
      .read(cx)
      .iter()
      .find(|p| p.id == profile)
      .cloned()
    else {
      return;
    };
    let pack = profile.modpack.as_ref().map(|m| Pack {
      source: m.source,
      id: m.project_id.clone(),
      name: profile.name.clone(),
      icon_url: None,
    });
    let title = format!("Change version of {}", profile.name);
    Self::show(title, pack, Some(profile), None, window, cx);
  }

  fn show(
    title: String,
    pack: Option<Pack>,
    profile: Option<Profile>,
    given: Option<PackPlan>,
    window: &mut Window,
    cx: &mut App,
  ) {
    let description: SharedString = if profile.is_some() {
      "Pack files are replaced with the chosen version. Content you added stays, edited configs may be overwritten."
    } else {
      "This creates a new profile. Your other profiles are not touched."
    }
    .into();
    let title: SharedString = title.into();
    let view = cx.new(|cx| Self::new(pack, profile, given, window, cx));
    window.open_dialog(cx, move |dialog, _, _| {
      dialog
        .title(title.clone())
        .w(px(540.))
        .child(DialogDescription::new().mb_2().child(description.clone()))
        .child(view.clone())
    });
  }

  fn new(
    pack: Option<Pack>,
    profile: Option<Profile>,
    given: Option<PackPlan>,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) -> Self {
    let this = cx.entity().downgrade();
    let select = cx.new(|cx| {
      let mut s = VersionSelect::new("Pack version", vec![], None, window, cx).on_change(
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
    let initial = given
      .as_ref()
      .map(|p| p.name.clone())
      .or(pack.as_ref().map(|p| p.name.clone()))
      .unwrap_or_default();
    let name = cx.new(|cx| InputState::new(window, cx).default_value(initial.clone()));
    let subs = vec![cx.subscribe(&name, |_, _, e: &InputEvent, cx| {
      if matches!(e, InputEvent::Change) {
        cx.notify();
      }
    })];
    let mut this = Self {
      pack,
      profile,
      remote: Remote::Loading,
      versions: vec![],
      version: None,
      select,
      name,
      auto_name: initial,
      start: false,
      manual: None,
      _load: None,
      _plan: None,
      _subs: subs,
      given: None,
    };
    if let Some(plan) = given {
      this.set_manual(&plan, cx);
      this.given = Some(plan);
    }
    this.load_versions(window, cx);
    this
  }

  fn load_versions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let Some(pack) = self.pack.clone() else {
      return;
    };
    let load = cx.content().versions(pack.source, pack.id, None);
    self._load = Some(cx.spawn_in(window, async move |this, cx| {
      let versions = load.await.unwrap_or_default();
      let _ = this.update_in(cx, |this, window, cx| {
        let version = logic::default_version(&versions, None).cloned();
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
    self.version = version.clone();
    let (Some(pack), Some(version)) = (self.pack.clone(), version) else {
      return;
    };
    self.remote = Remote::Loading;
    if self.given.is_none() {
      self.manual = None;
    }
    let plan = cx.content().modpack_plan(PackSource::Remote {
      source: pack.source,
      project_id: pack.id,
      version_id: Some(version.id),
    });
    self._plan = Some(cx.spawn(async move |this, cx| {
      let res = plan.await;
      let _ = this.update(cx, |this, cx| {
        this.remote = match res {
          Ok(plan) => {
            if this.given.is_none() {
              this.set_manual(&plan, cx);
            }
            Remote::Ready(Box::new(plan))
          }
          Err(e) => Remote::Failed {
            invalid: matches!(content_error(&e), Some(ContentError::InvalidPack)),
          },
        };
        cx.notify();
      });
    }));
    cx.notify();
  }

  fn set_manual(&mut self, plan: &PackPlan, cx: &mut Context<Self>) {
    self.manual = (!plan.manual.is_empty()).then(|| {
      let view = cx.new(|cx| ManualDownloads::new(plan.manual.clone(), cx));
      cx.observe(&view, |_, _, cx| cx.notify()).detach();
      view
    });
  }

  fn current(&self) -> Option<&PackPlan> {
    self.given.as_ref().or(match &self.remote {
      Remote::Ready(p) => Some(p.as_ref()),
      _ => None,
    })
  }

  fn manual_done(&self, cx: &App) -> bool {
    self.manual.as_ref().is_none_or(|m| m.read(cx).done())
  }

  fn install(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let Some(plan) = self.current().cloned() else {
      return;
    };
    let found = self
      .manual
      .as_ref()
      .map(|m| m.read(cx).found().clone())
      .unwrap_or_default();
    let start = self.start;
    window.close_dialog(cx);

    if let Some(profile) = self.profile.clone() {
      let version = plan.version.clone();
      let task = cx.content().clone().modpack_update(
        &profile.id,
        plan,
        found,
        format!("{}: update pack", profile.name),
        cx,
      );
      cx.spawn(async move |_, cx| {
        let res = task.await;
        cx.update(|cx| match res {
          Ok(true) => {
            toast(
              Notification::success(format!("Updated {} to {version}", profile.name)),
              cx,
            );
            if start {
              launch(profile.id.clone(), None, cx);
            }
          }
          Ok(false) => toast(Notification::warning("Download canceled"), cx),
          Err(e) => toast_error(&format!("Failed to update {}", profile.name), &e, cx),
        });
      })
      .detach();
      return;
    }

    let name = self.name.read(cx).value().trim().to_string();
    let task =
      cx.content()
        .modpack_install(plan, name.clone(), found, format!("{name}: install"), cx);
    cx.spawn(async move |_, cx| {
      let res = task.await;
      cx.update(|cx| match res {
        Ok(id) if !id.is_empty() => {
          toast(
            Notification::success(format!("Installed modpack {name}")),
            cx,
          );
          if start {
            launch(id, None, cx);
          }
        }
        Ok(_) => toast(Notification::warning("Download canceled"), cx),
        Err(e) => toast_error(&format!("Failed to install modpack {name}"), &e, cx),
      });
    })
    .detach();
  }
}

fn gb(mb: u64) -> String {
  format!("{:.1} GB", mb as f64 / 1024.)
}

impl Render for ModpackInstallDialog {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let muted = cx.theme().muted_foreground;
    // the name follows the loaded plan until edited
    if self.profile.is_none()
      && let Some(plan_name) = self.current().map(|p| p.name.clone())
      && plan_name != self.auto_name
      && self.name.read(cx).value() == self.auto_name.as_str()
    {
      self.auto_name = plan_name.clone();
      self
        .name
        .update(cx, |n, cx| n.set_value(plan_name, window, cx));
    }
    let current = self.current().cloned();
    let icon = current
      .as_ref()
      .and_then(|p| p.icon_url.clone())
      .or(self.pack.as_ref().and_then(|p| p.icon_url.clone()))
      .map(ImageSource::from);

    let info = match (&current, &self.remote) {
      (Some(plan), _) => {
        let chips: Vec<String> = [
          (plan.counts.mods, "mod"),
          (plan.counts.resourcepacks, "resource pack"),
          (plan.counts.shaders, "shader"),
        ]
        .into_iter()
        .filter(|(n, _)| *n > 0)
        .map(|(n, w)| logic::plural(n, w))
        .collect();
        div()
          .flex()
          .flex_col()
          .gap(px(6.))
          .child(div().text_color(muted).child(format!(
            "{} · Minecraft {}",
            logic::loader_label(plan.loader),
            plan.minecraft
          )))
          .child(
            div()
              .flex()
              .flex_wrap()
              .gap(px(6.))
              .children(chips.into_iter().map(|c| chip(c, cx))),
          )
          .into_any_element()
      }
      (None, Remote::Failed { invalid }) => div()
        .text_sm()
        .text_color(cx.theme().danger)
        .child(if *invalid {
          "This pack version can not be read."
        } else {
          "Failed to load the pack, check your connection."
        })
        .into_any_element(),
      _ => div()
        .flex()
        .items_center()
        .gap_2()
        .text_sm()
        .text_color(muted)
        .child(Spinner::new().small())
        .child("Loading pack")
        .into_any_element(),
    };

    let installed_label = self
      .profile
      .as_ref()
      .and_then(|p| p.modpack.as_ref())
      .and_then(|m| self.versions.iter().find(|v| v.id == m.version_id))
      .map(|v| v.version_number.clone());

    let memory = current
      .as_ref()
      .and_then(|p| p.recommended_memory_mb)
      .filter(|_| self.profile.is_none())
      .map(|mb| {
        let max = cx.settings().get(cx).system_max_mem;
        let max = (max > 0).then_some(max);
        if logic::memory_fits(mb, max) {
          let mut text = format!("The pack asks for {}.", gb(mb));
          if let Some(max) = max {
            text += &format!(" Your system has {}, so nothing needs changing.", gb(max));
          }
          Alert::info("memory", text)
            .title("Memory is set up for you")
            .icon(IconName::Info)
        } else {
          let text = format!(
            "The pack asks for {}, but your system only has {}. The game may run slowly or crash.",
            gb(mb),
            gb(max.unwrap_or(0))
          );
          Alert::warning("memory", text)
            .title("Not enough memory for this pack")
            .icon(IconName::TriangleAlert)
        }
      });

    let manual_done = self.manual_done(cx);
    let installed_id = self
      .profile
      .as_ref()
      .and_then(|p| p.modpack.as_ref())
      .map(|m| m.version_id.clone());
    let can = current.is_some()
      && manual_done
      && match &self.profile {
        Some(_) => self.version.as_ref().map(|v| &v.id) != installed_id.as_ref(),
        None => !self.name.read(cx).value().trim().is_empty(),
      };
    let label = if self.profile.is_some() {
      "Change version"
    } else {
      "Create profile and install"
    };

    div()
      .flex()
      .flex_col()
      .gap(px(18.))
      .child(
        div()
          .flex()
          .items_center()
          .gap(px(14.))
          .child(ProjectIcon::new(icon).size(px(64.)))
          .child(info),
      )
      .child(
        div()
          .flex()
          .flex_wrap()
          .gap_3()
          .when(self.profile.is_none(), |el| {
            el.child(
              div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .flex_grow(1.)
                .min_w(px(200.))
                .child(
                  div()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child("Profile name"),
                )
                .child(Input::new(&self.name)),
            )
          })
          .when(self.pack.is_some(), |el| {
            el.child(
              div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .flex_grow(1.)
                .min_w(px(160.))
                .child(
                  div()
                    .flex()
                    .gap_1()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child("Pack version")
                    .when_some(installed_label, |el, v| {
                      el.child(
                        div()
                          .text_color(muted)
                          .font_weight(FontWeight::NORMAL)
                          .child(format!("· installed {v}")),
                      )
                    }),
                )
                .child(self.select.clone()),
            )
          }),
      )
      .children(memory)
      .when_some(
        current
          .as_ref()
          .filter(|p| !p.manual.is_empty())
          .map(|p| p.manual.len())
          .zip(self.manual.clone()),
        |el, (n, manual)| {
          el.child(
            div()
              .flex()
              .flex_col()
              .gap_2()
              .child(
                Alert::new(
                  "manual",
                  "CurseForge only allows these from its website. Download each one, it is picked up automatically.",
                )
                .title(format!("{} need a manual download", logic::plural(n, "file")))
                .icon(IconName::CircleAlert),
              )
              .child(manual),
          )
        },
      )
      .child(
        Checkbox::new("start")
          .checked(self.start)
          .label("Start the game when the download finishes")
          .on_click(cx.listener(|this, checked: &bool, _, cx| {
            this.start = *checked;
            cx.notify();
          })),
      )
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
              .label(label)
              .disabled(!can)
              .on_click(cx.listener(|this, _, window, cx| this.install(window, cx))),
          ),
      )
  }
}
