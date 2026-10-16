//! `/profiles`: searchable, filterable grid of all profiles plus the create dialog.
//! Also the profile tile helpers the home page shares.

pub mod create;
pub mod meta;

use std::{cmp::Ordering, rc::Rc, time::Duration};

use chrono::Utc;
use gpui_kit::{
  Animation, AnimationExt, AnyElement, App, AppContext, Context, ElementId, Entity, FontWeight,
  InteractiveElement, IntoElement, ParentElement, Pixels, Render, SharedString,
  StatefulInteractiveElement, Styled, Subscription, Task, TestSupportExt, Window,
  assets::IconName,
  component::{
    ActiveTheme, Icon,
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
    popover::Popover,
  },
  div,
  prelude::FluentBuilder,
  px,
};
use proton_components::{
  animation::ease_out_cubic,
  widgets::{format::relative_time, profile::ProfileCard},
};
use proton_profiles::{
  ProfilesExt,
  config::{Profile, QuickPlayInfo, QuickPlayType},
};
use proton_versions::{VersionsExt, loader::LoaderType};

pub use create::{CreateTab, TabBuilder, set_create_tab};
pub use meta::{ProfileMeta, profile_meta};

use crate::{
  actions::{launch, toast_error},
  router::{ProfileTab, QuickPlayTab, Route, RouterExt},
  shell::{PageFactory, sidebar_width},
};

const CARD_MIN: f32 = 264.;
const GAP: f32 = 8.;
const FADE: Duration = Duration::from_millis(200);

pub fn register(cx: &mut App) {
  PageFactory::register(
    cx,
    |r| *r == Route::Profiles,
    |_, window, cx| cx.new(|cx| ProfilesPage::new(window, cx)).into(),
  );
}

// ---- helpers shared with the home page ----

/// `LoaderType` as the web app printed it
pub fn loader_name(loader: LoaderType) -> &'static str {
  match loader {
    LoaderType::Vanilla => "Vanilla",
    LoaderType::Fabric => "Fabric",
    LoaderType::Quilt => "Quilt",
    LoaderType::Forge => "Forge",
    LoaderType::NeoForge => "NeoForge",
  }
}

