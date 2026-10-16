//! "From modpack" and "Import file" tabs of the create profile dialog

use std::{path::PathBuf, rc::Rc, time::Duration};

use gpui_kit::{
  App, AppContext, Context, Entity, ExternalPaths, FontWeight, ImageSource, InteractiveElement,
  IntoElement, ParentElement, PathPromptOptions, Render, StatefulInteractiveElement, Styled,
  Subscription, Task, Window,
  assets::IconName,
  component::{
    ActiveTheme, Icon, Sizable,
    button::Button,
    input::{Input, InputEvent, InputState},
    spinner::Spinner,
  },
  div,
  prelude::FluentBuilder,
  px,
};
use proton_components::widgets::{content::ProjectIcon, format::format_count};
use proton_content::{
  Content, ContentError, ContentExt,
  model::{ContentKind, PackPlan, PackSource, ProjectSummary, SearchQuery, SearchSort},
};

use super::content_error;

const DEBOUNCE: Duration = Duration::from_millis(300);

type OnSelect = Rc<dyn Fn(ProjectSummary, &mut Window, &mut App)>;
type OnPlan = Rc<dyn Fn(PackPlan, &mut Window, &mut App)>;

/// Modpack search query of the tab, the 10 most downloaded without a query
pub fn modpack_query(query: &str) -> SearchQuery {
  let query = query.trim().to_string();
  SearchQuery {
    kind: ContentKind::Modpack,
    sort: if query.is_empty() {
      SearchSort::Downloads
    } else {
      SearchSort::Relevance
    },
    query,
    sources: Content::SOURCES.to_vec(),
    loaders: vec![],
    game_version: None,
    categories: vec![],
    environment: None,
    offset: 0,
    limit: 10,
    compatible_with: None,
  }
}

/// Search + list of modpacks with "Select"
pub struct FromModpackTab {
  search: Entity<InputState>,
  /// None while loading
  hits: Option<Vec<ProjectSummary>>,
  failed: bool,
  on_select: OnSelect,
  _search: Option<Task<()>>,
  _subs: [Subscription; 1],
}

impl FromModpackTab {
  pub fn new(
    on_select: impl Fn(ProjectSummary, &mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) -> Self {
    let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search modpacks"));
    let sub = cx.subscribe(&search, |this, input, e: &InputEvent, cx| {
      if matches!(e, InputEvent::Change) {
        let query = input.read(cx).value().to_string();
        this.run(query, true, cx);
      }
    });
    let mut this = Self {
      search,
      hits: None,
      failed: false,
      on_select: Rc::new(on_select),
      _search: None,
      _subs: [sub],
    };
    this.run(String::new(), false, cx);
    this
  }

  fn run(&mut self, query: String, debounce: bool, cx: &mut Context<Self>) {
    let content = cx.content().clone();
    // replacing the task cancels the pending one
    self._search = Some(cx.spawn(async move |this, cx| {
      if debounce {
        cx.background_executor().timer(DEBOUNCE).await;
      }
      let res = content.search(modpack_query(&query)).await;
      let _ = this.update(cx, |this, cx| {
        this.failed = res.is_err();
        this.hits = res.ok().map(|r| r.hits);
        cx.notify();
      });
    }));
  }
}

impl Render for FromModpackTab {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let muted = cx.theme().muted_foreground;
    let accent = cx.theme().accent;
    let message = |text: &'static str| div().m_auto().text_sm().text_color(muted).child(text);
    let list = div()
      .id("modpack-results")
      .flex()
      .flex_col()
      .gap_1()
      .h(px(320.))
      .overflow_y_scroll()
      .map(|el| match &self.hits {
        _ if self.failed => el.child(message(
          "Search failed. Check your connection and try again.",
        )),
        None => el.child(div().m_auto().child(Spinner::new())),
        Some(hits) if hits.is_empty() => el.child(message("No modpacks found.")),
        Some(hits) => el.children(hits.iter().enumerate().map(|(ix, hit)| {
          let sub: Vec<String> = hit
            .loaders
            .iter()
            .chain(hit.game_versions.last())
            .map(|s| super::logic::tag_label(s))
            .collect();
          let on_select = self.on_select.clone();
          let project = hit.clone();
          div()
            .id(("modpack", ix))
            .flex()
            .items_center()
            .gap_3()
            .p_2()
            .rounded(px(10.))
            .hover(move |s| s.bg(accent.opacity(0.5)))
            .child(ProjectIcon::new(hit.icon_url.clone().map(ImageSource::from)).size(px(36.)))
            .child(
              div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .child(
                  div()
                    .truncate()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child(hit.name.clone()),
                )
                .child(
                  div()
                    .truncate()
                    .text_xs()
                    .text_color(muted)
                    .child(sub.join(" · ")),
                ),
            )
            .child(
              div()
                .flex_none()
                .text_xs()
                .text_color(muted)
                .child(format!("{} downloads", format_count(hit.downloads))),
            )
            .child(
              Button::new(("select", ix))
                .outline()
                .small()
                .label("Select")
                .on_click(move |_, window, cx| on_select(project.clone(), window, cx)),
            )
        })),
      });
    div()
      .flex()
      .flex_col()
      .gap_3()
      .pt_4()
      .min_w_0()
      .child(Input::new(&self.search).prefix(Icon::new(IconName::Search).small()))
      .child(list)
  }
}

