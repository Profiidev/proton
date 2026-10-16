//! `DiscoverPage.svelte`: search, filters and a virtual list of project cards

use std::{collections::HashSet, rc::Rc, time::Duration};

use chrono::{DateTime, Utc};
use gpui_kit::{
  AnyElement, App, AppContext, Context, Entity, FontWeight, ImageSource, InteractiveElement,
  IntoElement, ListAlignment, ListState, ParentElement, Render, SharedString, Styled, Subscription,
  Task, WeakEntity, Window,
  assets::IconName,
  component::{
    ActiveTheme, Disableable, Icon, IndexPath, Sizable,
    alert::Alert,
    button::Button,
    checkbox::Checkbox,
    input::{Input, InputEvent, InputState},
    scroll::ScrollableElement,
    select::{SearchableVec, Select, SelectEvent, SelectState},
    separator::Separator,
    skeleton::Skeleton,
    switch::Switch,
  },
  div, list,
  prelude::FluentBuilder,
  px,
};
use proton_components::widgets::{
  content::{CardAction, ProjectCard},
  form::VersionSelect,
};
use proton_content::{
  ContentExt, InstalledContent,
  model::{
    ContentKind, Environment, ProjectSummary, SearchQuery, SearchResult, SearchSort, Source,
  },
};
use proton_profiles::ProfilesExt;
use proton_versions::{VersionsExt, loader::LoaderType};

use crate::{
  content::{
    InstallDialog, ModpackInstallDialog, ProjectRef, TargetPicker,
    logic::{self, CategoryGroup, Filters},
    target, target_picker,
  },
  router::{Kind, Route, RouterExt},
};

/// Runs a search; the page uses the content service, tests count calls
pub type SearchFn = Rc<dyn Fn(SearchQuery, &mut App) -> Task<anyhow::Result<SearchResult>>>;

const DEBOUNCE: Duration = Duration::from_millis(250);
const ANY_VERSION: &str = "Any version";
const SORTS: [(SearchSort, &str); 4] = [
  (SearchSort::Relevance, "Relevance"),
  (SearchSort::Downloads, "Downloads"),
  (SearchSort::Updated, "Updated"),
  (SearchSort::Newest, "Newest"),
];
const LOADERS: [(&str, &str); 4] = [
  ("fabric", "Fabric"),
  ("quilt", "Quilt"),
  ("forge", "Forge"),
  ("neoforge", "NeoForge"),
];

/// Title, subtitle and noun per kind
fn texts(kind: ContentKind) -> (&'static str, &'static str, &'static str) {
  match kind {
    ContentKind::Mod => (
      "Mods",
      "Browse Modrinth and CurseForge and install straight into a profile.",
      "mods",
    ),
    ContentKind::Modpack => (
      "Modpacks",
      "Each modpack installs as its own profile, with every mod, pack and setting in place.",
      "modpacks",
    ),
    ContentKind::ResourcePack => (
      "Resource packs",
      "Change textures, sounds and animations. Installed packs show up in the profile's content tab.",
      "resource packs",
    ),
    ContentKind::Shader => (
      "Shaders",
      "Lighting, shadows and water. Shaders need Iris and Sodium, which Proton can add for you.",
      "shaders",
    ),
  }
}

/// A page of results is loaded when the list scrolls this close to its end
const LOAD_AHEAD: usize = 3;

pub struct DiscoverPage {
  kind: ContentKind,
  filters: Filters,
  search_fn: SearchFn,
  search: Entity<InputState>,
  sort: Entity<SelectState<SearchableVec<SharedString>>>,
  game_version: Entity<VersionSelect>,
  picker: Option<Entity<TargetPicker>>,
  groups: Vec<CategoryGroup>,
  /// Query of the shown results, None before the first search
  query: Option<SearchQuery>,
  hits: Rc<Vec<ProjectSummary>>,
  total: u64,
  warnings: Vec<String>,
  loading: bool,
  more_loading: bool,
  failed: bool,
  list: ListState,
  /// (profile, its installed content of this kind, its mods)
  installed: Option<(String, Entity<InstalledContent>)>,
  _search: Option<Task<()>>,
  _debounce: Option<Task<()>>,
  _more: Option<Task<()>>,
  _subs: Vec<Subscription>,
}

