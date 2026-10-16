use gpui_kit::{App, Entity};
use proton_utils::entity::WriteChangedExt;

use crate::{OfflineEvent, OfflineEvents, offline::OfflineMsg};

/// Moves what the tokio side reports into the entities, on the gpui thread
pub(crate) fn listen(
  cx: &mut App,
  rx: flume::Receiver<OfflineMsg>,
  offline: Entity<bool>,
  events: Entity<OfflineEvents>,
) {
  cx.spawn(async move |cx| {
    while let Ok(msg) = rx.recv_async().await {
      let event = match msg {
        OfflineMsg::Offline(value) => {
          offline.write_changed(cx, value);
          continue;
        }
        OfflineMsg::Reconnected => OfflineEvent::Reconnected,
        OfflineMsg::ManifestRefreshError => OfflineEvent::ManifestRefreshError,
      };
      events.update(cx, |_, cx| cx.emit(event));
    }
  })
  .detach();
}
