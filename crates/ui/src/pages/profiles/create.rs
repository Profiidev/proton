//! "Create profile" dialog: the Custom form plus the From modpack / Import file tabs, whose
//! bodies the content pages plug in with [`set_create_tab`].

use std::path::Path;

use gpui_kit::{
  AnyView, App, AppContext, Context, Entity, FontWeight, Global, InteractiveElement, IntoElement,
  ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Subscription,
  TestSupportExt, Window,
  component::{
    ActiveTheme, Disableable, Sizable, WindowExt,
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
    switch::Switch,
  },
  div,
  prelude::FluentBuilder,
  px,
};
use proton_components::widgets::form::{ImageInput, VersionSelect};
use proton_components::widgets::label;
use proton_components::widgets::tabs::segmented_tabs;
use proton_config::SettingsExt;
use proton_profiles::{ProfilesExt, config::ProfileError};
use proton_utils::log::ErrorLogExt;
use proton_versions::{VersionsExt, loader::LoaderType};

use super::loader_name;
use crate::actions::toast_error;

pub const LOADERS: [LoaderType; 5] = [
  LoaderType::Vanilla,
  LoaderType::Fabric,
  LoaderType::Quilt,
  LoaderType::Forge,
  LoaderType::NeoForge,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreateTab {
  Custom,
  FromModpack,
  ImportFile,
}

/// Builds a tab body each time the tab is selected (dropped when it is left)
pub type TabBuilder = fn(&mut Window, &mut App) -> AnyView;

#[derive(Default)]
struct CreateTabs {
  modpack: Option<TabBuilder>,
  import: Option<TabBuilder>,
}

impl Global for CreateTabs {}

/// Plugs in the body of the "From modpack" or "Import file" tab. To hand over to the
/// modpack install dialog, the body calls `window.close_dialog(cx)` first.
/// [`CreateTab::Custom`] is built in and ignored here.
pub fn set_create_tab(cx: &mut App, tab: CreateTab, builder: TabBuilder) {
  let tabs = cx.default_global::<CreateTabs>();
  match tab {
    CreateTab::FromModpack => tabs.modpack = Some(builder),
    CreateTab::ImportFile => tabs.import = Some(builder),
    CreateTab::Custom => {}
  }
}

/// Opens the dialog on the Custom tab
pub fn open(window: &mut Window, cx: &mut App) {
  let view = cx.new(|cx| CreateDialog::new(window, cx));
  window.open_dialog(cx, move |dialog, _, _| {
    dialog.w(px(620.)).p_6().child(view.clone())
  });
}

pub struct CreateDialog {
  pub(crate) tab: CreateTab,
  /// Body of a plugged-in tab while it is selected
  body: Option<AnyView>,
  pub(crate) name: Entity<InputState>,
  pub(crate) icon: Entity<ImageInput>,
  pub(crate) loader: LoaderType,
  pub(crate) version: Entity<VersionSelect>,
  pub(crate) loader_version: Entity<VersionSelect>,
  pub(crate) name_error: Option<SharedString>,
  pub(crate) version_error: Option<SharedString>,
  pub(crate) icon_error: Option<SharedString>,
  submitting: bool,
  show_snapshots: bool,
  /// Bumped per list request, drops answers to outdated ones
  versions_gen: usize,
  loader_gen: usize,
  _subs: [Subscription; 3],
}

impl CreateDialog {
  pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
    let name = cx.new(|cx| InputState::new(window, cx).placeholder("Survival SMP"));
    let this = cx.weak_entity();
    let version = cx.new(|cx| {
      VersionSelect::new("Minecraft version", vec![], None, window, cx).on_change(
        move |_, window, cx| {
          this
            .update(cx, |this, cx| this.reload_loader_versions(window, cx))
            .ok();
        },
      )
    });
    let loader_version =
      cx.new(|cx| VersionSelect::new("Loader version", vec![], None, window, cx));
    let settings = cx.settings().settings.clone();
    let versions = cx.versions().versions.clone();
    let subs = [
      cx.subscribe_in(&name, window, |this, _, event, window, cx| match event {
        InputEvent::Change if this.name_error.is_some() => {
          this.name_error = None;
          cx.notify();
        }
        InputEvent::PressEnter { .. } => this.submit(window, cx),
        _ => {}
      }),
      // the version lists follow the global snapshot setting
      cx.observe_in(&settings, window, |this, settings, window, cx| {
        let show = settings.read(cx).minecraft.show_snapshots;
        if show != this.show_snapshots {
          this.show_snapshots = show;
          this.reload_versions(window, cx);
          cx.notify();
        }
      }),
      // manifests loaded or refreshed
      cx.observe_in(&versions, window, |this, _, window, cx| {
        this.reload_versions(window, cx)
      }),
    ];
    let mut this = Self {
      tab: CreateTab::Custom,
      body: None,
      name,
      icon: cx.new(|_| ImageInput::new(None).size(px(72.)).placeholder("Icon")),
      loader: LoaderType::Vanilla,
      version,
      loader_version,
      name_error: None,
      version_error: None,
      icon_error: None,
      submitting: false,
      show_snapshots: cx.settings().get(cx).minecraft.show_snapshots,
      versions_gen: 0,
      loader_gen: 0,
      _subs: subs,
    };
    this.reload_versions(window, cx);
    this
  }

  fn select_tab(&mut self, tab: CreateTab, window: &mut Window, cx: &mut Context<Self>) {
    if tab == self.tab {
      return;
    }
    let tabs = cx.default_global::<CreateTabs>();
    let builder = match tab {
      CreateTab::Custom => None,
      CreateTab::FromModpack => tabs.modpack,
      CreateTab::ImportFile => tabs.import,
    };
    self.tab = tab;
    self.body = builder.map(|b| b(window, cx));
    cx.notify();
  }

  pub(crate) fn set_loader(
    &mut self,
    loader: LoaderType,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    if loader != self.loader {
      self.loader = loader;
      self.reload_versions(window, cx);
      cx.notify();
    }
  }

  /// Minecraft versions of the loader; keeps the selection if it is still listed
  fn reload_versions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    self.versions_gen += 1;
    let generation = self.versions_gen;
    let task = cx.versions().clone().version_list(self.loader, cx);
    cx.spawn_in(window, async move |this, cx| {
      let list = task.await.log_err().unwrap_or_default();
      this
        .update_in(cx, |this, window, cx| {
          if this.versions_gen != generation {
            return;
          }
          let current = this.version.read(cx).value(cx);
          let value = current
            .filter(|v| list.iter().any(|l| l == v.as_ref()))
            .or_else(|| list.first().map(SharedString::from));
          let options = list.into_iter().map(SharedString::from).collect();
          this
            .version
            .update(cx, |s, cx| s.set_options(options, value, window, cx));
          this.reload_loader_versions(window, cx);
        })
        .ok();
    })
    .detach();
  }

  /// Loader versions for the selected Minecraft version, cleared for Vanilla
  fn reload_loader_versions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    self.loader_gen += 1;
    let generation = self.loader_gen;
    let version = self.version.read(cx).value(cx);
    let (
      LoaderType::Fabric | LoaderType::Quilt | LoaderType::Forge | LoaderType::NeoForge,
      Some(version),
    ) = (self.loader, version)
    else {
      self.loader_version.update(cx, |s, cx| {
        s.set_options(vec![], None, window, cx);
        s.set_disabled(true, cx);
      });
      return;
    };
    let task = cx
      .versions()
      .clone()
      .loader_version_list(self.loader, version.to_string(), cx);
    cx.spawn_in(window, async move |this, cx| {
      let list = task.await.log_err().unwrap_or_default();
      this
        .update_in(cx, |this, window, cx| {
          if this.loader_gen != generation {
            return;
          }
          let current = this.loader_version.read(cx).value(cx);
          let value = current
            .filter(|v| list.iter().any(|l| l == v.as_ref()))
            .or_else(|| list.first().map(SharedString::from));
          let empty = list.is_empty();
          let options = list.into_iter().map(SharedString::from).collect();
          this.loader_version.update(cx, |s, cx| {
            s.set_options(options, value, window, cx);
            s.set_disabled(empty, cx);
          });
        })
        .ok();
    })
    .detach();
  }

  /// Validates like the old zod schema, then creates the profile and closes the dialog
  pub(crate) fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if self.submitting {
      return;
    }
    let name = self.name.read(cx).value().to_string();
    let version = self.version.read(cx).value(cx);
    let icon = self.icon.read(cx).picked().map(Path::to_path_buf);
    self.name_error = name.is_empty().then(|| "Name is required".into());
    self.version_error = version.is_none().then(|| "Version is required".into());
    self.icon_error = icon
      .as_deref()
      .filter(|p| !is_image(p))
      .map(|_| "Invalid image file type".into());
    cx.notify();
    if self.name_error.is_some() || self.version_error.is_some() || self.icon_error.is_some() {
      return;
    }
    let Some(version) = version else { return };
    let icon = match icon.map(std::fs::read).transpose() {
      Ok(icon) => icon,
      Err(_) => {
        self.icon_error = Some("Invalid image".into());
        return;
      }
    };
    let loader_version = match self.loader {
      LoaderType::Vanilla => None,
      _ => self.loader_version.read(cx).value(cx).map(String::from),
    };
    self.submitting = true;
    let create = cx
      .profiles()
      .create(name, icon, version.to_string(), self.loader, loader_version);
    cx.spawn_in(window, async move |this, cx| {
      let result = create.await;
      this
        .update_in(cx, |this, window, cx| {
          this.submitting = false;
          cx.notify();
          match result {
            Ok(_) => window.close_dialog(cx),
            Err(e) if matches!(e.downcast_ref(), Some(ProfileError::InvalidImage)) => {
              this.icon_error = Some("Invalid image".into());
            }
            Err(e) => toast_error("Failed to create profile", &e, cx),
          }
        })
        .ok();
    })
    .detach();
  }

  fn render_custom(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let (muted_fg, primary, input) = (theme.muted_foreground, theme.primary, theme.input);
    let loaders = LOADERS.iter().enumerate().map(|(ix, &loader)| {
      let checked = loader == self.loader;
      div()
        .id(("loader", ix))
        .flex()
        .h_9()
        .items_center()
        .justify_center()
        .px_1()
        .rounded(px(10.))
        .border_1()
        .cursor_pointer()
        .text_size(px(13.))
        .font_weight(FontWeight::MEDIUM)
        .whitespace_nowrap()
        .map(|el| {
          if checked {
            el.border_color(primary).bg(primary.opacity(0.35))
          } else {
            el.border_color(input)
              .bg(input.opacity(0.3))
              .hover(move |s| s.bg(input.opacity(0.6)))
          }
        })
        .on_click(cx.listener(move |this, _, window, cx| this.set_loader(loader, window, cx)))
        .child(loader_name(loader))
        .test_support()
    });

    let snapshots = self.show_snapshots;
    div()
      .flex()
      .flex_col()
      .gap_4()
      .child(
        div()
          .flex()
          .items_end()
          .gap(px(14.))
          .child(
            div()
              .flex()
              .flex_col()
              .gap_1p5()
              .child(self.icon.clone())
              .children(error(self.icon_error.clone(), cx)),
          )
          .child(field(
            "Name",
            Input::new(&self.name).id("name"),
            self.name_error.clone(),
            cx,
          )),
      )
      .child(
        div()
          .flex()
          .flex_col()
          .gap_1p5()
          .child(label("Loader"))
          .child(div().grid().grid_cols(5).gap_1p5().children(loaders)),
      )
      .child(
        div()
          .flex()
          .flex_wrap()
          .gap_3()
          .child(field(
            "Minecraft version",
            self.version.clone(),
            self.version_error.clone(),
            cx,
          ))
          .child(field(
            "Loader version",
            self.loader_version.clone(),
            None,
            cx,
          )),
      )
      .child(
        div()
          .flex()
          .items_center()
          .gap_2()
          .text_size(px(13.))
          .child(
            Switch::new("snapshots")
              .small()
              .checked(snapshots)
              .on_click(|checked, _, cx| {
                let checked = *checked;
                proton_config::update(cx, |s| s.minecraft.show_snapshots = checked);
              }),
          )
          .child("Show snapshots"),
      )
      .child(
        div()
          .text_xs()
          .text_color(muted_fg)
          .child("Minecraft, Java and the loader download when you first press play."),
      )
  }
}

