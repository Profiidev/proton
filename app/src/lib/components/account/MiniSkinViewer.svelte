<script lang="ts">
  import { account_remove_skin } from '$lib/tauri/account.svelte';
  import { LoaderCircle, Trash2 } from '@lucide/svelte';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import { toast } from '@profidev/pleiades/components/util/general';
  import { cn } from '@profidev/pleiades/utils';
  import { SkinViewer } from 'skinview3d';
  import { onMount } from 'svelte';

  interface Props {
    id: string;
    label: string;
    skin: string;
    selected: boolean;
    change_fn: (id: string) => Promise<void>;
    cape?: string;
    flipped?: boolean;
    delete_disabled?: boolean;
    height?: number;
  }

  let {
    id,
    label,
    skin,
    selected,
    change_fn,
    cape,
    flipped,
    delete_disabled,
    height = 110
  }: Props = $props();

  let canvas: HTMLCanvasElement | undefined = $state();
  let viewer: SkinViewer | undefined = $state();
  let width = $state(0);
  let change_loading = $state(false);

  $effect(() => {
    if (viewer && width) viewer.setSize(width, height);
  });

  // Textures load in place, so a new skin or cape never rebuilds the viewer
  $effect(() => {
    viewer?.loadSkin(`data:image/png;base64, ${skin}`);
  });

  $effect(() => {
    if (cape) {
      viewer?.loadCape(`data:image/png;base64, ${cape}`);
    } else {
      viewer?.loadCape(null);
    }
  });

  onMount(() => {
    let v: SkinViewer | undefined;
    const timeout = setTimeout(() => {
      v = new SkinViewer({
        canvas,
        width: width || 100,
        height,
        zoom: 0.8
      });
      // the whole tile is the select button, so no orbit dragging
      v.controls.enabled = false;
      if (flipped) v.camera.position.z = -50;
      viewer = v;
    });
    return () => {
      clearTimeout(timeout);
      v?.dispose();
    };
  });

  const remove = async () => {
    if (!(await account_remove_skin(id))) {
      toast.success('Successfully removed Skin');
    } else {
      toast.error('Failed to remove Skin');
    }
  };

  const change = async () => {
    if (selected || change_loading) return;
    change_loading = true;
    await change_fn(id);
    change_loading = false;
  };
</script>

<div class="group relative">
  <button
    class={cn(
      'bg-input/30 hover:bg-input/60 flex w-full flex-col rounded-lg border p-2',
      selected ? 'border-primary' : 'border-input'
    )}
    aria-label={label}
    aria-pressed={selected}
    onclick={change}
  >
    <span
      class="bg-muted relative block w-full rounded-lg"
      style:height="{height}px"
      bind:clientWidth={width}
    >
      <canvas bind:this={canvas} class="size-full select-none"></canvas>
      {#if !viewer || change_loading}
        <span class="absolute inset-0 flex items-center justify-center">
          <LoaderCircle class="size-8 animate-spin" />
        </span>
      {/if}
    </span>
  </button>
  {#if !delete_disabled && !selected}
    <Button
      variant="ghost"
      size="icon-sm"
      class="absolute top-3 right-3 opacity-0 group-focus-within:opacity-100 group-hover:opacity-100"
      aria-label="Remove skin"
      onclick={remove}
    >
      <Trash2 class="text-destructive" />
    </Button>
  {/if}
</div>
