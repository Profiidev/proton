//! One content list row. Each row is its own entity, so a toggle re-renders only that row.

use std::rc::Rc;

use gpui_kit::{
  App, AppContext, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, Render,
  SharedString, StatefulInteractiveElement, Styled, TestSupportExt, WeakEntity, Window,
  assets::IconName,
  component::{
    ActiveTheme, Icon, Sizable,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    menu::{DropdownMenu, PopupMenuItem},
    radio::Radio,
    switch::Switch,
  },
  div,
  prelude::FluentBuilder,
  px,
};
use proton_components::widgets::{
  chip, colors,
  content::{ProjectIcon, radio_dot},
};
use proton_content::model::{ContentKind, Installed};

use super::{
  ContentPage,
  logic::{display_name, source_label},
};

/// Row height incl. its bottom border, all rows are equal for the virtual list
pub const ROW_HEIGHT: f32 = 61.;

/// What a row shows; rows only notify when this changes
#[derive(Debug, Clone, PartialEq)]
pub struct RowData {
  pub entry: Installed,
  /// Newer version number when an update is available
  pub update: Option<String>,
  pub selected: bool,
}

pub struct ContentRow {
  pub data: RowData,
  page: WeakEntity<ContentPage>,
  /// Render counter, tests assert fine-grained updates with it
  pub renders: usize,
}

/// The value carried while a resource pack is dragged
#[derive(Clone)]
pub struct DraggedRow {
  pub file: String,
  name: SharedString,
}

impl Render for DraggedRow {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let theme = cx.theme();
    div()
      .px_3()
      .py_2()
      .rounded(px(8.))
      .border_1()
      .border_color(theme.border)
      .bg(theme.popover)
      .text_sm()
      .opacity(0.8)
      .child(self.name.clone())
  }
}

impl ContentRow {
  pub fn new(data: RowData, page: WeakEntity<ContentPage>) -> Self {
    Self {
      data,
      page,
      renders: 0,
    }
  }

  /// Notifies only on a real change
  pub fn set(&mut self, data: RowData, cx: &mut Context<Self>) {
    if self.data != data {
      self.data = data;
      cx.notify();
    }
  }

  /// A menu action running on the page
  fn action(
    &self,
    f: impl Fn(&mut ContentPage, &mut Window, &mut Context<ContentPage>) + 'static,
  ) -> RowAction {
    let page = self.page.clone();
    Rc::new(move |window, cx| {
      let _ = page.update(cx, |p, cx| f(p, window, cx));
    })
  }
}

type RowAction = Rc<dyn Fn(&mut Window, &mut App)>;

impl Render for ContentRow {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    self.renders += 1;
    let theme = cx.theme().clone();
    let RowData {
      entry,
      update,
      selected,
    } = self.data.clone();
    let kind = entry.kind;
    let file = entry.file_name.clone();
    let name: SharedString = display_name(&entry).to_string().into();
    let page = self.page.clone();

    let call = |f: fn(&mut ContentPage, String, bool, &mut Window, &mut Context<ContentPage>)| {
      let (page, file) = (page.clone(), file.clone());
      move |on: &bool, window: &mut Window, cx: &mut App| {
        let (file, on) = (file.clone(), *on);
        let _ = page.update(cx, |p, cx| f(p, file, on, window, cx));
      }
    };

    let grip = (kind == ContentKind::ResourcePack).then(|| {
      if entry.enabled {
        let dragged = DraggedRow {
          file: file.clone(),
          name: name.clone(),
        };
        div()
          .id("grip")
          .flex_none()
          .cursor_grab()
          .child(
            Icon::new(IconName::GripVertical)
              .size_4()
              .text_color(theme.muted_foreground),
          )
          .on_drag(dragged, |v, _, _, cx| cx.new(|_| v.clone()))
          .into_any_element()
      } else {
        div().size_4().flex_none().into_any_element()
      }
    });

    let toggle = if kind == ContentKind::Shader {
      let radio = Radio::new("toggle")
        .cursor_pointer()
        .checked(entry.active == Some(true))
        .accessibility_label(format!("Use {name}"))
        .on_click({
          let (page, file) = (page.clone(), file.clone());
          move |_, window, cx| {
            let file = file.clone();
            let _ = page.update(cx, |p, cx| p.set_shader(Some(file), window, cx));
          }
        });
      radio_dot(radio, entry.active == Some(true), cx).into_any_element()
    } else {
      Switch::new("toggle")
        .cursor_pointer()
        .checked(entry.enabled)
        .accessibility_label(format!("Enable {name}"))
        .on_click(call(ContentPage::toggle))
        .into_any_element()
    };

