//! `/settings/minecraft` and `/settings/java`: global defaults for every profile

use gpui_kit::{
  AppContext, Context, ElementId, Entity, FontWeight, IntoElement, ParentElement,
  PathPromptOptions, Render, Styled, Subscription, Window,
  assets::IconName,
  component::{
    ActiveTheme, Sizable,
    button::{Button, ButtonVariants},
  },
  div, px,
};
use proton_components::widgets::{form::SwitchTile, input_tile};
use proton_config::{SettingsExt, settings::system_max_mem};

use super::{
  forms::{GameSettingsForm, JavaSettingsForm, memory_with_mb},
  heading, page,
};

const SUBTITLE: &str = "Defaults for every profile. Profiles can override them.";

/// `settings.system_max_mem ?? 8192`: the store value when set, else the detected memory
fn max_mem(cx: &gpui_kit::App) -> u64 {
  match cx.settings().get(cx).system_max_mem {
    0 => match system_max_mem() {
      0 => 8192,
      mb => mb,
    },
    mb => mb,
  }
}

pub struct MinecraftPage {
  game: Entity<GameSettingsForm>,
  _sub: Subscription,
}

impl MinecraftPage {
  pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
    let settings = cx.settings().settings.clone();
    let initial = settings.read(cx).minecraft.game_settings.clone();
    let game = cx.new(|cx| {
      GameSettingsForm::new(initial, window, cx).on_change(|next, _, cx| {
        let next = next.clone();
        proton_config::update(cx, |s| s.minecraft.game_settings = next);
      })
    });
    let sub = cx.observe_in(
      &settings,
      window,
      |this: &mut Self, settings, window, cx| {
        let value = settings.read(cx).minecraft.game_settings.clone();
        this
          .game
          .update(cx, |form, cx| form.set_settings(value, window, cx));
        cx.notify();
      },
    );
    Self { game, _sub: sub }
  }

  fn add_dir(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let paths = cx.prompt_for_paths(PathPromptOptions {
      files: false,
      directories: true,
      multiple: false,
      prompt: Some("Add download folder".into()),
    });
    cx.spawn_in(window, async move |_, cx| {
      let Ok(Ok(Some(paths))) = paths.await else {
        return;
      };
      cx.update(|_, cx| {
        proton_config::update(cx, |s| {
          let dirs = s.download_dirs.get_or_insert_default();
          for path in paths {
            if !dirs.contains(&path) {
              dirs.push(path);
            }
          }
        })
      })
      .ok();
    })
    .detach();
  }
}

impl Render for MinecraftPage {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let settings = cx.settings().get(cx);
    let show_snapshots = settings.minecraft.show_snapshots;
    let dirs = settings.download_dirs.clone().unwrap_or_default();
    let muted = cx.theme().muted_foreground;

    let folders = input_tile(cx)
      .flex()
      .flex_col()
      .gap_2()
      .px(px(14.))
      .py_3()
      .child(
        div()
          .flex()
          .flex_col()
          .child(
            div()
              .line_height(px(24.))
              .font_weight(FontWeight::MEDIUM)
              .child("Download folders"),
          )
          .child(
            div()
              .text_xs()
              .line_height(px(16.))
              .text_color(muted)
              .child("Watched for files CurseForge only allows from its website."),
          ),
      )
      .children(dirs.into_iter().map(|dir| {
        let label = dir.display().to_string();
        div()
          .flex()
          .items_center()
          .gap_2()
          .child(
            div()
              .flex_1()
              .min_w_0()
              .truncate()
              .text_sm()
              .child(label.clone()),
          )
          .child(
            Button::new(ElementId::Name(format!("remove-dir-{label}").into()))
              .ghost()
              .icon(IconName::X)
              .tooltip(format!("Remove {label}"))
              .on_click(move |_, _, cx| {
                proton_config::update(cx, |s| {
                  if let Some(dirs) = &mut s.download_dirs {
                    dirs.retain(|d| *d != dir);
                  }
                })
              }),
          )
      }))
      .child(
        div().flex().child(
          Button::new("add-folder")
            .outline()
            .small()
            // shadcn size sm, the kit's small is 24px
            .h_7()
            .icon(IconName::FolderPlus)
            .label("Add folder")
            .on_click(cx.listener(|this, _, window, cx| this.add_dir(window, cx))),
        ),
      );

    page(
      "minecraft-page",
      px(16.),
      [
        heading("Minecraft", Some(SUBTITLE.into()), cx).into_any_element(),
        SwitchTile::new(
          "show-snapshots",
          "Show snapshots",
          "List snapshots and pre-releases when creating a profile.",
        )
        .checked(show_snapshots)
        .on_change(|v, _, cx| {
          let v = *v;
          proton_config::update(cx, |s| s.minecraft.show_snapshots = v);
        })
        .into_any_element(),
        folders.into_any_element(),
        self.game.clone().into_any_element(),
      ],
    )
  }
}

pub struct JavaPage {
  java: Entity<JavaSettingsForm>,
  _sub: Subscription,
}

impl JavaPage {
  pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
    let settings = cx.settings().settings.clone();
    let initial = settings.read(cx).minecraft.jvm_settings.clone();
    let max = max_mem(cx);
    let java = cx.new(|cx| {
      JavaSettingsForm::new(
        initial,
        max,
        memory_with_mb,
        format!(
          "Your system has {} GB. Leave at least 2 GB for the OS.",
          (max as f64 / 1024.).round()
        ),
        window,
        cx,
      )
      .on_change(|next, _, cx| {
        let next = next.clone();
        proton_config::update(cx, |s| s.minecraft.jvm_settings = next);
      })
    });
    let sub = cx.observe_in(
      &settings,
      window,
      |this: &mut Self, settings, window, cx| {
        let value = settings.read(cx).minecraft.jvm_settings.clone();
        this
          .java
          .update(cx, |form, cx| form.set_settings(value, window, cx));
      },
    );
    Self { java, _sub: sub }
  }
}

impl Render for JavaPage {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    page(
      "java-page",
      px(16.),
      [
        heading("Java", Some(SUBTITLE.into()), cx).into_any_element(),
        self.java.clone().into_any_element(),
      ],
    )
  }
}