impl DiscoverPage {
  pub fn new(kind: Kind, window: &mut Window, cx: &mut Context<Self>) -> Self {
    let search_fn: SearchFn = Rc::new(|query, cx| {
      let fut = cx.content().search(query);
      cx.spawn(async move |_| fut.await)
    });
    Self::with_search(kind, search_fn, window, cx)
  }

  pub fn with_search(
    kind: Kind,
    search_fn: SearchFn,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) -> Self {
    let kind = logic::content_kind(kind);
    let noun = texts(kind).2;
    let search = cx.new(|cx| InputState::new(window, cx).placeholder(format!("Search {noun}")));
    let sort = cx.new(|cx| {
      SelectState::new(
        SearchableVec::new(
          SORTS
            .iter()
            .map(|(_, l)| SharedString::from(*l))
            .collect::<Vec<_>>(),
        ),
        Some(IndexPath::default().row(0)),
        window,
        cx,
      )
    });
    let this = cx.entity().downgrade();
    let game_version = cx.new(|cx| {
      VersionSelect::new(
        "Game version",
        vec![ANY_VERSION.into()],
        Some(ANY_VERSION.into()),
        window,
        cx,
      )
      .on_change(move |v, _, cx| {
        let v = (v != ANY_VERSION).then(|| v.to_string());
        let _ = this.update(cx, |this, cx| {
          this.filters.game_version = v;
          this.refresh(cx);
        });
      })
    });
    let picker = (kind != ContentKind::Modpack).then(|| {
      let filter = target_picker::any_profile();
      cx.new(|cx| TargetPicker::new(filter, window, cx))
    });

    let profiles = cx.profiles().profiles.clone();
    let target = target(cx);
    let updates = cx.content().updates.clone();
    let mut subs = vec![
      cx.subscribe(&search, |this, input, e: &InputEvent, cx| {
        if matches!(e, InputEvent::Change) {
          let text = input.read(cx).value().to_string();
          this.query_changed(text, cx);
        }
      }),
      cx.subscribe(&sort, |this, _, e: &SelectEvent<_>, cx| {
        let SelectEvent::Confirm(Some(label)): &SelectEvent<SearchableVec<SharedString>> = e else {
          return;
        };
        if let Some((sort, _)) = SORTS.iter().find(|(_, l)| *l == label.as_ref()) {
          this.filters.sort = *sort;
          this.refresh(cx);
        }
      }),
      cx.observe(&profiles, |this, _, cx| this.refresh(cx)),
      cx.observe(&target, |this, _, cx| this.refresh(cx)),
      cx.observe(&updates, |_, _, cx| cx.notify()),
    ];
    let categories = cx.content().categories.clone();
    subs.push(cx.observe(&categories, |this, c, cx| {
      let list = c.read(cx).get(&this.kind).cloned().unwrap_or_default();
      this.groups = logic::category_groups(this.kind, &list);
      cx.notify();
    }));
    cx.content().clone().load_categories(kind, cx).detach();
    if kind != ContentKind::Shader {
      let versions = cx.versions().clone().version_list(LoaderType::Vanilla, cx);
      let select = game_version.clone();
      cx.spawn_in(window, async move |_, cx| {
        if let Ok(list) = versions.await {
          let mut options = vec![SharedString::from(ANY_VERSION)];
          options.extend(list.into_iter().map(SharedString::from));
          let _ = select.update_in(cx, |s, window, cx| {
            s.set_options(options, Some(ANY_VERSION.into()), window, cx)
          });
        }
      })
      .detach();
    }

    let list = ListState::new(1, ListAlignment::Top, px(800.));
    let weak = cx.entity().downgrade();
    list.set_scroll_handler(move |e, _, cx| {
      if e.visible_range.end + LOAD_AHEAD >= e.count {
        let _ = weak.update(cx, |this, cx| this.load_more(cx));
      }
    });

    let mut this = Self {
      kind,
      filters: Filters::new(kind),
      search_fn,
      search,
      sort,
      game_version,
      picker,
      groups: vec![],
      query: None,
      hits: Rc::default(),
      total: 0,
      warnings: vec![],
      loading: true,
      more_loading: false,
      failed: false,
      list,
      installed: None,
      _search: None,
      _debounce: None,
      _more: None,
      _subs: subs,
    };
    this.refresh(cx);
    this
  }

