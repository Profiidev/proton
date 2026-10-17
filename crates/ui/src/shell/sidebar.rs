use std::{cell::RefCell, rc::Rc, time::Duration};

use gpui_kit::{
  AnyView, App, Bounds, Context, Div, Entity, InteractiveElement, IntoElement, ParentElement,
  Pixels, Render, SharedString, StatefulInteractiveElement, Styled, Window,
  assets::IconName,
  base::{
    TestSupportExt,
    motion::{Transition, transition},
  },
  canvas,
  component::{ActiveTheme, Icon, Sizable, StyledExt},
  div, point,
  prelude::FluentBuilder,
  px, size,
};

use proton_components::widgets::colors;

use crate::{
  router::{Kind, Route, RouterExt},
  shell::{PageFactory, ShellData, ShellDataExt},
};

/// Old crossfade duration
const SLIDE: Duration = Duration::from_millis(250);

/// Below this window width the sidebar shows icons only (Tailwind `md`)
pub const COMPACT_BELOW: Pixels = px(768.);

pub fn sidebar_width(window: &Window) -> Pixels {
  if window.viewport_size().width < COMPACT_BELOW {
    px(64.)
  } else {
    px(216.)
  }
}

/// Measured bounds: `[pill layer, row 0, row 1, ..]`
type Rows = Rc<RefCell<Vec<Option<Bounds<Pixels>>>>>;

/// The `bg-muted` pill behind the active row that slides between rows.
/// Rows are measured during prepaint; until then the active row paints its own background.
#[derive(Default)]
struct Indicator {
  /// bumped when the pill hides, so it reappears in place instead of sliding
  generation: usize,
  shown: bool,
  rows: Rows,
}

impl Indicator {
  /// Where to draw the pill this frame (relative to the nav), `None` while unmeasured
  fn frame<T: 'static>(
    &mut self,
    active: Option<usize>,
    count: usize,
    window: &mut Window,
    cx: &mut Context<T>,
  ) -> Option<Bounds<Pixels>> {
    let measured = {
      let mut rows = self.rows.borrow_mut();
      rows.resize(count + 1, None);
      rows[0].zip(active.and_then(|i| rows[i + 1]))
    };
    let Some((nav, row)) = measured else {
      self.hide();
      return None;
    };
    self.shown = true;
    let id = SharedString::from(format!("pill-{}-{}", cx.entity_id(), self.generation));
    let y = transition(
      (id.clone(), "y"),
      row.origin.y - nav.origin.y,
      Transition::new(SLIDE),
      window,
      cx,
    );
    let h = transition(
      (id, "h"),
      row.size.height,
      Transition::new(SLIDE),
      window,
      cx,
    );
    Some(Bounds::new(
      point(row.origin.x - nav.origin.x, y),
      size(row.size.width, h),
    ))
  }

  fn hide(&mut self) {
    if self.shown {
      self.shown = false;
      self.generation += 1;
    }
  }

  /// Records the bounds of `row` (`None` = the pill layer), re-rendering the view when they moved
  fn measure<T: 'static, E: ParentElement>(&self, el: E, row: Option<usize>, cx: &Context<T>) -> E {
    let rows = self.rows.clone();
    let view = cx.entity().downgrade();
    let slot = row.map_or(0, |i| i + 1);
    // not `ElementExt::on_prepaint`: its canvas sits at the content box, inside the padding
    let probe = canvas(
      move |bounds, _, cx| {
        let moved = match rows.borrow_mut().get_mut(slot) {
          Some(s) if *s != Some(bounds) => {
            *s = Some(bounds);
            true
          }
          _ => false,
        };
        if moved {
          view.update(cx, |_, cx| cx.notify()).ok();
        }
      },
      |_, _, _, _| {},
    );
    el.child(probe.absolute().top_0().left_0().size_full())
  }

  /// The nav-sized layer holding the pill; it is also what row bounds are measured against
  fn layer<T: 'static>(&self, pill: Option<Bounds<Pixels>>, cx: &Context<T>) -> Div {
    let pill = pill.map(|b| {
      div()
        .absolute()
        .left(b.origin.x)
        .top(b.origin.y)
        .w(b.size.width)
        .h(b.size.height)
        .rounded(px(8.))
        .bg(cx.theme().muted)
    });
    let layer = div().absolute().top_0().left_0().size_full().children(pill);
    self.measure(layer, None, cx)
  }
}

/// A nav row: highlighted when active (or by the unmeasured fallback)
fn row(
  id: impl Into<SharedString>,
  active: bool,
  fallback_bg: bool,
  cx: &App,
) -> gpui_kit::base::ObservedElement<gpui_kit::Stateful<Div>> {
  let theme = cx.theme();
  let id: SharedString = id.into();
  div()
    .id(id)
    .test_support()
    .relative()
    .flex()
    .flex_shrink_0()
    .items_center()
    .rounded(px(8.))
    .px(px(10.))
    .text_sm()
    .font_medium()
    .whitespace_nowrap()
    .cursor_pointer()
    .map(|el| {
      if active {
        el.text_color(theme.foreground)
          .when(fallback_bg, |el| el.bg(theme.muted))
      } else {
        let hover = theme.accent.opacity(0.6);
        let fg = theme.foreground;
        el.text_color(theme.muted_foreground)
          .hover(move |s| s.bg(hover).text_color(fg))
      }
    })
}

