<script lang="ts">
  import { Skeleton } from "positron-components/components/ui";
  import { SkinViewer } from "skinview3d";
  import { onMount } from "svelte";

  interface Props {
    skin: string;
  }

  let { skin }: Props = $props();

  let canvas: HTMLCanvasElement | undefined = $state();
  let viewer: SkinViewer | undefined = $state();

  const init = () => {
    viewer = new SkinViewer({
      canvas,
      width: 160,
      height: 220,
      zoom: 0.8,
      skin: `data:image/png;base64, ${skin}`,
    });

    viewer.controls.enableZoom = false;
  };

  onMount(() => {
    setTimeout(init);
  });
</script>

<div class="w-40 h-55 relative">
  <canvas bind:this={canvas} class="border w-40 h-55 select-none"></canvas>
  {#if !viewer}
    <Skeleton class="w-40 h-55 absolute top-0 left-0" />
  {/if}
</div>