  /// Search text typed; searches 250ms after the last keystroke
  pub fn query_changed(&mut self, text: String, cx: &mut Context<Self>) {
    // replacing the task cancels the pending timer
    self._debounce = Some(cx.spawn(async move |this, cx| {
      cx.background_executor().timer(DEBOUNCE).await;
      let _ = this.update(cx, |this, cx| {
        this.filters.query = text;
        this.refresh(cx);
      });
    }));
  }

  fn target_id(&self, cx: &mut App) -> Option<String> {
    if self.kind == ContentKind::Modpack {
      return None;
    }
    target_picker::candidates(&target_picker::any_profile(), cx)
      .1
      .map(|p| p.id)
  }

  fn can_filter_compatible(&self) -> bool {
    matches!(self.kind, ContentKind::Mod | ContentKind::ResourcePack)
  }

  /// Compatible mode is on and possible
  fn compatible(&self, target: Option<&str>) -> bool {
    self.can_filter_compatible() && self.filters.compatible && target.is_some()
  }

  /// Searches again when the query changed
  fn refresh(&mut self, cx: &mut Context<Self>) {
    let target = self.target_id(cx);
    self.watch_installed(target.clone(), cx);
    let mut filters = self.filters.clone();
    filters.compatible = self.compatible(target.as_deref());
    let query = logic::build_query(self.kind, &filters, target.as_deref(), 0);
    if self.query.as_ref().is_some_and(|q| same_query(q, &query)) {
      cx.notify();
      return;
    }
    self.query = Some(query.clone());
    self.loading = true;
    self._more = None;
    self.more_loading = false;
    let task = (self.search_fn)(query, cx);
    self._search = Some(cx.spawn(async move |this, cx| {
      let res = task.await;
      let _ = this.update(cx, |this, cx| {
        this.loading = false;
        this.failed = res.is_err();
        let res = res.unwrap_or(SearchResult {
          hits: vec![],
          total: 0,
          warnings: vec![],
        });
        this.hits = Rc::new(res.hits);
        this.total = res.total;
        this.warnings = res.warnings;
        this.list.reset(this.item_count());
        cx.notify();
      });
    }));
    cx.notify();
  }

  fn load_more(&mut self, cx: &mut Context<Self>) {
    if self.loading || self.more_loading || self.hits.len() as u64 >= self.total {
      return;
    }
    let Some(mut query) = self.query.clone() else {
      return;
    };
    query.offset = self.hits.len() as u32;
    self.more_loading = true;
    let task = (self.search_fn)(query.clone(), cx);
    self._more = Some(cx.spawn(async move |this, cx| {
      let res = task.await;
      let _ = this.update(cx, |this, cx| {
        this.more_loading = false;
        if let Ok(res) = res
          && this.query.as_ref().is_some_and(|q| same_query(q, &query))
        {
          let at = this.alert_count(cx) + this.hits.len();
          let added = res.hits.len();
          Rc::make_mut(&mut this.hits).extend(res.hits);
          this.total = res.total;
          this.list.splice(at..at, added);
        }
        cx.notify();
      });
    }));
    cx.notify();
  }

  fn watch_installed(&mut self, target: Option<String>, cx: &mut Context<Self>) {
    if self.installed.as_ref().map(|(id, _)| id) == target.as_ref() {
      return;
    }
    self.installed = target.map(|id| {
      let entity = cx.content().clone().installed(&id, cx);
      cx.observe(&entity, |_, _, cx| cx.notify()).detach();
      (id, entity)
    });
  }