pub fn plural(n: usize, word: &str) -> String {
  format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

/// `compareProfiles`: played first by last played desc, then never played by created desc
pub fn compare_profiles(a: &Profile, b: &Profile) -> Ordering {
  match (a.last_played, b.last_played) {
    (Some(a), Some(b)) => b.cmp(&a),
    (Some(_), None) => Ordering::Less,
    (None, Some(_)) => Ordering::Greater,
    (None, None) => b.created_at.cmp(&a.created_at),
  }
}

/// `card_meta`: `{profile name | n mods} · played {relative}` / `never played`
pub fn card_meta(
  profile: &Profile,
  quick_play: Option<&QuickPlayInfo>,
  mods: Option<usize>,
) -> String {
  let mut parts = vec![];
  match (quick_play, mods) {
    (Some(_), _) => parts.push(profile.name.clone()),
    (None, Some(n)) if profile.loader != LoaderType::Vanilla => parts.push(plural(n, "mod")),
    _ => {}
  }
  let played = match quick_play {
    Some(qp) => Some(qp.last_played_time),
    None => profile.last_played,
  };
  parts.push(match played {
    Some(t) => format!("played {}", relative_time(t, Utc::now())),
    None => "never played".into(),
  });
  parts.join(" · ")
}

pub fn quick_play_tab(quick_play: Option<&QuickPlayInfo>) -> QuickPlayTab {
  match quick_play.map(|q| &q.r#type) {
    Some(QuickPlayType::Multiplayer) => QuickPlayTab::Multiplayer,
    Some(QuickPlayType::Realms) => QuickPlayTab::Realms,
    _ => QuickPlayTab::Singleplayer,
  }
}

pub fn open_profile(id: String, quick_play: Option<&QuickPlayInfo>, cx: &mut App) {
  let tab = ProfileTab::QuickPlay(quick_play_tab(quick_play));
  cx.navigate(Route::Profile { id, tab });
}

pub fn running(profile: &str, cx: &App) -> usize {
  cx.profiles()
    .instances
    .read(cx)
    .iter()
    .filter(|i| i.profile_id == profile)
    .count()
}

/// Tile of a profile or one of its quick play entries; only profiles show running state
pub fn profile_card(
  id: impl Into<ElementId>,
  profile: &Profile,
  quick_play: Option<&QuickPlayInfo>,
  meta: &ProfileMeta,
  cx: &App,
) -> ProfileCard {
  let running = match quick_play {
    Some(_) => 0,
    None => running(&profile.id, cx),
  };
  let icon = quick_play
    .and_then(|qp| meta.quick_play_icon(&profile.id, qp))
    .or_else(|| meta.icon(&profile.id));
  let (open_id, play_id, stop_id) = (profile.id.clone(), profile.id.clone(), profile.id.clone());
  let (open_qp, play_qp) = (quick_play.cloned(), quick_play.cloned());
  ProfileCard::new(
    id,
    quick_play.map_or(&profile.name, |q| &q.name).clone(),
    format!("{} {}", loader_name(profile.loader), profile.version),
  )
  .meta(card_meta(profile, quick_play, meta.mods(&profile.id)))
  .icon(icon)
  .favorite(quick_play.map_or(profile.favorite, |q| q.favorite))
  .running(running)
  .on_open(move |_, cx| open_profile(open_id.clone(), open_qp.as_ref(), cx))
  .on_play(move |_, cx| launch(play_id.clone(), play_qp.clone(), cx))
  .on_stop(move |_, cx| stop_all(stop_id.clone(), cx))
}

pub fn stop_all(profile: String, cx: &mut App) {
  let stop = cx.profiles().stop_all(Some(profile));
  cx.spawn(async move |cx| {
    if let Err(e) = stop.await {
      cx.update(|cx| toast_error("Failed to stop instances", &e, cx));
    }
  })
  .detach();
}

/// Columns of `repeat(auto-fill, minmax(264px, 1fr))` in the content area (page padding 20px)
pub fn grid_columns(window: &Window) -> u16 {
  let width: Pixels = window.viewport_size().width - sidebar_width(window) - px(41.);
  (((f32::from(width) + GAP) / (CARD_MIN + GAP)).floor() as u16).max(1)
}

pub fn card_grid(
  window: &Window,
  cards: impl IntoIterator<Item = impl IntoElement>,
) -> gpui_kit::Div {
  div()
    .grid()
    .grid_cols(grid_columns(window))
    .gap_2()
    .children(cards)
}

/// Fades a list in once when it first renders; no frames are requested afterwards
pub fn fade_in(id: impl Into<ElementId>, el: impl IntoElement, cx: &App) -> AnyElement {
  if cx.reduce_motion() {
    return el.into_any_element();
  }
  div()
    .child(el)
    .with_animation(
      id,
      Animation::new(FADE).with_easing(ease_out_cubic),
      |el, t| el.opacity(t),
    )
    .into_any_element()
}

/// Every word of the query is in name/loader/version, or (typo tolerance like fuse.js)
/// its letters appear in order in the name
pub fn matches(profile: &Profile, query: &str) -> bool {
  let name = profile.name.to_lowercase();
  let haystack =
    format!("{name} {} {}", loader_name(profile.loader), profile.version).to_lowercase();
  query.to_lowercase().split_whitespace().all(|word| {
    haystack.contains(word) || {
      let mut rest = name.chars();
      word.chars().all(|c| rest.any(|n| n == c))
    }
  })
}

// ---- page ----

pub struct ProfilesPage {
  pub(crate) search: Entity<InputState>,
  pub(crate) loader_filter: Vec<SharedString>,
  pub(crate) version_filter: Vec<SharedString>,
  /// Vanilla versions in manifest order, for the version filter
  vanilla_versions: Vec<String>,
  meta: Entity<ProfileMeta>,
  _versions: Task<()>,
  _subs: [Subscription; 5],
}

impl ProfilesPage {
  pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
    let search = cx.new(|cx| {
      InputState::new(window, cx).placeholder("Search profiles by name, loader or version")
    });
    let meta = profile_meta(cx);
    let profiles = cx.profiles().clone();
    let versions = cx.versions().versions.clone();
    let subs = [
      cx.subscribe(&search, |_, _, event, cx| {
        if matches!(event, InputEvent::Change) {
          cx.notify();
        }
      }),
      cx.observe(&profiles.profiles, |_, _, cx| cx.notify()),
      cx.observe(&profiles.instances, |_, _, cx| cx.notify()),
      cx.observe(&meta, |_, _, cx| cx.notify()),
      // manifests loaded or refreshed
      cx.observe(&versions, |this, _, cx| this.load_versions(cx)),
    ];
    let mut this = Self {
      search,
      loader_filter: vec![],
      version_filter: vec![],
      vanilla_versions: vec![],
      meta,
      _versions: Task::ready(()),
      _subs: subs,
    };
    this.load_versions(cx);
    this
  }

  fn load_versions(&mut self, cx: &mut Context<Self>) {
    let task = cx.versions().clone().version_list(LoaderType::Vanilla, cx);
    self._versions = cx.spawn(async move |this, cx| {
      let list = task.await.unwrap_or_default();
      this
        .update(cx, |this, cx| {
          if this.vanilla_versions != list {
            this.vanilla_versions = list;
            cx.notify();
          }
        })
        .ok();
    });
  }

  /// Profiles passing search and filters, sorted
  pub(crate) fn visible(&self, cx: &App) -> Vec<Profile> {
    let query = self.search.read(cx).value();
    let mut list: Vec<Profile> = cx
      .profiles()
      .profiles
      .read(cx)
      .iter()
      .filter(|p| matches(p, &query))
      .filter(|p| {
        self.loader_filter.is_empty()
          || self
            .loader_filter
            .iter()
            .any(|l| l == loader_name(p.loader))
      })
      .filter(|p| {
        self.version_filter.is_empty() || self.version_filter.iter().any(|v| *v == p.version)
      })
      .cloned()
      .collect();
    list.sort_by(compare_profiles);
    list
  }

  fn toggle(&mut self, loader: bool, value: SharedString, cx: &mut Context<Self>) {
    let list = if loader {
      &mut self.loader_filter
    } else {
      &mut self.version_filter
    };
    match list.iter().position(|v| *v == value) {
      Some(ix) => {
        list.remove(ix);
      }
      None => list.push(value),
    }
    cx.notify();
  }

  fn filter_select(
    &self,
    loader: bool,
    options: Vec<SharedString>,
    cx: &mut Context<Self>,
  ) -> impl IntoElement + use<> {
    let (id, label, selected) = if loader {
      ("loader-filter", "Loader", self.loader_filter.clone())
    } else {
      ("version-filter", "Version", self.version_filter.clone())
    };
    let text: SharedString = if selected.is_empty() {
      label.into()
    } else {
      selected.join(", ").into()
    };
    let page = cx.weak_entity();
    let toggle = Rc::new(move |value: SharedString, cx: &mut App| {
      page
        .update(cx, |this, cx| this.toggle(loader, value, cx))
        .ok();
    });
    Popover::new(id)
      .trigger(Button::new(id).outline().label(text))
      .content(move |_, _, cx| {
        let hover = cx.theme().accent;
        div()
          .flex()
          .flex_col()
          .min_w(px(160.))
          .max_h(px(300.))
          .children(options.iter().enumerate().map(|(ix, option)| {
            let checked = selected.contains(option);
            let (toggle, value) = (toggle.clone(), option.clone());
            div()
              .id(ix)
              .flex()
              .items_center()
              .gap_2()
              .px_2()
              .py_1p5()
              .rounded_md()
              .text_sm()
              .cursor_pointer()
              .hover(move |s| s.bg(hover))
              .on_click(move |_, _, cx| toggle(value.clone(), cx))
              .child(
                div()
                  .size_4()
                  .when(checked, |el| el.child(Icon::new(IconName::Check).size_4())),
              )
              .child(option.clone())
          }))
      })
  }
}

