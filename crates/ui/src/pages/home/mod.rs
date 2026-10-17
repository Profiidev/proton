//! `/home`: continue playing card, favorites and history

use gpui_kit::{
  App, AppContext, BoxShadow, Context, Entity, FontWeight, InteractiveElement, IntoElement,
  ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Subscription,
  TestSupportExt, Window,
  assets::IconName,
  component::{
    ActiveTheme,
    button::{Button, ButtonVariants},
  },
  div, point, px,
};
use proton_components::widgets::{
  chip,
  content::alert,
  form::{filled, icon_label},
  format::running_label,
  profile::{ProfileExternalLink, ProfileIcon},
};
use proton_content::{ContentExt, ContentUpdates, ModpackUpdates, model::ContentKind};
use proton_profiles::{
  ProfilesExt,
  config::{PlayHistoryFavoriteInfo, Profile, QuickPlayInfo},
};
use proton_versions::loader::LoaderType;

use super::profiles::{
  ProfileMeta, card_grid, compare_profiles, fade_in, loader_name, open_profile, plural,
  profile_card, profile_meta, running,
};
use crate::{
  actions::launch,
  content::logic::route_kind,
  router::{ProfileTab, QuickPlayTab, Route, RouterExt},
  shell::PageFactory,
};

pub fn register(cx: &mut App) {
  PageFactory::register(
    cx,
    |r| *r == Route::Home,
    |_, _, cx| cx.new(HomePage::new).into(),
  );
}

/// Home banner for available content updates (`updates_banner` of the old app)
#[derive(Debug, PartialEq)]
pub struct Banner {
  pub title: String,
  pub description: String,
  pub target: Route,
}