/// `accept="image/png, image/jpeg, image/jpg"`
fn is_image(path: &Path) -> bool {
  path
    .extension()
    .and_then(|e| e.to_str())
    .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg"))
}

fn error(text: Option<SharedString>, cx: &App) -> Option<impl IntoElement + use<>> {
  let color = cx.theme().danger;
  text.map(|t| div().text_sm().text_color(color).child(t))
}

fn field(
  name: &'static str,
  control: impl IntoElement,
  err: Option<SharedString>,
  cx: &App,
) -> impl IntoElement {
  div()
    .flex()
    .flex_col()
    .flex_1()
    .min_w(px(220.))
    .gap_1p5()
    .child(label(name))
    .child(control)
    .children(error(err, cx))
}

impl Render for CreateDialog {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let muted_fg = cx.theme().muted_foreground;
    let selected = match self.tab {
      CreateTab::Custom => 0,
      CreateTab::FromModpack => 1,
      CreateTab::ImportFile => 2,
    };
    let tabs = segmented_tabs(
      "create-tabs",
      ["Custom", "From modpack", "Import file"],
      selected,
      cx.listener(|this, ix: &usize, window, cx| {
        let tab = [
          CreateTab::Custom,
          CreateTab::FromModpack,
          CreateTab::ImportFile,
        ][*ix];
        this.select_tab(tab, window, cx);
      }),
      cx,
    );

