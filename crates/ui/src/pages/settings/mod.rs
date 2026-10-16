//! `/settings/*`: accounts, skins, capes, Minecraft, Java, about, plus the sidebar account
//! selector. The reusable Minecraft/Java forms live in [`forms`].

mod about;
mod account_selector;
mod accounts;
mod capes;
pub mod forms;
mod minecraft;
mod skins;
mod textures;

#[cfg(test)]
mod tests;

pub use about::AboutPage;
pub use account_selector::AccountSelector;
pub use accounts::AccountsPage;
pub use capes::CapesPage;
pub use minecraft::{JavaPage, MinecraftPage};
pub use skins::SkinsPage;

use gpui_kit::{
  AnyElement, App, AppContext, Div, ElementId, FontWeight, InteractiveElement, IntoElement,
  ParentElement, Pixels, SharedString, StatefulInteractiveElement, Styled, Window,
  component::{ActiveTheme, WindowExt, notification::NotificationType},
  div, px,
};
use proton_config::OfflineExt;

use crate::{
  router::{Route, SettingsTab},
  shell::PageFactory,
};

/// Registers the settings routes and the sidebar account selector
pub fn register(cx: &mut App) {
  PageFactory::register(
    cx,
    |route| matches!(route, Route::Settings(_)),
    |route, window, cx| {
      let Route::Settings(tab) = route else {
        unreachable!("only settings routes match")
      };
      match tab {
        SettingsTab::Accounts => cx.new(|cx| AccountsPage::new(window, cx)).into(),
        SettingsTab::Skins => cx.new(|cx| SkinsPage::new(window, cx)).into(),
        SettingsTab::Capes => cx.new(CapesPage::new).into(),
        SettingsTab::Minecraft => cx.new(|cx| MinecraftPage::new(window, cx)).into(),
        SettingsTab::Java => cx.new(|cx| JavaPage::new(window, cx)).into(),
        SettingsTab::About => cx.new(AboutPage::new).into(),
      }
    },
  );
  PageFactory::set_sidebar_footer(cx, |_, cx| cx.new(AccountSelector::new).into());
}

/// Page root: fills the content area and scrolls itself
fn page(
  id: impl Into<ElementId>,
  gap: Pixels,
  children: impl IntoIterator<Item = AnyElement>,
) -> impl IntoElement {
  div()
    .id(id)
    .size_full()
    .overflow_y_scroll()
    .child(div().flex().flex_col().gap(gap).w_full().children(children))
}

/// H1 + muted subtitle
fn heading(title: &'static str, subtitle: Option<SharedString>, cx: &App) -> Div {
  div()
    .flex()
    .flex_col()
    .flex_1()
    .min_w_0()
    .child(
      div()
        .text_xl()
        .font_weight(FontWeight::SEMIBOLD)
        .line_height(px(28.))
        .child(title),
    )
    .children(subtitle.map(|s| {
      div()
        // mt-0.5 in the reference, gpui sets the glyphs 1px lower in the line box
        .mt(px(1.))
        .line_height(px(24.))
        .text_color(cx.theme().muted_foreground)
        .child(s)
    }))
}

/// "Ring card": `bg-card ring-1 ring-foreground/10 rounded-xl`. A ring is drawn outside the box
/// and takes no space, the -1px margin does the same for the border. Fixed sizes add 2px.
fn ring_card(cx: &App) -> Div {
  let theme = cx.theme();
  div()
    .m(px(-1.))
    .rounded(px(14.))
    .border_1()
    .border_color(theme.foreground.opacity(0.1))
    .bg(theme.popover)
}

fn toast(
  window: &mut Window,
  cx: &mut App,
  kind: NotificationType,
  message: impl Into<SharedString>,
) {
  window.push_notification((kind, message.into()), cx);
}

/// Warns and returns true while offline, `what` ends "please reconnect to the internet to ..."
fn offline_guard(what: &str, window: &mut Window, cx: &mut App) -> bool {
  let offline = cx.offline().is_offline(cx);
  if offline {
    toast(
      window,
      cx,
      NotificationType::Warning,
      format!("You are currently offline, please reconnect to the internet to {what}"),
    );
  }
  offline
}

/// `grid-cols-[repeat(auto-fill,minmax(min,1fr))]` with `gap-2` for a container `width` wide
fn auto_fill_cols(width: f32, min: f32) -> u16 {
  (((width + 8.) / (min + 8.)).floor() as u16).max(1)
}

/// Width of the settings page column (the shell's 720px max applies unless `wide`)
fn content_width(window: &Window, wide: bool) -> f32 {
  // shell: sidebar | 1px | px-5 + settings nav (w-40) + gap-5 + page
  let free = f32::from(window.viewport_size().width - crate::shell::sidebar_width(window))
    - 1.
    - 40.
    - 160.
    - 20.;
  if wide { free } else { free.min(720.) }
}