  /// Shader readiness alert + one per backend warning
  fn alert_count(&self, cx: &App) -> usize {
    let shader = self.kind == ContentKind::Shader && self.installed.is_some();
    let _ = cx;
    shader as usize + self.warnings.len()
  }

  /// alerts, cards, footer
  fn item_count(&self) -> usize {
    let shader = self.kind == ContentKind::Shader && self.installed.is_some();
    shader as usize + self.warnings.len() + self.hits.len() + 1
  }

  fn open_install(&self, project: &ProjectSummary, window: &mut Window, cx: &mut App) {
    let project = ProjectRef::from(project);
    if self.kind == ContentKind::Modpack {
      ModpackInstallDialog::open(project, window, cx);
    } else {
      InstallDialog::open(project, None, window, cx);
    }
  }
}

/// Same search except the paging offset
fn same_query(a: &SearchQuery, b: &SearchQuery) -> bool {
  let key = |q: &SearchQuery| {
    serde_json::to_string(&SearchQuery {
      offset: 0,
      ..q.clone()
    })
    .unwrap_or_default()
  };
  key(a) == key(b)
}

fn toggle<T: PartialEq>(list: &mut Vec<T>, value: T, on: bool) {
  list.retain(|v| *v != value);
  if on {
    list.push(value);
  }
}

/// Checkbox group of the filter aside
fn group(label: impl Into<SharedString>, options: Vec<AnyElement>) -> impl IntoElement {
  div()
    .flex()
    .flex_col()
    .gap(px(6.))
    .child(div().font_weight(FontWeight::MEDIUM).child(label.into()))
    .children(options)
}

fn skeleton_card(cx: &App) -> impl IntoElement {
  let theme = cx.theme();
  div()
    .flex()
    .items_center()
    .gap_4()
    .px_4()
    .py(px(14.))
    .rounded(px(14.))
    .border_1()
    .border_color(theme.foreground.opacity(0.1))
    .bg(theme.popover)
    .child(Skeleton::new().size(px(72.)).rounded(px(10.)))
    .child(
      div()
        .flex()
        .flex_col()
        .flex_1()
        .gap_2()
        .child(Skeleton::new().h_5().w(px(200.)))
        .child(Skeleton::new().h_4().w_full())
        .child(Skeleton::new().h_4().w(px(280.))),
    )
}

