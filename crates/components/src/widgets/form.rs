//! Form widgets: destroy confirmation, version select, switch tiles, image input

use std::{
  path::{Path, PathBuf},
  rc::Rc,
};

use gpui_kit::{
  App, AppContext, Context, ElementId, Entity, FontWeight, ImageSource, InteractiveElement,
  IntoElement, ObjectFit, ParentElement, PathPromptOptions, Pixels, Render, RenderOnce,
  SharedString, StatefulInteractiveElement, Styled, StyledImage, Subscription, TestSupportExt,
  Window,
  assets::IconName,
  component::{
    ActiveTheme, Disableable, Icon, IndexPath, WindowExt,
    button::{Button, ButtonVariants},
    dialog::{DialogDescription, DialogFooter},
    select::{SearchableVec, Select, SelectEvent, SelectState},
    switch::Switch,
    tooltip::Tooltip,
  },
  div, img,
  prelude::FluentBuilder,
  px,
};

use super::{Handler, input_tile};

/// `DestroyDialog`: title, description and one destructive button running `on_confirm`.
/// Escape closes it. A plain dialog like the web app's (shadcn `Dialog.Footer` is right-aligned,
/// kit `AlertDialog` centers its buttons).
pub fn open_destroy_dialog(
  window: &mut Window,
  cx: &mut App,
  title: impl Into<SharedString>,
  description: impl Into<SharedString>,
  button: impl Into<SharedString>,
  on_confirm: impl Fn(&mut Window, &mut App) + 'static,
) {
  let (title, description, button) = (title.into(), description.into(), button.into());
  let on_confirm = Rc::new(on_confirm);
  window.open_dialog(cx, move |dialog, _, _| {
    let on_confirm = on_confirm.clone();
    dialog
      .title(title.clone())
      .close_button(true)
      .child(DialogDescription::new().child(description.clone()))
      .footer(
        DialogFooter::new().child(Button::new("ok").danger().label(button.clone()).on_click(
          move |_, window, cx| {
            on_confirm(window, cx);
            window.close_dialog(cx);
          },
        )),
      )
  });
}

/// shadcn buttons have a 1px transparent border with `bg-clip-padding`: filled variants
/// (primary, destructive) paint only 30px of their 32px box, kit buttons paint all of it
pub fn filled(button: Button) -> Button {
  button.h(px(30.)).m(px(1.))
}

/// [`filled`] for an icon-only (`size-8`) button
pub fn filled_icon(button: Button) -> Button {
  button.size(px(30.)).m(px(1.))
}

/// shadcn icon + label content (`data-icon=inline-start`: `pl-2`, `gap-1.5`, 16px icon);
/// kit `.icon().label()` uses `px-2.5 gap-2`
pub fn icon_label(button: Button, icon: IconName, label: impl Into<SharedString>) -> Button {
  let label = label.into();
  button.pl_2().accessibility_label(label.clone()).child(
    div()
      .flex()
      .items_center()
      .gap(px(6.))
      .child(Icon::new(icon).size_4())
      .child(label),
  )
}

/// shadcn `Alert`: bg-card, rounded-lg border, `px-2.5 py-2`, 16px icon beside a medium
/// title and a muted description
pub fn alert(
  icon: IconName,
  title: impl Into<SharedString>,
  description: impl Into<SharedString>,
  cx: &App,
) -> gpui_kit::Div {
  super::content::alert(Some(icon), title, description.into(), None, cx)
}

type Versions = SelectState<SearchableVec<SharedString>>;

/// Searchable single select of versions; a chosen value can not be cleared
pub struct VersionSelect {
  label: SharedString,
  state: Entity<Versions>,
  /// A value is set but not among the options: the web select showed an empty button
  missing: bool,
  disabled: bool,
  on_change: Option<Handler<SharedString>>,
  _confirm: Subscription,
}

impl VersionSelect {
  pub fn new(
    label: impl Into<SharedString>,
    options: Vec<SharedString>,
    value: Option<SharedString>,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) -> Self {
    let selected = selected_index(&options, value.as_ref());
    let missing = value.is_some() && selected.is_none();
    let state = cx.new(|cx| {
      SelectState::new(SearchableVec::new(options), selected, window, cx).searchable(true)
    });
    let confirm = cx.subscribe_in(&state, window, |this, _, event, window, cx| {
      let SelectEvent::Confirm(Some(value)) = event else {
        return;
      };
      if let Some(f) = this.on_change.clone() {
        f(value, window, cx);
      }
    });
    Self {
      label: label.into(),
      state,
      missing,
      disabled: false,
      on_change: None,
      _confirm: confirm,
    }
  }

  pub fn on_change(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
    self.on_change = Some(Rc::new(f));
    self
  }

