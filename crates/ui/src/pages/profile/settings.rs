use gpui_kit::{
  AnyView, App, AppContext, Context, Entity, ImageSource, InteractiveElement, IntoElement,
  ParentElement, PathPromptOptions, Render, SharedString, StatefulInteractiveElement, Styled,
  Subscription, WeakEntity, Window,
  assets::IconName,
  component::{
    ActiveTheme,
    button::Button,
    input::{Input, InputEvent, InputState},
    notification::Notification,
  },
  div,
  prelude::FluentBuilder,
  px,
};
use proton_components::widgets::label;
use proton_components::widgets::{
  form::{SwitchTile, VersionSelect, alert},
  profile::ProfileIcon,
};
use proton_config::{SettingsExt, settings::system_max_mem};
use proton_profiles::{
  ProfilesExt,
  config::{Profile, ProfileUpdate},
};
use proton_versions::{VersionsExt, loader::LoaderType};

use super::{find_profile, load_icon, quick_play::tabs, update_of};
use crate::{
  actions::{toast, toast_error},
  pages::settings::forms::{GameSettingsForm, JavaSettingsForm, memory_of_total},
  router::{ProfileSettingsTab, ProfileTab, Route, RouterExt},
};

const TABS: &[(&str, ProfileSettingsTab)] = &[
  ("General", ProfileSettingsTab::General),
  ("Minecraft", ProfileSettingsTab::Minecraft),
  ("Java", ProfileSettingsTab::Java),
];

/// Profile settings: "General" | "Minecraft" | "Java"
pub struct SettingsPage {
  id: String,
  tab: ProfileSettingsTab,
  profile: Option<Profile>,
  icon: Option<ImageSource>,
  pub(super) name: Entity<InputState>,
  versions: Entity<VersionSelect>,
  loader_versions: Entity<VersionSelect>,
  game_form: Entity<GameSettingsForm>,
  java_form: Entity<JavaSettingsForm>,
  _subscriptions: [Subscription; 3],
}