impl DiscoverPage {
  fn aside(
    &self,
    target: Option<&proton_profiles::config::Profile>,
    cx: &mut Context<Self>,
  ) -> impl IntoElement {
    let theme = cx.theme();
    let muted = theme.muted_foreground;
    let compatible = self.compatible(target.map(|t| t.id.as_str()));
    let mut sections: Vec<AnyElement> = vec![];

    if self.can_filter_compatible() {
      let sub = match target {
        None => "No profile selected".to_string(),
        Some(t) if self.kind == ContentKind::Mod => format!(
          "{} {}, from {}",
          logic::loader_label(t.loader),
          t.version,
          t.name
        ),
        Some(t) => format!("Made for {}", t.version),
      };
      sections.push(
        div()
          .flex()
          .items_start()
          .justify_between()
          .gap_3()
          .child(
            div()
              .flex()
              .flex_col()
              .gap(px(2.))
              .child(
                div()
                  .font_weight(FontWeight::MEDIUM)
                  .child("Compatible only"),
              )
              .child(div().text_xs().text_color(muted).child(sub)),
          )
          .child(
            Switch::new("compatible")
              .checked(self.filters.compatible)
              .disabled(target.is_none())
              .on_click(cx.listener(|this, on: &bool, _, cx| {
                this.filters.compatible = *on;
                this.refresh(cx);
              })),
          )
          .into_any_element(),
      );
      sections.push(Separator::horizontal().into_any_element());
    }

    let sources = [Source::Modrinth, Source::CurseForge]
      .into_iter()
      .map(|s| {
        Checkbox::new(SharedString::from(format!("source-{s:?}")))
          .checked(self.filters.sources.contains(&s))
          .label(logic::source_label(s))
          .on_click(cx.listener(move |this, on: &bool, _, cx| {
            toggle(&mut this.filters.sources, s, *on);
            this.refresh(cx);
          }))
          .into_any_element()
      })
      .collect();
    sections.push(group("Source", sources).into_any_element());

    if matches!(self.kind, ContentKind::Mod | ContentKind::Modpack) && !compatible {
      let loaders = LOADERS
        .iter()
        .map(|(id, label)| {
          let id = id.to_string();
          Checkbox::new(SharedString::from(format!("loader-{id}")))
            .checked(self.filters.loaders.contains(&id))
            .label(*label)
            .on_click(cx.listener(move |this, on: &bool, _, cx| {
              toggle(&mut this.filters.loaders, id.clone(), *on);
              this.refresh(cx);
            }))
            .into_any_element()
        })
        .collect();
      sections.push(group("Loader", loaders).into_any_element());
    }

    if self.kind != ContentKind::Shader && !compatible {
      sections.push(
        group(
          "Game version",
          vec![self.game_version.clone().into_any_element()],
        )
        .into_any_element(),
      );
    }

    for g in &self.groups {
      let options = g
        .names
        .iter()
        .map(|name| {
          let name = name.clone();
          Checkbox::new(SharedString::from(format!("cat-{}-{name}", g.header)))
            .checked(self.filters.categories.contains(&name))
            .label(logic::tag_label(&name))
            .on_click(cx.listener(move |this, on: &bool, _, cx| {
              toggle(&mut this.filters.categories, name.clone(), *on);
              this.refresh(cx);
            }))
            .into_any_element()
        })
        .collect();
      sections.push(group(g.label.clone(), options).into_any_element());
    }

    if self.kind == ContentKind::Mod {
      let envs = [
        (Environment::Client, "Client"),
        (Environment::Server, "Server"),
      ]
      .into_iter()
      .map(|(env, label)| {
        Checkbox::new(label)
          .checked(self.filters.environment == Some(env))
          .label(label)
          .on_click(cx.listener(move |this, on: &bool, _, cx| {
            this.filters.environment = on.then_some(env);
            this.refresh(cx);
          }))
          .into_any_element()
      })
      .collect();
      sections.push(group("Runs on", envs).into_any_element());
    }

    div()
      .id("filters")
      .flex_none()
      .w(px(232.))
      .max_h_full()
      .mb_6()
      .flex()
      .flex_col()
      .gap(px(18.))
      .p_4()
      .rounded(px(14.))
      .border_1()
      .border_color(theme.foreground.opacity(0.1))
      .bg(theme.popover)
      .text_sm()
      .overflow_y_scrollbar()
      .children(sections)
  }

  fn render_item(
    this: &WeakEntity<Self>,
    ix: usize,
    window: &mut Window,
    cx: &mut App,
  ) -> AnyElement {
    this
      .update(cx, |page, cx| page.item(ix, window, cx))
      .unwrap_or_else(|_| div().into_any_element())
  }