  pub fn value(&self, cx: &App) -> Option<SharedString> {
    self.state.read(cx).selected_value().cloned()
  }

  pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
    self.disabled = disabled;
    cx.notify();
  }

  /// Replaces the options (e.g. another loader) and the selected value
  pub fn set_options(
    &mut self,
    options: Vec<SharedString>,
    value: Option<SharedString>,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    let selected = selected_index(&options, value.as_ref());
    self.missing = value.is_some() && selected.is_none();
    self.state.update(cx, |state, cx| {
      state.set_items(SearchableVec::new(options), window, cx);
      state.set_selected_index(selected, window, cx);
    });
  }
}

fn selected_index(options: &[SharedString], value: Option<&SharedString>) -> Option<IndexPath> {
  let ix = options.iter().position(|o| Some(o) == value)?;
  Some(IndexPath::default().row(ix))
}

impl Render for VersionSelect {
  fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
    let lower = self.label.to_lowercase();
    let placeholder = match self.missing {
      true => String::new(),
      false => format!("Select {}", self.label),
    };
    // the web select is an outline button: medium text centered, no chevron (the empty icon
    // keeps its 12px + 4px gap, the left padding balances it)
    Select::new(&self.state)
      .id("version-select")
      .w_full()
      .icon(Icon::empty())
      .text_center()
      .font_weight(FontWeight::MEDIUM)
      .pl(px(26.))
      .placeholder(placeholder)
      .search_placeholder(format!("Search {lower}..."))
      .empty({
        let text = SharedString::from(format!("No {} found", self.label));
        move |_, _| div().p_2().text_sm().child(text.clone())
      })
      .accessibility_label(self.label.clone())
      .disabled(self.disabled)
  }
}

/// Settings tile: switch left, title + description, whole tile toggles
#[derive(IntoElement)]
pub struct SwitchTile {
  id: ElementId,
  title: SharedString,
  description: SharedString,
  checked: bool,
  disabled: bool,
  on_change: Option<Handler<bool>>,
}

impl SwitchTile {
  pub fn new(
    id: impl Into<ElementId>,
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
  ) -> Self {
    Self {
      id: id.into(),
      title: title.into(),
      description: description.into(),
      checked: false,
      disabled: false,
      on_change: None,
    }
  }

  pub fn checked(mut self, checked: bool) -> Self {
    self.checked = checked;
    self
  }

  pub fn disabled(mut self, disabled: bool) -> Self {
    self.disabled = disabled;
    self
  }

  pub fn on_change(mut self, f: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
    self.on_change = Some(Rc::new(f));
    self
  }
}

impl RenderOnce for SwitchTile {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    let (checked, disabled) = (self.checked, self.disabled);
    let on_change = self.on_change.filter(|_| !disabled);
    let tile_change = on_change.clone();
    input_tile(cx)
      .id(self.id)
      .flex()
      .items_center()
      .gap_3()
      .px(px(14.))
      .py_3()
      .when(!disabled, |this| this.cursor_pointer())
      .when_some(tile_change, |this, f| {
        this.on_click(move |_, w, cx| f(&!checked, w, cx))
      })
      .child(
        Switch::new("switch")
          .checked(checked)
          .disabled(disabled)
          .accessibility_label(self.title.clone())
          .when_some(on_change, |s, f| s.on_change(move |v, w, cx| f(v, w, cx))),
      )
      .child(
        div()
          .flex()
          .flex_col()
          .flex_1()
          .child(
            div()
              .line_height(px(24.))
              .font_weight(FontWeight::MEDIUM)
              .child(self.title),
          )
          .child(
            div()
              .text_xs()
              .line_height(px(16.))
              .text_color(cx.theme().muted_foreground)
              .child(self.description),
          ),
      )
      .test_support()
  }
}

/// Label with a tooltip on the left, switch on the right
#[derive(IntoElement)]
pub struct SwitchTooltip {
  id: ElementId,
  label: SharedString,
  tooltip: SharedString,
  checked: bool,
  disabled: bool,
  on_change: Option<Handler<bool>>,
}

impl SwitchTooltip {
  pub fn new(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    tooltip: impl Into<SharedString>,
  ) -> Self {
    Self {
      id: id.into(),
      label: label.into(),
      tooltip: tooltip.into(),
      checked: false,
      disabled: false,
      on_change: None,
    }
  }

  pub fn checked(mut self, checked: bool) -> Self {
    self.checked = checked;
    self
  }

  pub fn disabled(mut self, disabled: bool) -> Self {
    self.disabled = disabled;
    self
  }

  pub fn on_change(mut self, f: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
    self.on_change = Some(Rc::new(f));
    self
  }
}