struct Item {
  title: &'static str,
  icon: IconName,
  route: Route,
}

fn items() -> [Item; 8] {
  let item = |title, icon, route| Item { title, icon, route };
  [
    item("Home", IconName::House, Route::Home),
    item("Profiles", IconName::LibraryBig, Route::Profiles),
    item("Instances", IconName::Gamepad2, Route::Instances),
    item("Mods", IconName::Puzzle, Route::Discover(Kind::Mod)),
    item(
      "Modpacks",
      IconName::Package,
      Route::Discover(Kind::Modpack),
    ),
    item(
      "Resource packs",
      IconName::Image,
      Route::Discover(Kind::ResourcePack),
    ),
    item("Shaders", IconName::Sparkles, Route::Discover(Kind::Shader)),
    item(
      "Settings",
      IconName::Settings,
      Route::Settings(crate::router::SettingsTab::Accounts),
    ),
  ]
}

/// Index into [`items`] of the item that owns `route`
fn active_item(route: &Route) -> usize {
  let kind = |k: &Kind| match k {
    Kind::Mod => 3,
    Kind::Modpack => 4,
    Kind::ResourcePack => 5,
    Kind::Shader => 6,
  };
  match route {
    Route::Home => 0,
    Route::Profiles | Route::Profile { .. } => 1,
    Route::Instances | Route::InstanceLogs { .. } => 2,
    Route::Discover(k) | Route::Project { kind: k, .. } => kind(k),
    Route::Settings(_) => 7,
  }
}

/// The main left nav
pub struct Sidebar {
  active: usize,
  running: usize,
  footer: Option<AnyView>,
  indicator: Indicator,
}

impl Sidebar {
  pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
    let router = cx.router();
    cx.observe(&router, |this, router, cx| {
      let active = active_item(router.read(cx).current());
      if active != this.active {
        this.active = active;
        cx.notify();
      }
    })
    .detach();
    cx.observe(&cx.shell_data(), |this, data, cx| {
      let running = data.read(cx).running;
      if running != this.running {
        this.running = running;
        cx.notify();
      }
    })
    .detach();
    let footer = cx
      .try_global::<PageFactory>()
      .and_then(|f| f.sidebar_footer.clone())
      .map(|build| build(window, cx));
    Self {
      active: active_item(&cx.route()),
      running: cx.shell_data().read(cx).running,
      footer,
      indicator: Indicator::default(),
    }
  }
}

impl Render for Sidebar {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    proton_components::render_count::hit::<Self>();
    let compact = window.viewport_size().width < COMPACT_BELOW;
    let items = items();
    let pill = self
      .indicator
      .frame(Some(self.active), items.len(), window, cx);
    let theme = cx.theme();
    let separator = || {
      div()
        .my_1()
        .h_px()
        .w_full()
        .flex_shrink_0()
        .bg(theme.border)
    };

    let mut rows = items.into_iter().enumerate().map(|(ix, item)| {
      let active = ix == self.active;
      let el = row(format!("nav-{}", item.title), active, pill.is_none(), cx)
        .h_10()
        .gap_3()
        .map(|el| {
          if compact {
            el.w_10().self_center().justify_center().px_0()
          } else {
            el.w_full()
          }
        })
        .child(Icon::new(item.icon).size_5().flex_shrink_0())
        .when(!compact, |el| {
          el.child(div().truncate().child(item.title))
            .when(ix == 2 && self.running > 0, |el| {
              el.child(running_pill(self.running))
            })
        })
        .on_click(move |_, _, cx| cx.navigate(item.route.clone()));
      self.indicator.measure(el, Some(ix), cx)
    });

    div()
      .id("sidebar")
      .relative()
      .flex()
      .flex_col()
      .size_full()
      .p_2()
      .gap(px(2.))
      .child(self.indicator.layer(pill, cx))
      .children(rows.by_ref().take(3))
      .map(|el| {
        if compact {
          el.child(separator())
        } else {
          el.child(
            div()
              .mt_3()
              .mb_1()
              .px(px(10.))
              .text_xs()
              .line_height(px(16.))
              .font_medium()
              .text_color(theme.muted_foreground)
              .child("Discover"),
          )
        }
      })
      .children(rows.by_ref().take(4))
      .child(div().flex_1().min_h_4())
      .children(rows)
      .child(separator())
      .children(self.footer.clone())
  }
}

fn running_pill(count: usize) -> Div {
  let green = colors::green_500();
  div()
    .ml_auto()
    .flex()
    .items_center()
    .gap_1()
    .h_5()
    .px_2()
    .rounded_full()
    .bg(green.opacity(0.15))
    .text_xs()
    .font_medium()
    .text_color(green)
    .child(div().size(px(6.)).rounded_full().bg(green))
    .child(count.to_string())
}