    let body = match self.tab {
      CreateTab::Custom => self.render_custom(cx).into_any_element(),
      _ => match self.body.clone() {
        Some(view) => view.into_any_element(),
        None => div()
          .h(px(320.))
          .flex()
          .items_center()
          .justify_center()
          .text_sm()
          .text_color(muted_fg)
          .child("Not available yet")
          .into_any_element(),
      },
    };

    let submitting = self.submitting;
    div()
      .id("create-profile")
      .test_support()
      .flex()
      .flex_col()
      .gap_5()
      .child(
        div()
          .flex()
          .flex_col()
          .gap_2()
          .child(
            div()
              .text_lg()
              .font_weight(FontWeight::SEMIBOLD)
              .line_height(px(18.))
              .child("Create profile"),
          )
          .child(
            div()
              .text_sm()
              .text_color(muted_fg)
              .child("Pick a loader and version, or start from a modpack."),
          ),
      )
      .child(div().flex().flex_col().gap_4().child(tabs).child(body))
      .when(self.tab == CreateTab::Custom, |el| {
        el.child(
          div()
            .flex()
            .justify_end()
            .gap_2()
            .child(
              Button::new("cancel")
                .outline()
                .label("Cancel")
                .on_click(|_, window, cx| window.close_dialog(cx)),
            )
            .child(
              Button::new("create")
                .primary()
                .label("Create profile")
                .loading(submitting)
                .disabled(submitting)
                .on_click(cx.listener(|this, _, window, cx| this.submit(window, cx))),
            ),
        )
      })
  }
}