impl RenderOnce for SwitchTooltip {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    let tooltip = self.tooltip;
    div()
      .id(self.id)
      .flex()
      .items_center()
      .text_sm()
      .font_weight(FontWeight::MEDIUM)
      .child(
        div()
          .id("label")
          .when(self.disabled, |d| d.text_color(cx.theme().muted_foreground))
          .child(self.label)
          .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx)),
      )
      .child(
        Switch::new("switch")
          .ml_auto()
          .checked(self.checked)
          .disabled(self.disabled)
          .when_some(self.on_change, |s, f| {
            s.on_change(move |v, w, cx| f(v, w, cx))
          }),
      )
  }
}

/// `FormImage` / `ImageInput`: clickable avatar previewing the picked png/jpeg,
/// `Upload` fallback with optional placeholder text
pub struct ImageInput {
  label: Option<SharedString>,
  placeholder: Option<SharedString>,
  preview: Option<ImageSource>,
  picked: Option<PathBuf>,
  size: Pixels,
  disabled: bool,
  on_change: Option<Handler<Path>>,
}

impl ImageInput {
  pub fn new(preview: Option<ImageSource>) -> Self {
    Self {
      label: None,
      placeholder: None,
      preview,
      picked: None,
      size: px(80.),
      disabled: false,
      on_change: None,
    }
  }

  pub fn label(mut self, label: impl Into<SharedString>) -> Self {
    self.label = Some(label.into());
    self
  }

  pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
    self.placeholder = Some(placeholder.into());
    self
  }

  pub fn size(mut self, size: Pixels) -> Self {
    self.size = size;
    self
  }

  pub fn on_change(mut self, f: impl Fn(&Path, &mut Window, &mut App) + 'static) -> Self {
    self.on_change = Some(Rc::new(f));
    self
  }

  pub fn picked(&self) -> Option<&Path> {
    self.picked.as_deref()
  }

  pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
    self.disabled = disabled;
    cx.notify();
  }

  fn pick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let paths = cx.prompt_for_paths(PathPromptOptions {
      files: true,
      directories: false,
      multiple: false,
      prompt: Some("Select image".into()),
    });
    cx.spawn_in(window, async move |this, cx| {
      let Ok(Ok(Some(paths))) = paths.await else {
        return;
      };
      let Some(path) = paths.into_iter().find(|p| is_image(p)) else {
        return;
      };
      this
        .update_in(cx, |this, window, cx| {
          if let Some(f) = this.on_change.clone() {
            f(&path, window, cx);
          }
          this.picked = Some(path);
          cx.notify();
        })
        .ok();
    })
    .detach();
  }
}

/// `accept="image/png, image/jpeg, image/jpg"`
fn is_image(path: &Path) -> bool {
  path
    .extension()
    .and_then(|e| e.to_str())
    .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg"))
}

impl Render for ImageInput {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let theme = cx.theme();
    let placeholder = self.placeholder.clone();
    let muted = theme.muted;
    let fallback = move || {
      div()
        .flex()
        .flex_col()
        .size_full()
        .items_center()
        .justify_center()
        .gap_1()
        .rounded(px(8.))
        .bg(muted)
        .child(Icon::new(IconName::Upload).size_4())
        .when_some(placeholder.clone(), |d, p| {
          d.child(div().text_xs().child(p))
        })
        .into_any_element()
    };
    let source = self
      .picked
      .clone()
      .map(ImageSource::from)
      .or_else(|| self.preview.clone());

    div()
      .flex()
      .flex_col()
      .gap_2()
      .when_some(self.label.clone(), |d, l| {
        d.child(div().text_sm().font_weight(FontWeight::MEDIUM).child(l))
      })
      .child(
        div()
          .id("image-input")
          .flex_none()
          .size(self.size + px(8.))
          .p(px(2.))
          .rounded(px(12.))
          .border_2()
          .border_color(theme.accent)
          .when(!self.disabled, |d| {
            d.cursor_pointer()
              .on_click(cx.listener(|this, _, window, cx| this.pick(window, cx)))
          })
          // overflow_hidden clips square, so the image and fallback carry the radius
          .child(match source {
            Some(src) => img(src)
              .size_full()
              .rounded(px(8.))
              .object_fit(ObjectFit::Cover)
              .with_fallback(fallback)
              .into_any_element(),
            None => fallback(),
          })
          .test_support(),
      )
  }
}

#[cfg(test)]
mod tests {
  use std::cell::{Cell, RefCell};

  use gpui_kit::{
    self as gpui, TestAppContext,
    component::button::{Button, ButtonVariants},
    test::TestWindowExt,
  };

  use super::*;
  use crate::test_view;

