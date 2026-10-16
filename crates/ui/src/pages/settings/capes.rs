//! `/settings/capes`: pick a cape of the active account (or none)

use std::rc::Rc;

use gpui_kit::{
  AnyElement, Context, ElementId, IntoElement, ParentElement, Render, Styled, Window,
  component::{ActiveTheme, Sizable, notification::NotificationType, spinner::Spinner},
  div, px,
};
use proton_account::{AccountsExt, store::REMOVE_CAPE};

use super::{
  auto_fill_cols, content_width, heading, offline_guard, page,
  skins::Tile,
  textures::{self, SkinSrc, Textures},
  toast,
};

pub struct CapesPage {
  /// cape id being selected right now
  changing: Option<String>,
}

impl CapesPage {
  pub fn new(cx: &mut Context<Self>) -> Self {
    let accounts = cx.accounts().clone();
    cx.observe(&accounts.accounts, |_, _, cx| cx.notify()).detach();
    cx.observe(&accounts.active, |_, _, cx| cx.notify()).detach();
    Self { changing: None }
  }

  fn change(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
    if cx.accounts().active_id(cx).is_none()
      || self.changing.is_some()
      || offline_guard("change Cape", window, cx)
    {
      return;
    }
    self.changing = Some(id.clone());
    cx.notify();
    let task = cx.accounts().change_cape(id);
    cx.spawn_in(window, async move |this, cx| {
      let res = task.await;
      this
        .update_in(cx, |this, window, cx| {
          this.changing = None;
          cx.notify();
          match res {
            Ok(()) => toast(window, cx, NotificationType::Success, "Successfully changed Cape"),
            Err(_) => toast(window, cx, NotificationType::Error, "Failed to change Cape"),
          }
        })
        .ok();
    })
    .detach();
  }
}

fn loader() -> AnyElement {
  div()
    .flex()
    .h(px(148.))
    .items_center()
    .justify_center()
    .child(Spinner::new().large())
    .into_any_element()
}

impl Render for CapesPage {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let accounts = cx.accounts();
    let info = accounts
      .active_id(cx)
      .and_then(|id| accounts.accounts.read(cx).get(id).cloned().flatten());
    let muted = cx.theme().muted_foreground;
    let owner = info.as_ref().map_or("this account".to_string(), |i| i.name.clone());
    let header = heading(
      "Capes",
      Some(format!("Capes owned by {owner}. Choose none to hide your cape.").into()),
      cx,
    )
    .into_any_element();

    let Some(info) = info else {
      return page(
        "capes-page",
        [
          header,
          div()
            .text_color(muted)
            .child("No capes available")
            .into_any_element(),
        ],
      );
    };
    let Some(skin) = textures::active_skin(&info).map(|s| s.url.clone()) else {
      return page("capes-page", [header, loader()]);
    };
    // the bare skin decides when the grid shows, like the old page waited for it
    let bare = (SkinSrc::Url(skin.clone()), None);
    if window.use_asset::<Textures>(&bare, cx).flatten().is_none() {
      return page("capes-page", [header, loader()]);
    }

    let active = textures::active_cape(&info).map(|c| c.url.clone());
    let model = textures::model(&info);
    let this = cx.entity().downgrade();
    let mut tiles = vec![];
    let options = std::iter::once((REMOVE_CAPE.to_string(), None, "No cape"))
      .chain(info.capes.iter().map(|c| (c.id.clone(), Some(c.url.clone()), "Cape")));
    for (id, cape, label) in options {
      let src = (SkinSrc::Url(skin.clone()), cape.clone());
      let textures = window.use_asset::<Textures>(&src, cx).flatten();
      let this = this.clone();
      let change_id = id.clone();
      tiles.push(
        Tile {
          id: ElementId::Name(format!("cape-{id}").into()),
          label,
          textures,
          model,
          selected: cape == active,
          busy: self.changing.as_deref() == Some(id.as_str()),
          height: px(130.),
          flipped: true,
          on_click: Rc::new(move |window, cx| {
            let id = change_id.clone();
            this.update(cx, |p, cx| p.change(id, window, cx)).ok();
          }),
          on_remove: None,
        }
        .render(cx),
      );
    }
    let cols = auto_fill_cols(content_width(window, false), 140.);
    page(
      "capes-page",
      [
        header,
        div()
          .grid()
          .grid_cols(cols)
          .gap_2()
          .children(tiles)
          .into_any_element(),
      ],
    )
  }
}
