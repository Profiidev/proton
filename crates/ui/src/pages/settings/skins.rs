//! `/settings/skins`: 3D viewer of the worn skin, the local skin library

use std::{rc::Rc, sync::Arc};

use gpui_kit::{
  AnyElement, App, AppContext, Context, ElementId, Entity, FontWeight, InteractiveElement,
  IntoElement, ParentElement, PathPromptOptions, Pixels, Render, SharedString,
  StatefulInteractiveElement, Styled, Subscription, Window,
  assets::IconName,
  base::TestSupportExt,
  component::{
    ActiveTheme, Disableable, Icon, IndexPath, Sizable,
    button::{Button, ButtonVariants},
    notification::NotificationType,
    select::{SearchableVec, Select, SelectEvent, SelectState},
    spinner::Spinner,
  },
  div,
  prelude::FluentBuilder,
  px, size,
};
use proton_account::AccountsExt;
use proton_components::widgets::Callback;
use proton_skin_render::{
  Animation, BackEquipment, MiniSkinViewer, ModelType, SkinTextures, SkinViewer,
};
use proton_utils::log::ErrorLogExt;
use url::Url;

use super::{
  auto_fill_cols, content_width, heading, offline_guard, page, ring_card,
  textures::{self, SkinSrc, Textures, load_textures},
  toast,
};

type Choices = SelectState<SearchableVec<SharedString>>;

const ANIMATIONS: [(&str, Animation); 5] = [
  ("None", Animation::None),
  ("Idle", Animation::Idle),
  ("Walking", Animation::Walking),
  ("Running", Animation::Running),
  ("Waving", Animation::Waving),
];
const BACKS: [(&str, BackEquipment); 3] = [
  ("None", BackEquipment::None),
  ("Cape", BackEquipment::Cape),
  ("Elytra", BackEquipment::Elytra),
];

/// What the main viewer shows: skin url, cape url, model
type Shown = (Url, Option<Url>, ModelType);

pub struct SkinsPage {
  viewer: Entity<SkinViewer>,
  animation: Entity<Choices>,
  back: Entity<Choices>,
  back_choice: BackEquipment,
  shown: Option<Shown>,
  loading: bool,
  adding: bool,
  /// skin id being worn right now
  changing: Option<String>,
  _subs: Vec<Subscription>,
}

