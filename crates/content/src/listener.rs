use gpui_kit::{App, Entity};
use proton_utils::entity::WriteChangedExt;

use crate::{ContentUpdates, ModpackUpdates, updates::ContentMsg};

/// Moves what the tokio side reports into the entities, on the gpui thread
pub(crate) fn listen(
  cx: &mut App,
  rx: flume::Receiver<ContentMsg>,
  updates: Entity<ContentUpdates>,
  packs: Entity<ModpackUpdates>,
) {
  cx.spawn(async move |cx| {
    while let Ok(msg) = rx.recv_async().await {
      match msg {
        ContentMsg::Updates(v) => updates.write_changed(cx, ContentUpdates(v)),
        ContentMsg::PackUpdates(v) => packs.write_changed(cx, ModpackUpdates(v)),
      }
    }
  })
  .detach();
}
