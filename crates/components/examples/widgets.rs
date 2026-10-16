//! `cargo run -p proton_components --example widgets`: every shared widget with sample data

use std::{sync::Arc, time::Duration};

use chrono::Utc;
use gpui_kit::{
  App, AppContext, Bounds, Context, Entity, Image, ImageFormat, ImageSource, InteractiveElement,
  IntoElement, ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Window,
  WindowBounds, WindowOptions,
  component::{
    ActiveTheme,
    button::{Button, ButtonVariants},
  },
  div, px, size,
};
use proton_components::widgets::{
  account::AccountImage,
  content::{CardAction, ProjectCard, ProjectIcon},
  form::{ImageInput, SwitchTile, SwitchTooltip, VersionSelect, open_destroy_dialog},
  instance::InstanceRow,
  logs::{LogBuffer, LogWindow},
  profile::{ProfileCard, ProfileExternalLink, QuickPlayIcon},
};

/// Gradient square like the fixture profile icons
fn gradient(from: &str, to: &str) -> ImageSource {
  let svg = format!(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="{from}"/><stop offset="1" stop-color="{to}"/></linearGradient></defs><rect width="64" height="64" fill="url(#g)"/></svg>"#
  );
  Arc::new(Image::from_bytes(ImageFormat::Svg, svg.into_bytes())).into()
}

const LOG: &[&str] = &[
  "[12:00:01] [main/INFO]: Loading Minecraft 1.21.8 with Fabric Loader 0.17.2",
  "[12:00:01] [main/INFO]: Setting user: Steve_Fixture",
  "[12:00:02] [Render thread/INFO]: Backend library: LWJGL version 3.3.3",
  "[12:00:02] [Render thread/WARN]: Missing sound for event: minecraft:item.goat_horn.play",
  "[12:00:03] [Render thread/INFO]: Reloading ResourceManager: vanilla, fabric",
  "[12:00:04] [Worker-Main-2/INFO]: Preparing spawn area: 0%",
  "[12:00:05] [Worker-Main-2/INFO]: Preparing spawn area: 83%",
  "[12:00:06] [Server thread/INFO]: Steve_Fixture joined the game",
  "[12:00:07] [Render thread/ERROR] [Iris]: Failed to load shader pack",
  "java.lang.RuntimeException: boom",
  "\tat net.minecraft.client.Main.main(Main.java:42)",
  "[12:00:30] [Render thread/DEBUG]: Stopping!",
];

struct Gallery {
  versions: Entity<VersionSelect>,
  image: Entity<ImageInput>,
  log: Entity<LogWindow>,
  snapshots: bool,
}

impl Gallery {
  fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
    let buffer = cx.new(|cx| {
      let mut b = LogBuffer::new();
      b.extend(LOG.iter().copied(), cx);
      b
    });
    // live tail: a line every 250ms
    cx.spawn({
      let buffer = buffer.clone();
      async move |_, cx| {
        for i in 0..u64::MAX {
          cx.background_executor()
            .timer(Duration::from_millis(250))
            .await;
          let line = format!("[12:01:{:02}] [Server thread/INFO]: tick {i}", i % 60);
          buffer.update(cx, |b, cx| b.push(&line, cx));
        }
      }
    })
    .detach();
    Self {
      versions: cx.new(|cx| {
        VersionSelect::new(
          "Version",
          ["1.21.8", "1.21.7", "1.21.1", "1.20.1", "25w41a"]
            .map(SharedString::from)
            .to_vec(),
          Some("1.21.8".into()),
          window,
          cx,
        )
      }),
      image: cx.new(|_| ImageInput::new(None).label("Icon").placeholder("Icon")),
      log: cx.new(|cx| LogWindow::new(buffer, cx).source(true)),
      snapshots: false,
    }
  }
}

fn section(title: &str) -> gpui_kit::Div {
  div().flex().flex_col().gap_2().child(
    div()
      .text_base()
      .font_weight(gpui_kit::FontWeight::MEDIUM)
      .child(title.to_string()),
  )
}

