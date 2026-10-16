use gpui_kit::{App, AppContext, AsyncApp, Entity};

use crate::{LogLines, Profiles, ProfilesEvent, store::ProfilesMsg};

/// Moves what the tokio side reports into the entities, on the gpui thread
pub(crate) fn listen(cx: &mut App, rx: flume::Receiver<ProfilesMsg>, profiles: Profiles) {
  cx.spawn(async move |cx| {
    while let Ok(msg) = rx.recv_async().await {
      apply(cx, &profiles, msg);
    }
  })
  .detach();
}

fn apply(cx: &mut AsyncApp, p: &Profiles, msg: ProfilesMsg) {
  match msg {
    ProfilesMsg::Profiles {
      profiles,
      favorites,
      history,
    } => {
      set(cx, &p.profiles, profiles);
      set(cx, &p.favorites, favorites);
      set(cx, &p.history, history);
    }
    ProfilesMsg::QuickPlay(id, list) => {
      if let Some(entity) = p.lazy.read_with(cx, |l, _| l.quick_play.get(&id).cloned()) {
        set(cx, &entity, list);
      }
    }
    ProfilesMsg::Runs(id, runs) => {
      if let Some(entity) = p.lazy.read_with(cx, |l, _| l.runs.get(&id).cloned()) {
        set(cx, &entity, runs);
      }
    }
    ProfilesMsg::QuickPlayFileChanged(id) => p.handle.detach(move |h| async move {
      h.refresh_quick_play(&id).await?;
      h.refresh().await
    }),
    ProfilesMsg::ContentChanged(id) => emit(cx, p, ProfilesEvent::ContentChanged(id)),
    ProfilesMsg::InstanceCrashed { profile_name } => {
      emit(cx, p, ProfilesEvent::InstanceCrashed { profile_name })
    }
    ProfilesMsg::InstanceAdded(info) => {
      let logs = cx.new(|_| LogLines::default());
      p.lazy
        .update(cx, |l, _| l.logs.insert(info.id.clone(), logs));
      p.instances.update(cx, |list, cx| {
        list.push(info);
        cx.notify();
      });
    }
    ProfilesMsg::InstanceRemoved(id) => {
      p.lazy.update(cx, |l, _| l.logs.remove(&id));
      p.instances.update(cx, |list, cx| {
        list.retain(|i| i.id != id);
        cx.notify();
      });
    }
    ProfilesMsg::InstanceLines(id, lines) => {
      if let Some(entity) = p.lazy.read_with(cx, |l, _| l.logs.get(&id).cloned()) {
        entity.update(cx, |log, cx| {
          log.0.extend(lines);
          cx.notify();
        });
      }
    }
  }
}

fn set<T: 'static>(cx: &mut AsyncApp, entity: &Entity<T>, value: T) {
  entity.update(cx, |v, cx| {
    *v = value;
    cx.notify();
  });
}

fn emit(cx: &mut AsyncApp, p: &Profiles, event: ProfilesEvent) {
  p.events.update(cx, |_, cx| cx.emit(event));
}
