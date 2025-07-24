<script lang="ts">
  import { account_remove_skin } from '$lib/tauri/account.svelte';
  import { Check, LoaderCircle, Trash } from '@lucide/svelte';
  import { Badge, Button, toast } from 'positron-components/components/ui';
  import { SkinViewer } from 'skinview3d';
  import { onMount } from 'svelte';

  interface Props {
    id: string;
    skin: string;
    selected: boolean;
    change_fn: (id: string) => Promise<void>;
    cape?: string;
    flipped?: boolean;
    delete_disabled?: boolean;
  }

  let { id, skin, selected, change_fn, cape, flipped, delete_disabled }: Props =
    $props();

  let canvas: HTMLCanvasElement | undefined = $state();
  let viewer: SkinViewer | undefined = $state();
  let change_loading = $state(false);

  const init = () => {
    viewer = new SkinViewer({
      canvas,
      width: 148,
      height: 220,
      zoom: 0.8,
      skin: `data:image/png;base64, ${skin}`
    });

    if (cape) {
      viewer.loadCape(`data:image/png;base64, ${cape}`);
    }

    viewer.controls.enableZoom = false;
    if (flipped) {
      viewer.camera.position.z = -50;
    }
  };

  onMount(() => {
    setTimeout(init);
  });

  const remove = async () => {
    if (!(await account_remove_skin(id))) {
      toast.success('Successfully removed Skin');
    } else {
      toast.error('Failed to remove Skin');
    }
  };

  const change = async () => {
    change_loading = true;

    await change_fn(id);

    change_loading = false;
  };
</script>

<div class="relative">
  <div class="relative h-55 w-37">
    <canvas bind:this={canvas} class="h-55 w-37 select-none"></canvas>
    {#if !viewer}
      <div
        class="absolute top-0 left-0 flex h-55 w-37 items-center justify-center"
      >
        <LoaderCircle class="size-10 animate-spin" />
      </div>
    {/if}
  </div>
  <div class="absolute top-0 flex w-full justify-between p-2">
    <Button
      size="icon"
      class="size-6 cursor-pointer"
      disabled={selected || change_loading}
      onclick={change}
    >
      {#if change_loading}
        <LoaderCircle class="animate-spin" />
      {:else}
        <Check />
      {/if}
    </Button>
    {#if !delete_disabled}
      <Button
        size="icon"
        class="size-6 cursor-pointer"
        variant="destructive"
        onclick={remove}
        disabled={selected}
      >
        <Trash />
      </Button>
    {/if}
  </div>
  {#if selected}
    <div class="absolute bottom-0 flex w-full justify-center">
      <Badge>Selected</Badge>
    </div>
  {/if}
</div>