impl Render for Gallery {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let grid = || div().grid().grid_cols(3).gap_2();
    let now = Utc::now();
    let snapshots = self.snapshots;
    div()
      .id("gallery")
      .size_full()
      .overflow_y_scroll()
      .bg(cx.theme().background)
      .text_color(cx.theme().foreground)
      .flex()
      .flex_col()
      .gap_6()
      .px_5()
      .pt_4()
      .pb_6()
      .child(
        section("Profiles").child(
          grid()
            .child(
              ProfileCard::new("p1", "Vanilla Survival", "Vanilla 1.21.8")
                .icon(Some(gradient("#4d9a3a", "#8a6b2f")))
                .favorite(true)
                .meta("played 15 hours ago"),
            )
            .child(
              ProfileCard::new("p2", "Fabulously Optimized", "Fabric 1.21.8")
                .icon(Some(gradient("#e0559a", "#6a3fc9")))
                .favorite(true)
                .meta("4 mods · played 1 day ago"),
            )
            .child(
              ProfileCard::new("p3", "Fabric Performance", "Fabric 1.21.8")
                .icon(Some(gradient("#c9b48a", "#3c3f7a")))
                .meta("6 mods · played 1 day ago"),
            )
            .child(
              ProfileCard::new("p4", "Forge Classic", "Forge 1.20.1")
                .icon(Some(gradient("#3c4a7a", "#e08a3c")))
                .meta("3 mods · never played"),
            )
            .child(
              ProfileCard::new("p5", "Running Pack", "NeoForge 1.21.1")
                .running(2)
                .meta("12 mods · played 2 days ago"),
            )
            .child(
              div()
                .flex()
                .gap_2()
                .child(QuickPlayIcon::new(None, true))
                .child(QuickPlayIcon::new(None, false))
                .child(ProjectIcon::new(None))
                .child(ProfileExternalLink::new(|_, _| {})),
            ),
        ),
      )
      .child(
        section("Discover").child(
          div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
              ProjectCard::new("fabric-api", "Fabric API")
                .byline("Modrinth", "modmuss50")
                .description(
                  "Lightweight and modular API providing common hooks and intercompatibility \
                   measures utilized by mods using the Fabric toolchain.",
                )
                .tags(["Library", "Fabric", "Client and server"].map(SharedString::from))
                .downloads(280_123_456)
                .updated(now - chrono::Duration::days(1)),
            )
            .child(
              ProjectCard::new("sodium", "Sodium")
                .icon(Some(gradient("#7bd36b", "#3c9a3a")))
                .byline("Modrinth", "jellysquid3")
                .description(
                  "A high-performance rendering engine replacement for Minecraft, which greatly \
                   improves frame rates and reduces micro-stutter.",
                )
                .tags(
                  ["Optimization", "Fabric", "Neoforge", "Quilt", "Client"].map(SharedString::from),
                )
                .downloads(246_000_000)
                .updated(now - chrono::Duration::hours(15))
                .action(CardAction::Update),
            )
            .child(
              ProjectCard::new("jei", "Just Enough Items (JEI)")
                .byline("CurseForge", "mezz")
                .description("View Items and Recipes")
                .tags(["Map and Information", "API and Library", "Forge"].map(SharedString::from))
                .downloads(636_000_000)
                .updated(now - chrono::Duration::minutes(37))
                .action(CardAction::Installed),
            ),
        ),
      )
      .child(
        section("Accounts").child(
          div()
            .flex()
            .items_center()
            .gap_4()
            .child(AccountImage::new("head", Some("icons/proton.svg".into())))
            .child(AccountImage::new("loading", None).loading(true))
            .child(AccountImage::new("unknown", None)),
        ),
      )
      .child(
        section("Forms").child(
          div()
            .flex()
            .flex_col()
            .gap_3()
            .max_w(px(720.))
            .child(
              SwitchTile::new(
                "snapshots",
                "Show snapshots",
                "List snapshots and pre-releases when creating a profile.",
              )
              .checked(snapshots)
              .on_change(cx.listener(|this, v: &bool, _, cx| {
                this.snapshots = *v;
                cx.notify();
              })),
            )
            .child(
              SwitchTile::new(
                "local",
                "Use profile game settings",
                "Override the global Minecraft settings for this profile only.",
              )
              .disabled(true),
            )
            .child(
              SwitchTooltip::new("custom", "Custom size (1.13+)", "Use a custom window size")
                .checked(snapshots),
            )
            .child(self.versions.clone())
            .child(self.image.clone())
            .child(
              Button::new("destroy")
                .danger()
                .label("Stop instance")
                .on_click(|_, window, cx| {
                  open_destroy_dialog(
                    window,
                    cx,
                    "Stop Instance",
                    "Are you sure you want to stop this instance?",
                    "Stop",
                    |_, _| {},
                  )
                }),
            ),
        ),
      )
      .child(
        section("Instances")
          .child(
            InstanceRow::new("i1", 1, "1h 12m")
              .account(Some("Steve_Fixture".into()))
              .memory(Some(3 << 30), Some(8192))
              .card(true),
          )
          .child(
            div()
              .rounded(px(14.))
              .border_1()
              .border_color(cx.theme().border)
              .bg(cx.theme().popover.opacity(0.5))
              .overflow_hidden()
              .child(InstanceRow::new("i2", 1, "5m").account(Some("Alex_Fixture".into())))
              .child(InstanceRow::new("i3", 2, "<1m")),
          ),
      )
      .child(
        section("Logs").child(
          div().h(px(420.)).child(
            self
              .log
              .clone()
              .cached(gpui_kit::StyleRefinement::default().size_full()),
          ),
        ),
      )
  }
}

fn main() {
  gpui_kit::application()
    .with_assets(proton_components::assets::Assets)
    .run(|cx: &mut App| {
      gpui_kit::init(cx);
      proton_components::assets::load(cx).expect("Failed to load the theme");
      let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
          None,
          size(px(1300.), px(800.)),
          cx,
        ))),
        app_id: Some("proton-widgets".into()),
        ..Default::default()
      };
      gpui_kit::open_window(options, cx, |window, cx| {
        cx.new(|cx| Gallery::new(window, cx))
      })
      .expect("Failed to open the window");
    });
}
