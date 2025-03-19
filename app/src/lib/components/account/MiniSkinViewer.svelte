<script lang="ts">
  import {
    account_list_skins,
    account_remove_skin,
  } from "$lib/tauri/account.svelte";
  import { Check, Trash } from "lucide-svelte";
  import {
    Badge,
    Button,
    Skeleton,
    toast,
  } from "positron-components/components/ui";
  import { SkinViewer } from "skinview3d";
  import { onMount } from "svelte";

  interface Props {
    id: string;
    skin: string;
    selected: boolean;
  }

  let { id, skin, selected }: Props = $props();

  let canvas: HTMLCanvasElement | undefined = $state();
  let viewer: SkinViewer | undefined = $state();

  const init = () => {
    viewer = new SkinViewer({
      canvas,
      width: 148,
      height: 220,
      zoom: 0.8,
      skin: `data:image/png;base64, ${skin}`,
    });

    viewer.controls.enableZoom = false;
  };

  onMount(() => {
    setTimeout(init);
  });

  const remove = async () => {
    if (await account_remove_skin(id)) {
      await account_list_skins.update();
      toast.success("Successfully removed Skin");
    } else {
      toast.error("Failed to remove Skin");
    }
  };
</script>

<div class="relative">
  <div class="w-37 h-55 relative">
    <canvas bind:this={canvas} class="w-37 h-55 select-none"></canvas>
    {#if !viewer}
      <Skeleton class="w-37 h-55 absolute top-0 left-0" />
    {/if}
  </div>
  <div class="flex absolute w-full top-0 p-2 justify-between">
    <Button size="icon" class="size-6" disabled={selected}>
      <Check />
    </Button>
    <Button size="icon" class="size-6" variant="destructive" onclick={remove}>
      <Trash />
    </Button>
  </div>
  {#if selected}
    <div class="flex justify-center w-full bottom-0 absolute">
      <Badge>Selected</Badge>
    </div>
  {/if}
</div>