  fn item(&mut self, ix: usize, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
    let (_, _, noun) = texts(self.kind);
    let shader = self.kind == ContentKind::Shader && self.installed.is_some();
    let wrap = |el: AnyElement| div().pb(px(10.)).child(el).into_any_element();
    let mut ix = ix;
    if shader {
      if ix == 0 {
        let profile = target_picker::candidates(&target_picker::any_profile(), cx).1;
        let name = profile.map(|p| p.name).unwrap_or_default();
        let ready = self
          .installed_mods(cx)
          .is_some_and(|m| logic::shader_ready(&m));
        let alert = if ready {
          Alert::new(
            "shader-alert",
            "Iris Shaders and Sodium are already installed. On profiles without them, Proton adds both when you install a shader.",
          )
          .title(format!("{name} is ready for shaders"))
        } else {
          Alert::new(
            "shader-alert",
            format!("Proton adds both to {name} when you install a shader."),
          )
          .title("Shaders need Iris and Sodium")
        };
        return wrap(alert.into_any_element());
      }
      ix -= 1;
    }
    if let Some(w) = self.warnings.get(ix) {
      return wrap(
        Alert::warning(("warning", ix), w.clone())
          .title("Some results are missing")
          .icon(IconName::TriangleAlert)
          .into_any_element(),
      );
    }
    ix -= self.warnings.len();
    if let Some(project) = self.hits.get(ix).cloned() {
      return wrap(self.card(project, cx).into_any_element());
    }
    // footer
    let muted = cx.theme().muted_foreground;
    if self.hits.is_empty() {
      if self.loading {
        return div()
          .flex()
          .flex_col()
          .gap(px(10.))
          .children((0..3).map(|_| skeleton_card(cx)))
          .into_any_element();
      }
      let (title, text) = if self.failed {
        (
          "Search failed".to_string(),
          "Check your connection and try again.",
        )
      } else {
        (
          format!("No {noun} found"),
          "Try other search terms or fewer filters.",
        )
      };
      return div()
        .flex()
        .flex_col()
        .items_center()
        .gap_1()
        .p_6()
        .rounded(px(14.))
        .border_1()
        .border_dashed()
        .border_color(cx.theme().border)
        .child(div().font_weight(FontWeight::MEDIUM).child(title))
        .child(div().text_sm().text_color(muted).child(text))
        .into_any_element();
    }
    if self.more_loading {
      return wrap(skeleton_card(cx).into_any_element());
    }
    if (self.hits.len() as u64) < self.total {
      return div()
        .flex()
        .justify_center()
        .pt(px(6.))
        .pb_6()
        .child(
          Button::new("load-more")
            .outline()
            .label("Load more")
            .on_click(cx.listener(|this, _, _, cx| this.load_more(cx))),
        )
        .into_any_element();
    }
    div().pb_6().into_any_element()
  }

  fn installed_mods(&self, cx: &mut App) -> Option<Vec<proton_content::model::Installed>> {
    let (id, _) = self.installed.as_ref()?;
    let mods = cx.content().clone().installed(id, cx);
    Some(mods.read(cx).get(ContentKind::Mod).to_vec())
  }

  fn card(&self, project: ProjectSummary, cx: &mut Context<Self>) -> impl IntoElement {
    let action = match (&self.installed, self.kind) {
      (_, ContentKind::Modpack) | (None, _) => CardAction::Install,
      (Some((target, installed)), kind) => {
        let ids: HashSet<String> = installed
          .read(cx)
          .get(kind)
          .iter()
          .filter_map(|i| i.project_id.clone())
          .collect();
        let updates = cx.content().updates.read(cx).0.get(target).cloned();
        logic::card_action(&project.id, &ids, updates.as_deref().unwrap_or_default())
      }
    };
    let route = Route::Project {
      id: project.id.clone(),
      kind: logic::route_kind(project.kind),
      source: logic::route_source(project.source),
    };
    let weak = cx.entity().downgrade();
    let install = project.clone();
    ProjectCard::new(
      SharedString::from(format!("{:?}-{}", project.source, project.id)),
      project.name.clone(),
    )
    .icon(project.icon_url.clone().map(ImageSource::from))
    .byline(logic::source_label(project.source), &project.author)
    .description(project.description.clone())
    .tags(
      logic::card_tags(&project)
        .into_iter()
        .map(SharedString::from),
    )
    .downloads(project.downloads)
    .when_some(
      project.updated_at.parse::<DateTime<Utc>>().ok(),
      ProjectCard::updated,
    )
    .action(action)
    .on_open(move |_, cx| cx.navigate(route.clone()))
    .on_install(move |window, cx| {
      let _ = weak.update(cx, |this, cx| this.open_install(&install, window, cx));
    })
  }
}