impl SettingsPage {
  pub fn new(
    id: String,
    tab: ProfileSettingsTab,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) -> Self {
    let this = cx.entity().downgrade();
    let name = cx.new(|cx| InputState::new(window, cx));
    let versions = cx.new(|cx| {
      let this = this.clone();
      VersionSelect::new("Minecraft version", vec![], None, window, cx).on_change(
        move |v, _, cx| {
          let v = v.to_string();
          this.update(cx, |this, cx| this.change_version(v, cx)).ok();
        },
      )
    });
    let loader_versions = cx.new(|cx| {
      let this = this.clone();
      VersionSelect::new("Loader version", vec![], None, window, cx).on_change(move |v, _, cx| {
        let v = v.to_string();
        this
          .update(cx, |this, cx| {
            this.send(cx, |u| u.loader_version = Some(v));
          })
          .ok();
      })
    });
    let global = cx.settings().get(cx);
    let max_mem = match global.system_max_mem {
      0 => match system_max_mem() {
        0 => 8192,
        mb => mb,
      },
      mb => mb,
    };
    let initial_game = global.minecraft.game_settings.clone();
    let initial_jvm = global.minecraft.jvm_settings.clone();
    let game_form = cx.new(|cx| {
      let this = this.clone();
      GameSettingsForm::new(initial_game, window, cx).on_change(move |next, _, cx| {
        let next = next.clone();
        this
          .update(cx, |this, cx| {
            let Some(use_local) = this.profile.as_ref().map(|p| p.use_local_game) else {
              return;
            };
            this.send(cx, |u| {
              u.use_local_game = use_local;
              u.game = Some(next);
            });
          })
          .ok();
      })
    });
    let java_form = cx.new(|cx| {
      let this = this.clone();
      JavaSettingsForm::new(
        initial_jvm,
        max_mem,
        move |mb| memory_of_total(mb, max_mem),
        "Most modded profiles run well with 4 to 6 GB.",
        window,
        cx,
      )
      .on_change(move |next, _, cx| {
        let next = next.clone();
        this
          .update(cx, |this, cx| {
            let Some(use_local) = this.profile.as_ref().map(|p| p.use_local_jvm) else {
              return;
            };
            this.send(cx, |u| {
              u.use_local_jvm = use_local;
              u.jvm = Some(next);
            });
          })
          .ok();
      })
    });
    let profiles = cx.profiles().profiles.clone();
    let settings = cx.settings().settings.clone();
    let subscriptions = [
      cx.observe_in(&profiles, window, |this, _, window, cx| {
        this.sync_profile(window, cx)
      }),
      cx.observe_in(&settings, window, |this, _, window, cx| {
        this.sync_profile(window, cx)
      }),
      cx.subscribe_in(&name, window, |this, _, event, _, cx| {
        if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }) {
          this.rename(cx);
        }
      }),
    ];
    let mut this = Self {
      id,
      tab,
      profile: None,
      icon: None,
      name,
      versions,
      loader_versions,
      game_form,
      java_form,
      _subscriptions: subscriptions,
    };
    this.sync_profile(window, cx);
    this
  }

  fn sync_profile(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let old = self.profile.take();
    self.profile = find_profile(&self.id, cx);
    let Some(p) = self.profile.clone() else {
      return;
    };
    if old.as_ref().is_none_or(|o| o.name != p.name) {
      self
        .name
        .update(cx, |s, cx| s.set_value(p.name.clone(), window, cx));
    }
    let versions_changed = old.as_ref().is_none_or(|o| {
      (o.loader, &o.version, &o.loader_version) != (p.loader, &p.version, &p.loader_version)
    });
    if versions_changed {
      self.load_versions(&p, window, cx);
    }
    let global = &cx.settings().get(cx).minecraft;
    let game = p
      .game
      .clone()
      .unwrap_or_else(|| global.game_settings.clone());
    let jvm = p.jvm.clone().unwrap_or_else(|| global.jvm_settings.clone());
    let game_disabled = p.game.is_none() || !p.use_local_game;
    let jvm_disabled = p.jvm.is_none() || !p.use_local_jvm;
    self.game_form.update(cx, |f, cx| {
      f.set_settings(game, window, cx);
      f.set_disabled(game_disabled, cx);
    });
    self.java_form.update(cx, |f, cx| {
      f.set_settings(jvm, window, cx);
      f.set_disabled(jvm_disabled, cx);
    });
    // ponytail: re-reads the icon on every profiles change, one small file
    let load = load_icon(self.id.clone(), cx);
    cx.spawn(async move |this, cx| {
      let icon = load.await;
      this.update(cx, |this, cx| {
        this.icon = icon;
        cx.notify();
      })
    })
    .detach();
    cx.notify();
  }

  fn load_versions(&self, p: &Profile, window: &mut Window, cx: &mut Context<Self>) {
    let version: SharedString = p.version.clone().into();
    let list = cx.versions().clone().version_list(p.loader, cx);
    let select = self.versions.downgrade();
    cx.spawn_in(window, async move |_, cx| {
      let mut options: Vec<SharedString> = list
        .await
        .unwrap_or_default()
        .into_iter()
        .map(Into::into)
        .collect();
      if !options.contains(&version) {
        options.insert(0, version.clone());
      }
      select.update_in(cx, |s, window, cx| {
        s.set_options(options, Some(version), window, cx)
      })
    })
    .detach();

    if p.loader == LoaderType::Vanilla {
      return;
    }
    let current = p.loader_version.clone().map(SharedString::from);
    let list = cx
      .versions()
      .clone()
      .loader_version_list(p.loader, p.version.clone(), cx);
    let select = self.loader_versions.downgrade();
    cx.spawn_in(window, async move |_, cx| {
      let options = list
        .await
        .unwrap_or_default()
        .into_iter()
        .map(Into::into)
        .collect();
      select.update_in(cx, |s, window, cx| {
        s.set_options(options, current, window, cx)
      })
    })
    .detach();
  }

  /// Sends the profile with `f` applied; "Failed to update profile" on error
  fn send(&self, cx: &mut Context<Self>, f: impl FnOnce(&mut ProfileUpdate)) {
    let Some(p) = &self.profile else {
      return;
    };
    let mut update = update_of(p);
    f(&mut update);
    let task = cx.profiles().update(update);
    cx.spawn(async move |_, cx| {
      if let Err(e) = task.await {
        cx.update(|cx| toast_error("Failed to update profile", &e, cx));
      }
    })
    .detach();
  }

  pub(super) fn rename(&mut self, cx: &mut Context<Self>) {
    let Some(p) = &self.profile else {
      return;
    };
    let name = self.name.read(cx).value().trim().to_string();
    if name.is_empty() {
      toast(Notification::error("Profile name cannot be empty"), cx);
    } else if name != p.name {
      self.send(cx, |u| u.name = name);
    }
  }

  /// For modded profiles keeps the loader version when the new version has it, else the newest
  fn change_version(&mut self, version: String, cx: &mut Context<Self>) {
    let Some(p) = self.profile.clone() else {
      return;
    };
    if version == p.version {
      return;
    }
    if p.loader == LoaderType::Vanilla {
      self.send(cx, |u| u.version = version);
      return;
    }
    let list = cx
      .versions()
      .clone()
      .loader_version_list(p.loader, version.clone(), cx);
    cx.spawn(async move |this, cx| {
      let available = list.await.ok();
      let loader_version = match (available, p.loader_version) {
        (Some(list), Some(current)) if list.contains(&current) => Some(current),
        (Some(list), current) => list.into_iter().next().or(current),
        (None, current) => current,
      };
      this.update(cx, |this, cx| {
        this.send(cx, |u| {
          u.version = version;
          u.loader_version = loader_version;
        })
      })
    })
    .detach();
  }

  fn pick_icon(&mut self, cx: &mut Context<Self>) {
    let paths = cx.prompt_for_paths(PathPromptOptions {
      files: true,
      directories: false,
      multiple: false,
      prompt: Some("Select image".into()),
    });
    let id = self.id.clone();
    cx.spawn(async move |_, cx| {
      let Ok(Ok(Some(paths))) = paths.await else {
        return;
      };
      let Some(path) = paths.into_iter().next() else {
        return;
      };
      let result = async {
        let bytes = std::fs::read(&path)?;
        let task = cx.update(|cx| cx.profiles().update_icon(id, bytes));
        task.await
      }
      .await;
      if let Err(e) = result {
        cx.update(|cx| toast_error("Failed to update profile icon", &e, cx));
      }
    })
    .detach();
  }

  fn general(&self, p: &Profile, cx: &mut Context<Self>) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let modded = p.loader != LoaderType::Vanilla;
    div()
      .flex()
      .flex_col()
      .gap_4()
      .child(
        div()
          .flex()
          .items_center()
          .gap(px(14.))
          .child(
            div()
              .flex_none()
              .size_16()
              .rounded(px(8.))
              .overflow_hidden()
              .bg(theme.muted)
              .child(
                ProfileIcon::new(self.icon.clone())
                  .size(px(64.))
                  .fallback_size(px(32.)),
              ),
          )
          .child(
            Button::new("change-icon")
              .cursor_pointer()
              .outline()
              .label("Change icon")
              .on_click(cx.listener(|this, _, _, cx| this.pick_icon(cx))),
          ),
      )
      .child(
        div()
          .flex()
          .flex_col()
          .gap(px(6.))
          .child(label("Name"))
          .child(Input::new(&self.name).id("profile-name")),
      )
      .child(
        div()
          .flex()
          .flex_wrap()
          .gap_3()
          .child(
            div()
              .flex()
              .flex_col()
              .flex_1()
              .min_w(px(180.))
              .gap(px(6.))
              .child(label("Minecraft version"))
              .child(self.versions.clone()),
          )
          .when(modded, |el| {
            el.child(
              div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w(px(180.))
                .gap(px(6.))
                .child(label(format!("{:?} loader version", p.loader)))
                .child(self.loader_versions.clone()),
            )
          }),
      )
      .when(modded, |el| {
        el.child(
          alert(
            IconName::TriangleAlert,
            "Changing the version can break mods",
            "Mods built for another version may fail to load.",
            cx,
          )
          .id("version-warning"),
        )
      })
  }
}