    let open_project = entry
      .source
      .zip(entry.project_id.clone())
      .map(|(source, id)| self.action(move |p, _, cx| p.open_project(source, id.clone(), cx)));
    let show_folder = self.action(|p, _, cx| p.show_folder(cx));
    let remove = {
      let file = file.clone();
      self.action(move |p, window, cx| p.ask_remove(vec![file.clone()], window, cx))
    };
    let menu = Button::new("menu")
      .cursor_pointer()
      .ghost()
      .small()
      .size_7() // shadcn icon-sm
      .icon(IconName::Ellipsis)
      .accessibility_label("More actions")
      .dropdown_menu_with_anchor(gpui_kit::Anchor::TopRight, move |menu, _, _| {
        let (show_folder, remove) = (show_folder.clone(), remove.clone());
        menu
          .when_some(open_project.clone(), |menu, f| {
            menu.item(
              PopupMenuItem::new("Open project page")
                .icon(IconName::ExternalLink)
                .on_click(move |_, w, cx| f(w, cx)),
            )
          })
          .item(
            PopupMenuItem::new("Show in folder")
              .icon(IconName::FolderOpen)
              .on_click(move |_, w, cx| show_folder(w, cx)),
          )
          .item(
            PopupMenuItem::new("Remove")
              .icon(IconName::Trash)
              .on_click(move |_, w, cx| remove(w, cx)),
          )
      });

    let drop_page = page.clone();
    let target = file.clone();
    div()
      .id(SharedString::from(file.clone()))
      .w_full()
      .h(px(ROW_HEIGHT))
      .flex()
      .items_center()
      .gap_3()
      .px_3()
      // top separator: a bottom one on the last row pokes past the table's rounded corners
      .border_t_1()
      .border_color(theme.border)
      .when(kind == ContentKind::ResourcePack && entry.enabled, |el| {
        el.drag_over::<DraggedRow>(|s, _, _, cx| s.bg(cx.theme().accent.opacity(0.6)))
          .on_drop(move |dragged: &DraggedRow, window, cx| {
            let (from, to) = (dragged.file.clone(), target.clone());
            let _ = drop_page.update(cx, |p, cx| p.reorder(from, to, window, cx));
          })
      })
      .children(grip)
      .child(
        Checkbox::new("select")
          .cursor_pointer()
          .checked(selected)
          .accessibility_label(format!("Select {name}"))
          .on_click(call(ContentPage::select)),
      )
      .child(ProjectIcon::new(entry.icon_url.clone().map(Into::into)))
      .child(
        div()
          .flex()
          .flex_col()
          .flex_1()
          .min_w_0()
          .child(
            div()
              .flex()
              .min_w_0()
              .items_center()
              .gap_2()
              .child(
                div()
                  .truncate()
                  .line_height(px(24.))
                  .font_weight(FontWeight::MEDIUM)
                  .child(name.clone()),
              )
              .when(!entry.dependency_of.is_empty(), |el| {
                el.child(chip("Dependency", cx))
              })
              .when(entry.from_pack, |el| el.child(chip("From pack", cx))),
          )
          .child(
            div()
              .truncate()
              .text_xs()
              .line_height(px(16.))
              .text_color(theme.muted_foreground)
              .child(file.clone()),
          ),
      )
      .child(
        div()
          .flex()
          .flex_col()
          .flex_none()
          .w(px(144.))
          .text_xs()
          .line_height(px(16.))
          .child(
            div()
              .truncate()
              .child(entry.version_number.clone().unwrap_or("-".into())),
          )
          .when_some(update.clone(), |el, v| {
            el.child(
              div()
                .truncate()
                .text_color(colors::yellow_500())
                .child(format!("Update: {v}")),
            )
          }),
      )
      .child(
        div()
          .w(px(104.))
          .flex()
          .flex_none()
          // the web chip is inline-flex on a 24px line: baseline aligned, 1.5px low
          .h(px(24.))
          .pt(px(3.5))
          .child(chip(source_label(entry.source), cx)),
      )
      .child(
        div()
          .w_32()
          .flex_none()
          .flex()
          .items_center()
          .justify_end()
          .gap_2()
          .when(update.is_some(), |el| {
            let file = file.clone();
            el.child(
              Button::new("update")
                .cursor_pointer()
                .outline()
                .xsmall()
                .label("Update")
                .on_click(move |_, _, cx| {
                  let file = file.clone();
                  let _ = page.update(cx, |p, cx| p.update(Some(vec![file]), cx));
                }),
            )
          })
          .child(toggle),
      )
      .child(menu)
      .test_support()
  }
}
