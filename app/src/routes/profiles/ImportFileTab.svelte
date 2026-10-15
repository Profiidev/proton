<script lang="ts">
  import { Button } from '@profidev/pleiades/components/ui/button';
  import { cn } from '@profidev/pleiades/utils';
  import { FileUp, LoaderCircle } from '@lucide/svelte';
  import { getCurrentWebview } from '@tauri-apps/api/webview';
  import { open } from '@tauri-apps/plugin-dialog';
  import {
    ContentError,
    modpack_plan,
    type PackPlan
  } from '$lib/tauri/content.svelte';

  interface Props {
    onplan: (plan: PackPlan) => void;
  }

  let { onplan }: Props = $props();

  let dragging = $state(false);
  let busy = $state(false);
  let error = $state<string>();

  const load = async (path: string) => {
    busy = true;
    error = undefined;
    const res = await modpack_plan({ path, type: 'path' });
    busy = false;
    if (typeof res !== 'string') {
      onplan(res);
    } else if (res === ContentError.InvalidPack) {
      error = 'This file is not a supported modpack';
    } else {
      error = 'Failed to read the modpack file';
    }
  };

  const choose = async () => {
    const path = await open({
      filters: [{ extensions: ['mrpack', 'zip'], name: 'Modpack' }]
    });
    if (path) {
      await load(path);
    }
  };

  $effect(() => {
    const unlisten = getCurrentWebview().onDragDropEvent((e) => {
      const { type } = e.payload;
      dragging = type === 'enter' || type === 'over';
      const [path] = type === 'drop' ? e.payload.paths : [];
      if (!path || busy) return;
      if (/\.(mrpack|zip)$/i.test(path)) {
        const _ = load(path);
      } else {
        error = 'Only .mrpack and .zip files can be imported';
      }
    });
    return () => {
      const _ = unlisten.then((f) => f());
    };
  });
</script>

<div class="flex min-w-0 flex-col gap-3 pt-4">
  <div
    class={cn(
      'flex h-48 w-full flex-none flex-col items-center justify-center gap-3 rounded-xl border border-dashed p-6 text-center transition-colors',
      dragging && 'border-primary bg-primary/10'
    )}
  >
    {#if busy}
      <LoaderCircle class="text-muted-foreground size-6 animate-spin" />
      <p class="text-muted-foreground text-sm">Reading modpack…</p>
    {:else}
      <FileUp class="text-muted-foreground size-6" />
      <p class="text-muted-foreground text-sm">
        Drop an .mrpack or .zip modpack file here
      </p>
      <Button variant="outline" onclick={choose}>Choose file</Button>
    {/if}
  </div>
  {#if error}
    <p class="text-destructive text-sm" role="alert">{error}</p>
  {/if}
</div>