/// "Use profile ..." switch over the form of the global settings
fn override_tile(
  this: WeakEntity<SettingsPage>,
  p: &Profile,
  java: bool,
  form: AnyView,
  cx: &App,
) -> impl IntoElement + use<> {
  let global = &cx.settings().get(cx).minecraft;
  let (game, jvm) = (
    p.game
      .clone()
      .unwrap_or_else(|| global.game_settings.clone()),
    p.jvm.clone().unwrap_or_else(|| global.jvm_settings.clone()),
  );
  let (title, description, checked) = if java {
    (
      "Use profile Java settings",
      "Override the global Java settings for this profile only.",
      p.use_local_jvm,
    )
  } else {
    (
      "Use profile game settings",
      "Override the global Minecraft settings for this profile only.",
      p.use_local_game,
    )
  };
  div()
    .flex()
    .flex_col()
    .gap_4()
    .child(
      SwitchTile::new("use-local", title, description)
        .checked(checked)
        .on_change(move |value, _, cx| {
          let (value, game, jvm) = (*value, game.clone(), jvm.clone());
          this
            .update(cx, |this, cx| {
              this.send(cx, |u| {
                if java {
                  u.use_local_jvm = value;
                  u.jvm = Some(jvm);
                } else {
                  u.use_local_game = value;
                  u.game = Some(game);
                }
              })
            })
            .ok();
        }),
    )
    .child(form)
}

impl Render for SettingsPage {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let id = self.id.clone();
    let body = self.profile.clone().map(|p| match self.tab {
      ProfileSettingsTab::General => self.general(&p, cx).into_any_element(),
      tab => {
        let java = tab == ProfileSettingsTab::Java;
        let form: AnyView = if java {
          self.java_form.clone().into()
        } else {
          self.game_form.clone().into()
        };
        override_tile(cx.entity().downgrade(), &p, java, form, cx).into_any_element()
      }
    });
    div()
      .id("profile-settings")
      .size_full()
      .overflow_y_scroll()
      .child(
        div()
          .flex()
          .flex_col()
          .gap_4()
          .max_w(px(720.))
          .child(tabs(
            "profile-settings-tabs",
            TABS,
            self.tab,
            move |tab, cx| {
              cx.navigate(Route::Profile {
                id: id.clone(),
                tab: ProfileTab::Settings(tab),
              })
            },
            cx,
          ))
          .children(body),
      )
  }
}