/// "A", "A and B", "A, B and C"
fn names(list: &[&str]) -> String {
  match list {
    [] => String::new(),
    [one] => one.to_string(),
    [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
  }
}

pub fn updates_banner(updates: &ContentUpdates, profiles: &[Profile]) -> Option<Banner> {
  let affected: Vec<_> = profiles
    .iter()
    .filter(|p| updates.0.get(&p.id).is_some_and(|u| !u.is_empty()))
    .collect();
  let first = affected.first()?;
  let all: Vec<_> = affected.iter().flat_map(|p| &updates.0[&p.id]).collect();
  let word = if all.iter().all(|u| u.kind == ContentKind::Mod) {
    "mod update"
  } else {
    "update"
  };
  let list: Vec<_> = affected.iter().map(|p| p.name.as_str()).collect();
  Some(Banner {
    title: format!("{} available", plural(all.len(), word)),
    description: format!(
      "{} {} newer compatible versions.",
      names(&list),
      if affected.len() == 1 { "has" } else { "have" }
    ),
    target: Route::Profile {
      id: first.id.clone(),
      tab: ProfileTab::Content(route_kind(updates.0[&first.id][0].kind)),
    },
  })
}

/// Home banner for modpack profiles with a new pack version
pub fn pack_banner(updates: &ModpackUpdates, profiles: &[Profile]) -> Option<Banner> {
  let affected: Vec<_> = profiles
    .iter()
    .filter(|p| updates.0.contains_key(&p.id))
    .collect();
  let first = affected.first()?;
  let list: Vec<_> = affected.iter().map(|p| p.name.as_str()).collect();
  Some(Banner {
    title: format!("{} available", plural(affected.len(), "modpack update")),
    description: format!(
      "{} {} a new pack version.",
      list.join(", "),
      if affected.len() == 1 { "has" } else { "have" }
    ),
    target: Route::Profile {
      id: first.id.clone(),
      tab: ProfileTab::QuickPlay(QuickPlayTab::Singleplayer),
    },
  })
}

fn banner(id: &'static str, b: Banner, button: &'static str, cx: &App) -> impl IntoElement {
  alert(
    Some(IconName::CircleArrowUp),
    b.title,
    b.description,
    None,
    cx,
  )
  .id(id)
  .child(
    div().self_center().mr(px(2.)).child(
      Button::new((id, 0usize))
        .cursor_pointer()
        .outline()
        .label(button)
        .on_click(move |_, _, cx| cx.navigate(b.target.clone())),
    ),
  )
}

/// Most recently played profile
pub fn latest_played(profiles: &[Profile]) -> Option<&Profile> {
  profiles
    .iter()
    .filter(|p| p.last_played.is_some())
    .max_by_key(|p| p.last_played)
}

/// World or server to resume when it was played after the last plain launch
pub fn continue_quick_play(profile: &Profile) -> Option<&QuickPlayInfo> {
  let last = profile
    .quick_play
    .iter()
    .max_by_key(|q| q.last_played_time)?;
  match profile.last_played_non_quick_play {
    Some(plain) if last.last_played_time <= plain => None,
    _ => Some(last),
  }
}

pub struct HomePage {
  meta: Entity<ProfileMeta>,
  _subs: [Subscription; 7],
}

impl HomePage {
  pub fn new(cx: &mut Context<Self>) -> Self {
    let meta = profile_meta(cx);
    let p = cx.profiles().clone();
    let c = cx.content().clone();
    let subs = [
      cx.observe(&c.updates, |_, _, cx| cx.notify()),
      cx.observe(&c.pack_updates, |_, _, cx| cx.notify()),
      cx.observe(&p.profiles, |_, _, cx| cx.notify()),
      cx.observe(&p.favorites, |_, _, cx| cx.notify()),
      cx.observe(&p.history, |_, _, cx| cx.notify()),
      cx.observe(&p.instances, |_, _, cx| cx.notify()),
      cx.observe(&meta, |_, _, cx| cx.notify()),
    ];
    Self { meta, _subs: subs }
  }

  fn continue_card(&self, profile: &Profile, cx: &App) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let muted_fg = theme.muted_foreground;
    let meta = self.meta.read(cx);
    let qp = continue_quick_play(profile).cloned();
    let running = running(&profile.id, cx);
    let mods = (profile.loader != LoaderType::Vanilla)
      .then(|| meta.mods(&profile.id))
      .flatten();

    let mut parts = vec![];
    if qp.is_some() {
      parts.push(profile.name.clone());
    }
    parts.push(format!(
      "{} {}",
      loader_name(profile.loader),
      profile.version
    ));
    if let Some(n) = mods {
      parts.push(plural(n, "mod"));
    }
    if running > 0 {
      parts.push(running_label(running));
    }

    let (open_id, play_id) = (profile.id.clone(), profile.id.clone());
    let (open_qp, play_qp) = (qp.clone(), qp.clone());
    let title = qp.as_ref().map_or(&profile.name, |q| &q.name).clone();
    div()
      .id("continue")
      .flex()
      .flex_wrap()
      .items_center()
      .gap_4()
      .p_4()
      .rounded(px(14.))
      // ring-1 ring-foreground/10: outside the box, no layout space
      .shadow(vec![BoxShadow {
        color: theme.foreground.opacity(0.1),
        offset: point(px(0.), px(0.)),
        blur_radius: px(0.),
        spread_radius: px(1.),
        inset: false,
      }])
      .bg(theme.popover)
      .child(
        div()
          .id("continue-icon")
          .flex_none()
          .size(px(56.))
          .rounded(px(10.))
          .overflow_hidden()
          .bg(theme.muted)
          .cursor_pointer()
          .on_click(move |_, _, cx| open_profile(open_id.clone(), open_qp.as_ref(), cx))
          .child(
            ProfileIcon::new(meta.icon(&profile.id))
              .size(px(56.))
              .fallback_size(px(28.)),
          ),
      )
      .child(
        div()
          .flex()
          .flex_col()
          .flex_1()
          .min_w(px(240.))
          .gap_0p5()
          .child(
            div()
              .text_xs()
              .line_height(px(16.))
              .font_weight(FontWeight::MEDIUM)
              .text_color(muted_fg)
              .child("CONTINUE PLAYING"),
          )
          .child(
            div()
              .truncate()
              .text_xl()
              // measured: the web card is 2px shorter than xs + xl + xs leading + gaps
              .line_height(px(26.))
              .font_weight(FontWeight::SEMIBOLD)
              .child(title),
          )
          .child(
            div()
              .text_xs()
              .line_height(px(16.))
              .text_color(muted_fg)
              .child(parts.join(" · ")),
          ),
      )
      .child(
        filled(icon_label(
          Button::new("continue-play").cursor_pointer().primary(),
          IconName::Play,
          if running > 0 {
            "Launch another"
          } else {
            "Play"
          },
        ))
        .on_click(move |_, _, cx| launch(play_id.clone(), play_qp.clone(), cx)),
      )
  }

  fn section(
    &self,
    id: &'static str,
    header: gpui_kit::AnyElement,
    entries: &[PlayHistoryFavoriteInfo],
    empty: &'static str,
    window: &Window,
    cx: &App,
  ) -> impl IntoElement + use<> {
    let mut entries = entries.to_vec();
    entries.sort_by(|a, b| compare_profiles(&a.profile, &b.profile));
    let meta = self.meta.read(cx);
    let body = if entries.is_empty() {
      div()
        .flex()
        .flex_col()
        .child(
          div()
            .text_center()
            .text_color(cx.theme().muted_foreground)
            .child(empty),
        )
        .child(ProfileExternalLink::new(|_, cx| {
          cx.navigate(Route::Profiles)
        }))
        .into_any_element()
    } else {
      let cards: Vec<_> = entries
        .iter()
        .map(|e| {
          let qp = e.quick_play.as_ref();
          let key = format!(
            "{id}-{}-{}",
            e.profile.id,
            qp.map_or(String::new(), |q| format!("{:?}-{}", q.r#type, q.id))
          );
          profile_card(SharedString::from(key), &e.profile, qp, meta, cx)
        })
        .collect();
      fade_in(
        SharedString::from(format!("{id}-grid")),
        card_grid(window, cards),
        cx,
      )
    };
    div()
      .id(id)
      .flex()
      .flex_col()
      .gap(px(10.))
      .child(header)
      .child(body)
  }
}

fn heading(text: &'static str) -> impl IntoElement {
  div()
    .text_base()
    .line_height(px(24.))
    .font_weight(FontWeight::MEDIUM)
    .child(text)
}

impl Render for HomePage {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let p = cx.profiles().clone();
    let favorites = p.favorites.read(cx).clone();
    let history = p.history.read(cx).clone();
    let profiles = p.profiles.read(cx);
    let current = latest_played(profiles).cloned();
    let content = cx.content();
    let updates = updates_banner(content.updates.read(cx), profiles);
    let packs = pack_banner(content.pack_updates.read(cx), profiles);
    let muted_fg = cx.theme().muted_foreground;

    let fav_header = div()
      .flex()
      .items_center()
      .gap_2()
      .child(heading("Favorites"))
      .child(chip(favorites.len().to_string(), cx));
    let history_header = div()
      .flex()
      .items_center()
      .gap_2()
      .child(heading("History"))
      .child(
        div()
          .text_xs()
          .text_color(muted_fg)
          .child("Recently played"),
      );

    div()
      .id("home-page")
      .size_full()
      .overflow_y_scroll()
      .child(
        div()
          .flex()
          .flex_col()
          .gap_6()
          .px_5()
          .pt_4()
          .pb_6()
          .children(current.map(|c| self.continue_card(&c, cx)))
          .children(updates.map(|b| banner("updates-banner", b, "Review updates", cx)))
          .children(packs.map(|b| banner("packs-banner", b, "Review", cx)))
          .child(self.section(
            "favorites",
            fav_header.into_any_element(),
            &favorites,
            "No favorites found. Add a profile or quick play option to your favorites in the profile settings.",
            window,
            cx,
          ))
          .child(self.section(
            "history",
            history_header.into_any_element(),
            &history,
            "No entries found in history. Launch a profile to create one.",
            window,
            cx,
          )),
      )
      .test_support()
  }
}

#[cfg(test)]
mod tests;