/// Drop box / file picker for `.mrpack` and `.zip` packs
pub struct ImportFileTab {
  busy: bool,
  error: Option<&'static str>,
  on_plan: OnPlan,
  _load: Option<Task<()>>,
}

fn is_pack(path: &std::path::Path) -> bool {
  path
    .extension()
    .and_then(|e| e.to_str())
    .is_some_and(|e| e.eq_ignore_ascii_case("mrpack") || e.eq_ignore_ascii_case("zip"))
}

impl ImportFileTab {
  pub fn new(on_plan: impl Fn(PackPlan, &mut Window, &mut App) + 'static) -> Self {
    Self {
      busy: false,
      error: None,
      on_plan: Rc::new(on_plan),
      _load: None,
    }
  }

  fn load(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
    if self.busy {
      return;
    }
    if !is_pack(&path) {
      self.error = Some("Only .mrpack and .zip files can be imported");
      cx.notify();
      return;
    }
    self.busy = true;
    self.error = None;
    let plan = cx.content().modpack_plan(PackSource::Path { path });
    self._load = Some(cx.spawn_in(window, async move |this, cx| {
      let res = plan.await;
      let _ = this.update_in(cx, |this, window, cx| {
        this.busy = false;
        match res {
          Ok(plan) => (this.on_plan.clone())(plan, window, cx),
          Err(e) => {
            this.error = Some(
              if matches!(content_error(&e), Some(ContentError::InvalidPack)) {
                "This file is not a supported modpack"
              } else {
                "Failed to read the modpack file"
              },
            )
          }
        }
        cx.notify();
      });
    }));
    cx.notify();
  }

  fn choose(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let paths = cx.prompt_for_paths(PathPromptOptions {
      files: true,
      directories: false,
      multiple: false,
      prompt: Some("Modpack".into()),
    });
    cx.spawn_in(window, async move |this, cx| {
      if let Ok(Ok(Some(paths))) = paths.await
        && let Some(path) = paths.into_iter().next()
      {
        let _ = this.update_in(cx, |this, window, cx| this.load(path, window, cx));
      }
    })
    .detach();
  }
}

impl Render for ImportFileTab {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let theme = cx.theme();
    let (muted, primary) = (theme.muted_foreground, theme.primary);
    div()
      .flex()
      .flex_col()
      .gap_3()
      .pt_4()
      .min_w_0()
      .child(
        div()
          .id("import-drop")
          .flex()
          .flex_col()
          .items_center()
          .justify_center()
          .gap_3()
          .w_full()
          .h(px(192.))
          .p_6()
          .rounded(px(14.))
          .border_1()
          .border_dashed()
          .border_color(theme.border)
          .drag_over::<ExternalPaths>(move |s, _, _, _| {
            s.border_color(primary).bg(primary.opacity(0.1))
          })
          .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
            if let Some(path) = paths.paths().first() {
              this.load(path.clone(), window, cx);
            }
          }))
          .map(|el| {
            if self.busy {
              el.child(Spinner::new().large())
                .child(div().text_sm().text_color(muted).child("Reading modpack…"))
            } else {
              el.child(Icon::new(IconName::FileUp).size(px(24.)).text_color(muted))
                .child(
                  div()
                    .text_sm()
                    .text_color(muted)
                    .child("Drop an .mrpack or .zip modpack file here"),
                )
                .child(
                  Button::new("choose-file")
                    .outline()
                    .label("Choose file")
                    .on_click(cx.listener(|this, _, window, cx| this.choose(window, cx))),
                )
            }
          }),
      )
      .when_some(self.error, |el, error| {
        el.child(div().text_sm().text_color(theme.danger).child(error))
      })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn modpack_queries_and_pack_files() {
    let q = modpack_query("  ");
    assert_eq!(
      (q.query.as_str(), q.sort, q.limit),
      ("", SearchSort::Downloads, 10)
    );
    assert_eq!(modpack_query(" fo ").sort, SearchSort::Relevance);
    assert!(is_pack("a/b.MRPACK".as_ref()));
    assert!(is_pack("b.zip".as_ref()));
    assert!(!is_pack("b.jar".as_ref()));
  }
}