  #[gpui::test]
  fn destroy_dialog_confirms(cx: &mut TestAppContext) {
    let confirmed = Rc::new(Cell::new(0));
    let (handle, _) = test_view::open(cx, {
      let confirmed = confirmed.clone();
      move |_, _| {
        let confirmed = confirmed.clone();
        Button::new("open")
          .danger()
          .label("Stop")
          .on_click(move |_, window, cx| {
            let confirmed = confirmed.clone();
            open_destroy_dialog(
              window,
              cx,
              "Stop Instance",
              "Are you sure you want to stop this instance?",
              "Stop",
              move |_, _| confirmed.set(confirmed.get() + 1),
            );
          })
          .into_any_element()
      }
    });
    cx.update_window(handle, |_, window, cx| {
      window.render_frame(cx);
      window.click("open", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
      window.render_frame(cx);
      assert!(window.has_active_dialog(cx));
      window.within("dialog").click("ok", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(confirmed.get(), 1);
    cx.update_window(handle, |_, window, cx| {
      assert!(!window.has_active_dialog(cx))
    })
    .unwrap();
  }

  #[gpui::test]
  fn version_select_filters_and_picks(cx: &mut TestAppContext) {
    let picked = Rc::new(RefCell::new(None));
    let select = Rc::new(RefCell::new(None));
    let (handle, _) = test_view::open(cx, {
      let (picked, select) = (picked.clone(), select.clone());
      move |window, cx| {
        let entity = select
          .borrow_mut()
          .get_or_insert_with(|| {
            let picked = picked.clone();
            cx.new(|cx| {
              VersionSelect::new(
                "Version",
                vec!["1.21.8".into(), "1.21.1".into(), "1.20.1".into()],
                Some("1.21.8".into()),
                window,
                cx,
              )
              .on_change(move |v, _, _| *picked.borrow_mut() = Some(v.clone()))
            })
          })
          .clone();
        div().w(px(300.)).child(entity).into_any_element()
      }
    });
    cx.update_window(handle, |_, window, cx| {
      window.render_frame(cx);
      window.within("version-select").click("input", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
      window.render_frame(cx);
      window.input("1.20", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
      window.render_frame(cx);
      window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(picked.borrow().as_deref(), Some("1.20.1"));
    let entity = select.borrow().clone().unwrap();
    assert_eq!(
      entity.read_with(cx, |s, cx| s.value(cx)).as_deref(),
      Some("1.20.1")
    );
  }

  #[gpui::test]
  fn switch_tiles_toggle(cx: &mut TestAppContext) {
    let value = Rc::new(Cell::new(false));
    let (handle, _) = test_view::open(cx, {
      let value = value.clone();
      move |_, _| {
        let (a, b) = (value.clone(), value.clone());
        div()
          .w(px(500.))
          .child(
            SwitchTile::new("tile", "Show snapshots", "List snapshots")
              .checked(a.get())
              .on_change(move |v, _, _| a.set(*v)),
          )
          .child(SwitchTile::new("off", "Disabled", "Nothing").disabled(true))
          .child(
            SwitchTooltip::new("tip", "Label", "Tooltip")
              .checked(b.get())
              .on_change(move |v, _, _| b.set(*v)),
          )
          .into_any_element()
      }
    });
    cx.update_window(handle, |_, window, cx| {
      window.render_frame(cx);
      window.within("tile").click("switch", cx);
    })
    .unwrap();
    assert!(value.get());
    cx.update_window(handle, |_, window, cx| {
      window.render_frame(cx);
      window.click("tile", cx);
      assert_eq!(window.within("tile").find("switch").checked(), Some(false));
      window.within("tip").click("switch", cx);
    })
    .unwrap();
    assert!(value.get());
  }

  #[gpui::test]
  fn image_input_picks_a_file(cx: &mut TestAppContext) {
    let picked = Rc::new(RefCell::new(None::<PathBuf>));
    let input = cx.new(|_| {
      let picked = picked.clone();
      ImageInput::new(None)
        .placeholder("Icon")
        .on_change(move |p, _, _| *picked.borrow_mut() = Some(p.to_path_buf()))
    });
    let (handle, _) = test_view::open(cx, {
      let input = input.clone();
      move |_, _| input.clone().into_any_element()
    });
    cx.update_window(handle, |_, window, cx| {
      window.render_frame(cx);
      window.click("image-input", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(cx.did_prompt_for_paths());
    cx.simulate_path_prompt_response(|_| Some(vec!["/tmp/icon.png".into()]));
    cx.run_until_parked();
    assert_eq!(picked.borrow().as_deref(), Some(Path::new("/tmp/icon.png")));
    assert_eq!(
      input.read_with(cx, |i, _| i.picked().map(Path::to_path_buf)),
      Some("/tmp/icon.png".into())
    );
  }
}
