<script lang="ts">
    import { SkinViewer } from "skinview3d";
    import { onMount } from "svelte";

  interface Props {
    skin: string;
  }

  let { skin }: Props = $props();

  let canvas: HTMLCanvasElement | undefined = $state();
  let viewer: SkinViewer | undefined = $state();

  onMount(() => {
    viewer = new SkinViewer({
      canvas,
      width: 160,
      height: 220,
      zoom: 0.8,
      skin: `data:image/png;base64, ${skin}`
    });

    viewer.controls.enableZoom = false;
  })
</script>

<canvas bind:this={canvas} class="border w-40 h-55 select-none"></canvas>