impl Render for ProfilesPage {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let theme = cx.theme();
    let muted_fg = theme.muted_foreground;
    let all = cx.profiles().profiles.read(cx).clone();
    let favorites = all.iter().filter(|p| p.favorite).count();

    let mut loaders: Vec<SharedString> = vec![];
    for p in &all {
      let name = SharedString::from(loader_name(p.loader));
      if !loaders.contains(&name) {
        loaders.push(name);
      }
    }
    let mut versions: Vec<SharedString> = self
      .vanilla_versions
      .iter()
      .filter(|v| all.iter().any(|p| p.version == **v))
      .map(|v| SharedString::from(v.clone()))
      .collect();
    for p in &all {
      let v = SharedString::from(p.version.clone());
      if !versions.contains(&v) {
        versions.push(v);
      }
    }

    let visible = self.visible(cx);
    let meta = self.meta.read(cx);
    let list = if visible.is_empty() {
      div()
        .text_color(muted_fg)
        .text_center()
        .child("No profiles found. Adjust your filters or create a new profile.")
        .into_any_element()
    } else {
      let cards: Vec<_> = visible
        .iter()
        .map(|p| profile_card(SharedString::from(p.id.clone()), p, None, meta, cx))
        .collect();
      fade_in("profiles-grid", card_grid(window, cards), cx)
    };

    let toolbar = div()
      .flex()
      .flex_wrap()
      .items_center()
      .gap_2()
      .child(
        div().flex_1().min_w(px(200.)).child(
          Input::new(&self.search)
            .id("search")
            .prefix(Icon::new(IconName::Search).size_4().text_color(muted_fg))
            .cleanable(true),
        ),
      )
      .child(self.filter_select(true, loaders, cx))
      .child(self.filter_select(false, versions, cx))
      .child(
        Button::new("new-profile")
          .primary()
          .icon(IconName::Plus)
          .label("New profile")
          .on_click(|_, window, cx| create::open(window, cx)),
      );

    div()
      .id("profiles-page")
      .size_full()
      .overflow_y_scroll()
      .child(
        div()
          .flex()
          .flex_col()
          .gap_4()
          .px_5()
          .pt_4()
          .pb_6()
          .child(
            div()
              .flex()
              .flex_wrap()
              .items_baseline()
              .gap(px(10.))
              .child(
                div()
                  .text_xl()
                  .line_height(px(28.))
                  .font_weight(FontWeight::SEMIBOLD)
                  .child("Profiles"),
              )
              .child(div().text_sm().text_color(muted_fg).child(format!(
                "{} · {}",
                plural(all.len(), "profile"),
                plural(favorites, "favorite")
              ))),
          )
          .child(toolbar)
          .child(list),
      )
      .test_support()
  }
}

#[cfg(test)]
pub(crate) mod tests;
