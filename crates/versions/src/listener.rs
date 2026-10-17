use std::time::Instant;

use gpui_kit::{App, AppContext, AsyncApp, Entity};

use crate::{Downloads, downloads::DownloadProgress, event::DownloadMsg};

/// Moves what the tokio side reports into the download entities, on the gpui thread. A tick
/// notifies only its row, the list notifies only on add/remove.
pub(crate) fn listen(cx: &mut App, rx: flume::Receiver<DownloadMsg>, downloads: Entity<Downloads>) {
  cx.spawn(async move |cx| {
    while let Ok(msg) = rx.recv_async().await {
      match msg {
        DownloadMsg::Start { id, title } => {
          let row = cx.new(|_| DownloadProgress::new(id, title, Instant::now()));
          downloads.update(cx, |list, cx| {
            list.rows.push((id, row));
            cx.notify();
          });
        }
        DownloadMsg::Status { id, status } => update_row(cx, &downloads, id, |d| {
          d.apply_status(&status, Instant::now())
        }),
        DownloadMsg::Finish { id, ok } => update_row(cx, &downloads, id, |d| d.finish(ok)),
      }
    }
  })
  .detach();
}

fn update_row(
  cx: &mut AsyncApp,
  downloads: &Entity<Downloads>,
  id: usize,
  f: impl FnOnce(&mut DownloadProgress),
) {
  // the row may be cleared already
  let Some(row) = downloads.read_with(cx, |list, _| list.get(id)) else {
    return;
  };
  row.update(cx, |d, cx| {
    let before = d.clone();
    f(d);
    if *d != before {
      cx.notify();
    }
  });
}
