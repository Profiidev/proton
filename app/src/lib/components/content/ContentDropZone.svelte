<script lang="ts">
  import { Upload } from '@lucide/svelte';
  import { getCurrentWebview } from '@tauri-apps/api/webview';
  import { toast } from '@profidev/pleiades/components/util/general';
  import { cn } from '@profidev/pleiades/utils';
  import { content_add_file } from '$lib/tauri/content.svelte';
  import {
    accepts,
    error_message,
    KIND_WORD,
    type ListKind
  } from './content-list';

  interface Props {
    kind: ListKind;
    profile: string;
  }

  let { kind, profile }: Props = $props();

  let hover = $state(false);
  let ext = $derived(kind === 'mod' ? '.jar' : '.zip');

  const add = async (paths: string[]) => {
    const files = paths.filter((p) => accepts(kind, p));
    if (files.length < paths.length) {
      toast.warning(`Only ${ext} files can be added here`);
    }
    for (const file of files) {
      const error = await content_add_file(profile, file);
      if (error) {
        toast.error(error_message(error));
      }
    }
  };

  $effect(() => {
    // The whole window is the drop target, the box shows where files go
    const unlisten = getCurrentWebview().onDragDropEvent(({ payload }) => {
      if (payload.type === 'drop') {
        hover = false;
        if (payload.paths.length > 0) {
          void add(payload.paths);
        }
      } else {
        hover = payload.type !== 'leave';
      }
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  });
</script>

<div
  class={cn(
    'text-muted-foreground flex h-16 w-full flex-none items-center justify-center gap-2 rounded-xl border border-dashed text-sm transition-colors',
    hover && 'border-primary bg-primary/10 text-foreground'
  )}
>
  <Upload class="size-5" />
  Drop {ext} files here to add a {KIND_WORD[kind]} manually
</div>
