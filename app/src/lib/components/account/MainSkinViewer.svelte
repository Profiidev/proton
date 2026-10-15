<script lang="ts">
  import {
    account_active,
    account_get_cape,
    account_get_skin,
    account_list,
    State
  } from '$lib/tauri/account.svelte';
  import { LoaderCircle } from '@lucide/svelte';
  import { Label } from '@profidev/pleiades/components/ui/label';
  import * as NativeSelect from '@profidev/pleiades/components/ui/native-select';
  import {
    IdleAnimation,
    RunningAnimation,
    SkinViewer,
    WalkingAnimation,
    WaveAnimation,
    type BackEquipment
  } from 'skinview3d';
  import { onMount } from 'svelte';

  let accounts = $derived(account_list.value);
  let active_account = $derived(account_active.value);
  let account = $derived(active_account ? accounts?.[active_account] : null);
  let cape_url = $derived(
    account?.capes.find((c) => c.state === State.Active)?.url
  );

  let canvas: HTMLCanvasElement | undefined = $state();
  let viewer: SkinViewer | undefined = $state();
  let animation = $state('idle');
  let back: BackEquipment | 'none' = $state('cape');

  const animations = {
    none: () => null,
    idle: () => new IdleAnimation(),
    walking: () => new WalkingAnimation(),
    running: () => new RunningAnimation(),
    waving: () => new WaveAnimation()
  };

  $effect(() => {
    if (viewer) {
      viewer.animation =
        animations[animation as keyof typeof animations]?.() ?? null;
    }
  });

  let skin_url = $derived(
    account?.skins.find((s) => s.state === State.Active)?.url
  );

  $effect(() => {
    let url = skin_url;
    if (viewer && url) {
      account_get_skin(url).then((data) => {
        if (data) viewer?.loadSkin(`data:image/png;base64, ${data.data}`);
      });
    }
  });

  $effect(() => {
    if (!viewer) return;
    if (!cape_url || back === 'none') {
      viewer.loadCape(null);
      return;
    }
    let backEquipment = back;
    account_get_cape(cape_url).then((data) => {
      if (data) {
        viewer?.loadCape(`data:image/png;base64, ${data.data}`, {
          backEquipment
        });
      }
    });
  });

  onMount(() => {
    let v: SkinViewer | undefined;
    const timeout = setTimeout(() => {
      v = new SkinViewer({ canvas, width: 212, height: 280, zoom: 0.8 });
      v.controls.enableZoom = false;
      viewer = v;
    });
    return () => {
      clearTimeout(timeout);
      v?.dispose();
    };
  });
</script>

<div
  class="bg-card ring-foreground/10 flex w-60 shrink-0 flex-col gap-3 rounded-xl p-3.5 ring-1"
>
  <div class="bg-muted relative h-70 rounded-lg">
    <canvas bind:this={canvas} class="h-70 w-full select-none"></canvas>
    {#if !viewer}
      <div class="absolute inset-0 flex items-center justify-center">
        <LoaderCircle class="size-10 animate-spin" />
      </div>
    {/if}
  </div>
  <div class="flex flex-col gap-1.5">
    <Label for="skin-animation">Animation</Label>
    <NativeSelect.Root
      id="skin-animation"
      class="w-full"
      bind:value={animation}
    >
      <NativeSelect.Option value="none">None</NativeSelect.Option>
      <NativeSelect.Option value="idle">Idle</NativeSelect.Option>
      <NativeSelect.Option value="walking">Walking</NativeSelect.Option>
      <NativeSelect.Option value="running">Running</NativeSelect.Option>
      <NativeSelect.Option value="waving">Waving</NativeSelect.Option>
    </NativeSelect.Root>
  </div>
  <div class="flex flex-col gap-1.5">
    <Label for="skin-back">Back equipment</Label>
    <NativeSelect.Root
      id="skin-back"
      class="w-full"
      bind:value={back}
      disabled={!cape_url}
    >
      <NativeSelect.Option value="none">None</NativeSelect.Option>
      <NativeSelect.Option value="cape">Cape</NativeSelect.Option>
      <NativeSelect.Option value="elytra">Elytra</NativeSelect.Option>
    </NativeSelect.Root>
  </div>
</div>