#[derive(Debug, Clone, PartialEq)]
pub struct NavItem {
  pub title: SharedString,
  pub icon: Option<IconName>,
  pub route: Route,
  /// Amber count chip, hidden when 0
  pub badge: usize,
}

/// The nav of nested sections (settings, profile tabs)
#[derive(Default)]
pub struct SimpleSidebar {
  items: Vec<NavItem>,
  active: Option<usize>,
  indicator: Indicator,
}

impl SimpleSidebar {
  /// Notifies only when something changed. Without an active item the next
  /// appearance starts without sliding.
  pub fn set(&mut self, items: Vec<NavItem>, active: Option<usize>, cx: &mut Context<Self>) {
    if active.is_none() {
      self.indicator.hide();
    }
    if items != self.items || active != self.active {
      self.items = items;
      self.active = active;
      cx.notify();
    }
  }
}

impl Render for SimpleSidebar {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let pill = self
      .indicator
      .frame(self.active, self.items.len(), window, cx);
    let amber = colors::amber_500();
    let rows: Vec<_> = self
      .items
      .iter()
      .enumerate()
      .map(|(ix, item)| {
        let route = item.route.clone();
        let el = row(
          format!("tab-{}", item.title),
          self.active == Some(ix),
          pill.is_none(),
          cx,
        )
        .h_8()
        .gap_2()
        .children(item.icon.map(|i| Icon::new(i).small().flex_shrink_0()))
        .child(div().child(item.title.clone()))
        .when(item.badge > 0, |el| {
          el.child(
            div()
              .ml_auto()
              .flex()
              .items_center()
              .h_5()
              .px(px(6.))
              .rounded_full()
              .bg(amber.opacity(0.15))
              .text_xs()
              .font_medium()
              .text_color(amber)
              .child(item.badge.to_string()),
          )
        })
        .on_click(move |_, _, cx| cx.navigate(route.clone()));
        self.indicator.measure(el, Some(ix), cx)
      })
      .collect();
    div()
      .relative()
      .flex()
      .flex_col()
      .size_full()
      .gap(px(2.))
      .child(self.indicator.layer(pill, cx))
      .children(rows)
  }
}

/// Keeps the nested navs in sync with the route and shell data
pub(crate) fn sync_navs(
  settings: &Entity<SimpleSidebar>,
  profile: &Entity<SimpleSidebar>,
  route: &Route,
  data: &ShellData,
  cx: &mut App,
) {
  use crate::router::{ProfileSettingsTab, ProfileTab, QuickPlayTab, SettingsTab};

  let item = |title: &'static str, icon, route, badge| NavItem {
    title: title.into(),
    icon,
    route,
    badge,
  };

  let tabs = [
    ("Accounts", SettingsTab::Accounts),
    ("Skins", SettingsTab::Skins),
    ("Capes", SettingsTab::Capes),
    ("Minecraft", SettingsTab::Minecraft),
    ("Java", SettingsTab::Java),
    ("About", SettingsTab::About),
  ];
  let active = match route {
    Route::Settings(tab) => tabs.iter().position(|(_, t)| t == tab),
    _ => None,
  };
  let items = tabs
    .iter()
    .map(|(title, tab)| item(title, None, Route::Settings(*tab), 0))
    .collect();
  settings.update(cx, |nav, cx| nav.set(items, active, cx));

  let (items, active) = match route {
    Route::Profile { id, tab } => {
      let to = |tab| Route::Profile {
        id: id.clone(),
        tab,
      };
      let updates = data.content_updates.get(id).copied().unwrap_or(0);
      let items = vec![
        item(
          "Quick play",
          Some(IconName::FastForward),
          to(ProfileTab::QuickPlay(QuickPlayTab::Singleplayer)),
          0,
        ),
        item(
          "Content",
          Some(IconName::Package),
          to(ProfileTab::Content(Kind::Mod)),
          updates,
        ),
        item(
          "Instances",
          Some(IconName::Monitor),
          to(ProfileTab::Instances),
          0,
        ),
        item("Logs", Some(IconName::Terminal), to(ProfileTab::Logs), 0),
        item(
          "Settings",
          Some(IconName::SlidersHorizontal),
          to(ProfileTab::Settings(ProfileSettingsTab::General)),
          0,
        ),
      ];
      let active = match tab {
        ProfileTab::QuickPlay(_) => 0,
        ProfileTab::Content(_) => 1,
        ProfileTab::Instances => 2,
        ProfileTab::Logs => 3,
        ProfileTab::Settings(_) => 4,
      };
      (items, Some(active))
    }
    _ => (vec![], None),
  };
  profile.update(cx, |nav, cx| nav.set(items, active, cx));
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn every_route_has_its_sidebar_item() {
    let items = items();
    for (ix, item) in items.iter().enumerate() {
      assert_eq!(active_item(&item.route), ix, "{}", item.title);
    }
    let project = Route::Project {
      id: "x".into(),
      kind: Kind::Shader,
      source: crate::router::Source::Modrinth,
    };
    assert_eq!(items[active_item(&project)].title, "Shaders");
    let logs = Route::InstanceLogs { id: "i".into() };
    assert_eq!(items[active_item(&logs)].title, "Instances");
  }
}