impl Render for DiscoverPage {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let (title, subtitle, _) = texts(self.kind);
    let theme = cx.theme();
    let (muted, primary) = (theme.muted_foreground, theme.primary);
    let target = (self.kind != ContentKind::Modpack)
      .then(|| target_picker::candidates(&target_picker::any_profile(), cx).1)
      .flatten();
    let count = self.item_count();
    if self.list.item_count() != count {
      self.list.reset(count);
    }
    let sort_label = SORTS
      .iter()
      .find(|(s, _)| *s == self.filters.sort)
      .map(|(_, l)| *l)
      .unwrap_or_default();
    let _ = sort_label;
    let weak = cx.entity().downgrade();

    div()
      .size_full()
      .flex()
      .flex_col()
      .gap_4()
      .px_5()
      .pt_4()
      .child(
        div()
          .flex()
          .flex_none()
          .flex_wrap()
          .items_center()
          .gap_3()
          .child(
            div()
              .flex_1()
              .min_w(px(240.))
              .child(
                div()
                  .text_xl()
                  .font_weight(FontWeight::SEMIBOLD)
                  .child(title),
              )
              .child(div().mt(px(2.)).text_sm().text_color(muted).child(subtitle)),
          )
          .map(|el| match &self.picker {
            Some(picker) => el.child(picker.clone()),
            None => el.child(
              div()
                .flex()
                .items_center()
                .h_7()
                .px_3()
                .rounded_full()
                .bg(primary.opacity(0.15))
                .text_color(primary)
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .child("Creates a new profile"),
            ),
          }),
      )
      .child(
        div()
          .flex()
          .flex_none()
          .flex_wrap()
          .gap_2()
          .child(
            div()
              .flex_1()
              .min_w(px(200.))
              .child(Input::new(&self.search).prefix(Icon::new(IconName::Search).small())),
          )
          .child(
            div().w(px(176.)).child(
              Select::new(&self.sort)
                .id("sort")
                .title_prefix("Sort: ")
                .accessibility_label("Sort"),
            ),
          ),
      )
      .child(
        div()
          .flex()
          .flex_1()
          .min_h_0()
          .items_start()
          .gap_5()
          .child(self.aside(target.as_ref(), cx))
          .child(
            div()
              .id("results")
              .relative()
              .flex_1()
              .min_w_0()
              .h_full()
              .child(
                list(self.list.clone(), move |ix, window, cx| {
                  Self::render_item(&weak, ix, window, cx)
                })
                .size_full(),
              )
              .vertical_scrollbar(&self.list),
          ),
      )
  }
}

#[cfg(test)]
mod tests {
  use std::cell::RefCell;

  use gpui_kit::{self as gpui, TestAppContext};

  use super::*;
  use crate::pages::discover::project::tests::init_services;

  #[gpui::test]
  fn typing_debounces_into_one_search(cx: &mut TestAppContext) {
    init_services(cx);
    let seen: Rc<RefCell<Vec<String>>> = Rc::default();
    let log = seen.clone();
    let search: SearchFn = Rc::new(move |q, _| {
      log.borrow_mut().push(q.query.clone());
      Task::ready(Ok(SearchResult {
        hits: vec![crate::content::logic::tests::summary("sodium")],
        total: 1,
        warnings: vec![],
      }))
    });
    let (page, cx) =
      cx.add_window_view(|window, cx| DiscoverPage::with_search(Kind::Modpack, search, window, cx));
    cx.run_until_parked();
    assert_eq!(*seen.borrow(), [""]);

    for text in ["s", "so", "sod"] {
      page.update(cx, |p, cx| p.query_changed(text.into(), cx));
      cx.executor().advance_clock(Duration::from_millis(100));
    }
    cx.run_until_parked();
    // still inside the 250ms window of the last keystroke
    assert_eq!(seen.borrow().len(), 1);
    cx.executor().advance_clock(DEBOUNCE);
    cx.run_until_parked();
    assert_eq!(*seen.borrow(), ["", "sod"]);
    page.read_with(cx, |p, _| assert_eq!(p.hits.len(), 1));
  }
}