fn choices<T>(
  list: &[(&'static str, T)],
  selected: usize,
  window: &mut Window,
  cx: &mut App,
) -> Entity<Choices> {
  let items: Vec<SharedString> = list.iter().map(|(l, _)| SharedString::from(*l)).collect();
  cx.new(|cx| {
    SelectState::new(
      SearchableVec::new(items),
      Some(IndexPath::default().row(selected)),
      window,
      cx,
    )
  })
}

fn pick<T: Copy>(list: &[(&'static str, T)], label: &SharedString) -> Option<T> {
  list
    .iter()
    .find(|(l, _)| *l == label.as_ref())
    .map(|(_, v)| *v)
}

impl SkinsPage {
  pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
    let viewer = cx.new(|cx| {
      let mut viewer = SkinViewer::new(cx);
      viewer.set_size(size(px(210.), px(280.)), cx);
      viewer
    });
    // reduced motion starts the model still
    let still = cx.reduce_motion();
    if still {
      viewer.update(cx, |v, cx| v.set_animation(Animation::None, cx));
    }
    let animation = choices(&ANIMATIONS, if still { 0 } else { 1 }, window, cx);
    let back = choices(&BACKS, 1, window, cx);
    let accounts = cx.accounts().clone();
    let subs = vec![
      cx.observe(&accounts.accounts, |this, _, cx| this.sync_viewer(cx)),
      cx.observe(&accounts.active, |this, _, cx| this.sync_viewer(cx)),
      cx.observe(&accounts.skins, |_, _, cx| cx.notify()),
      cx.subscribe(&animation, |this, _, e: &SelectEvent<_>, cx| {
        if let SelectEvent::Confirm(Some(label)) = e
          && let Some(a) = pick(&ANIMATIONS, label)
        {
          this.viewer.update(cx, |v, cx| v.set_animation(a, cx));
        }
      }),
      cx.subscribe(&back, |this, _, e: &SelectEvent<_>, cx| {
        if let SelectEvent::Confirm(Some(label)) = e
          && let Some(b) = pick(&BACKS, label)
        {
          this.back_choice = b;
          this.viewer.update(cx, |v, cx| v.set_back(b, cx));
          this.sync_viewer(cx);
        }
      }),
    ];
    let mut this = Self {
      viewer,
      animation,
      back,
      back_choice: BackEquipment::Cape,
      shown: None,
      loading: false,
      adding: false,
      changing: None,
      _subs: subs,
    };
    this.sync_viewer(cx);
    this
  }

  /// Active skin and cape of the active account
  fn wanted(&self, cx: &App) -> Option<Shown> {
    let accounts = cx.accounts();
    let id = accounts.active_id(cx)?;
    let info = accounts.accounts.read(cx).get(id)?.as_ref()?;
    let skin = textures::active_skin(info)?.url.clone();
    let cape = textures::active_cape(info)
      .filter(|_| self.back_choice != BackEquipment::None)
      .map(|c| c.url.clone());
    Some((skin, cape, textures::model(info)))
  }

  fn has_cape(cx: &App) -> bool {
    let accounts = cx.accounts();
    accounts
      .active_id(cx)
      .and_then(|id| accounts.accounts.read(cx).get(id).cloned().flatten())
      .is_some_and(|info| textures::active_cape(&info).is_some())
  }

  fn sync_viewer(&mut self, cx: &mut Context<Self>) {
    cx.notify();
    let wanted = self.wanted(cx);
    if wanted == self.shown {
      return;
    }
    self.shown = wanted.clone();
    let Some((skin, cape, model)) = wanted else {
      self.viewer.update(cx, |v, cx| v.set_skin(None, cx));
      return;
    };
    self.loading = true;
    let handle = cx.accounts().handle.clone();
    let load = cx.background_spawn(load_textures(handle, SkinSrc::Url(skin), cape));
    let expected = self.shown.clone();
    cx.spawn(async move |this, cx| {
      let textures = load.await.log_err().ok();
      this
        .update(cx, |this, cx| {
          // a newer skin was requested meanwhile
          if this.shown != expected {
            return;
          }
          this.loading = false;
          this.viewer.update(cx, |v, cx| {
            v.set_model(model, cx);
            v.set_skin(textures, cx);
          });
          cx.notify();
        })
        .ok();
    })
    .detach();
  }

  fn add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let paths = cx.prompt_for_paths(PathPromptOptions {
      files: true,
      directories: false,
      multiple: false,
      prompt: Some("Add skin".into()),
    });
    cx.spawn_in(window, async move |this, cx| {
      let Ok(Ok(Some(paths))) = paths.await else {
        return;
      };
      let Some(path) = paths.into_iter().next() else {
        return;
      };
      let Ok(task) = this.update(cx, |this, cx| {
        this.adding = true;
        cx.notify();
        let accounts = cx.accounts().handle.clone();
        cx.background_spawn(async move {
          let png = std::fs::read(&path)?;
          accounts_add(accounts, png).await
        })
      }) else {
        return;
      };
      let res = task.await;
      this
        .update_in(cx, |this, window, cx| {
          this.adding = false;
          cx.notify();
          match res {
            Ok(()) => toast(
              window,
              cx,
              NotificationType::Success,
              "Successfully added Skin",
            ),
            Err(_) => toast(window, cx, NotificationType::Error, "Failed to add Skin"),
          }
        })
        .ok();
    })
    .detach();
  }

  fn change(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
    if cx.accounts().active_id(cx).is_none()
      || self.changing.is_some()
      || offline_guard("change Skin", window, cx)
    {
      return;
    }
    self.changing = Some(id.clone());
    cx.notify();
    let task = cx.accounts().change_skin(id);
    cx.spawn_in(window, async move |this, cx| {
      let res = task.await;
      this
        .update_in(cx, |this, window, cx| {
          this.changing = None;
          cx.notify();
          match res {
            Ok(()) => toast(
              window,
              cx,
              NotificationType::Success,
              "Successfully changed Skin",
            ),
            Err(_) => toast(window, cx, NotificationType::Error, "Failed to change Skin"),
          }
        })
        .ok();
    })
    .detach();
  }
}

/// `Accounts` holds entities, which stay on the gpui thread; the handle goes to tokio
async fn accounts_add(
  accounts: proton_account::AccountsHandle,
  png: Vec<u8>,
) -> anyhow::Result<()> {
  proton_utils::rt::spawn(async move { accounts.add_skin(png).await.map(|_| ()) }).await?
}

fn remove_skin(id: String, window: &mut Window, cx: &mut App) {
  let task = cx.accounts().remove_skin(id);
  window
    .spawn(cx, async move |cx| {
      let (kind, message) = match task.await {
        Ok(()) => (NotificationType::Success, "Successfully removed Skin"),
        Err(_) => (NotificationType::Error, "Failed to remove Skin"),
      };
      cx.update(|window, cx| toast(window, cx, kind, message))
        .ok();
    })
    .detach();
}

/// A `MiniSkinViewer` tile: the whole tile wears the skin, a hover trash removes it
pub(super) struct Tile {
  pub id: ElementId,
  pub label: &'static str,
  /// `None` while loading
  pub textures: Option<Arc<SkinTextures>>,
  pub model: ModelType,
  pub selected: bool,
  pub busy: bool,
  pub height: Pixels,
  pub flipped: bool,
  pub on_click: Callback,
  pub on_remove: Option<Callback>,
}

impl Tile {
  pub fn render(self, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let input = theme.input;
    let (selected, on_click) = (self.selected, self.on_click);
    let viewer = match self.textures {
      Some(t) => MiniSkinViewer::new(t)
        .model(self.model)
        .flipped(self.flipped)
        .size(size(self.height - px(10.), self.height))
        .into_any_element(),
      None => Spinner::new().large().into_any_element(),
    };
    let group: SharedString = format!("tile-{:?}", self.id).into();
    div()
      .relative()
      .group(group.clone())
      .child(
        div()
          .id(self.id.clone())
          .flex()
          .flex_col()
          .p_2()
          .rounded_lg()
          .border_1()
          .border_color(if selected { theme.primary } else { input })
          .bg(input.opacity(0.3))
          .hover(move |s| s.bg(input.opacity(0.6)))
          .cursor_pointer()
          .on_click(move |_, window, cx| {
            if !selected {
              on_click(window, cx)
            }
          })
          .child(
            div()
              .relative()
              .flex()
              .items_center()
              .justify_center()
              .w_full()
              .h(self.height)
              .overflow_hidden()
              .rounded_lg()
              .bg(theme.muted)
              .child(viewer)
              .when(self.busy, |d| {
                d.child(
                  div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(Spinner::new().large()),
                )
              }),
          )
          .test_support(),
      )
      .when_some(self.on_remove.filter(|_| !selected), |d, remove| {
        d.child(
          div()
            .absolute()
            .top_3()
            .right_3()
            .opacity(0.)
            .group_hover(group, |s| s.opacity(1.))
            .child(
              Button::new(ElementId::NamedChild(Arc::new(self.id), "remove".into()))
                .ghost()
                .small()
                .icon(Icon::new(IconName::Trash).text_color(theme.danger))
                .tooltip(format!("Remove {}", self.label.to_lowercase()))
                .on_click(move |_, window, cx| remove(window, cx)),
            ),
        )
      })
      .into_any_element()
  }
}

/// `label` + full width select
fn select_field(
  id: &'static str,
  label: &'static str,
  state: &Entity<Choices>,
  disabled: bool,
) -> AnyElement {
  div()
    .flex()
    .flex_col()
    .gap(px(6.))
    .child(
      div()
        .text_sm()
        .line_height(px(14.))
        .font_weight(FontWeight::MEDIUM)
        .child(label),
    )
    .child(Select::new(state).id(id).w_full().disabled(disabled))
    .into_any_element()
}

impl Render for SkinsPage {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let theme = cx.theme();
    let muted_fg = theme.muted_foreground;
    let skins = cx.accounts().skins.read(cx).clone();
    let worn = self.wanted(cx).map(|(skin, ..)| skin);
    let has_cape = Self::has_cape(cx);

    let viewer_card = ring_card(cx)
      .flex()
      .flex_col()
      .flex_none()
      .gap_3()
      .w(px(240.))
      .p(px(14.))
      .child(
        div()
          .relative()
          .flex()
          .justify_center()
          .h(px(280.))
          .overflow_hidden()
          .rounded_lg()
          .bg(theme.muted)
          .child(self.viewer.clone())
          .when(self.loading, |d| {
            d.child(
              div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .child(Spinner::new().large()),
            )
          }),
      )
      .child(select_field(
        "skin-animation",
        "Animation",
        &self.animation,
        false,
      ))
      .child(select_field(
        "skin-back",
        "Back equipment",
        &self.back,
        !has_cape,
      ));

    let full = content_width(window, true);
    let column = if full - 260. >= 320. {
      full - 260.
    } else {
      full
    };
    let cols = auto_fill_cols(column, 120.);

    let this = cx.entity().downgrade();
    let tiles: Vec<AnyElement> = skins
      .iter()
      .map(|skin| {
        let src = (SkinSrc::File(skin.data.clone()), None);
        let textures = window.use_asset::<Textures>(&src, cx).flatten();
        let (change_id, remove_id) = (skin.id.clone(), skin.id.clone());
        let this = this.clone();
        Tile {
          id: ElementId::Name(format!("skin-{}", skin.id).into()),
          label: "Skin",
          textures,
          model: ModelType::Classic,
          selected: worn.is_some() && skin.url == worn,
          busy: self.changing.as_deref() == Some(skin.id.as_str()),
          height: px(110.),
          flipped: false,
          on_click: Rc::new(move |window, cx| {
            let id = change_id.clone();
            this.update(cx, |p, cx| p.change(id, window, cx)).ok();
          }),
          on_remove: Some(Rc::new(move |window, cx| {
            remove_skin(remove_id.clone(), window, cx)
          })),
        }
        .render(cx)
      })
      .collect();
    let library = if tiles.is_empty() {
      div()
        .text_color(muted_fg)
        .child("No skins available")
        .into_any_element()
    } else {
      div()
        .grid()
        .grid_cols(cols)
        .gap_2()
        .children(tiles)
        .into_any_element()
    };

    let column = div()
      .flex()
      .flex_col()
      .flex_1()
      .min_w(px(320.))
      .gap_3()
      .child(
        div()
          .flex()
          .items_center()
          .gap_2()
          .child(heading(
            "Skins",
            Some("Click a skin to wear it.".into()),
            cx,
          ))
          .child(
            Button::new("add-skin")
              .primary()
              .label("Add skin")
              .loading(self.adding)
              .disabled(self.adding)
              .on_click(cx.listener(|this, _, window, cx| this.add(window, cx))),
          ),
      )
      .child(library);

    page(
      "skins-page",
      px(12.),
      [div()
        .flex()
        .flex_wrap()
        .items_start()
        .gap_5()
        .child(viewer_card)
        .child(column)
        .into_any_element()],
    )
  }
}
