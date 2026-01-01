<script lang="ts">
  import {
    account_active,
    account_get_cape,
    account_get_skin,
    account_list,
    State
  } from '$lib/tauri/account.svelte';
  import { LoaderCircle } from '@lucide/svelte';
  import { Label } from 'positron-components/components/ui/label';
  import * as Select from 'positron-components/components/ui/select';
  import {
    IdleAnimation,
    type PlayerAnimation,
    SkinViewer,
    WalkingAnimation,
    WaveAnimation,
    type BackEquipment
  } from 'skinview3d';
  import { onMount } from 'svelte';

  interface Props {
    flipped?: boolean;
  }

  let { flipped }: Props = $props();

  let accounts = $derived(account_list.value);
  let active_account = $derived(account_active.value);
  let account = $derived(
    accounts && Object.entries(accounts).find((a) => a[0] === active_account)
  );

  let canvas: HTMLCanvasElement | undefined = $state();
  let mainViewer: SkinViewer | undefined = $state();
  let animation: string = $state('Idle');
  let cape_option: string = $state('Cape');

  let animations: {
    key: string;
    value: PlayerAnimation;
  }[] = [
    {
      key: 'Idle',
      value: new IdleAnimation()
    },
    {
      key: 'Walking',
      value: new WalkingAnimation()
    },
    {
      key: 'Waving',
      value: new WaveAnimation()
    }
  ];

  let cape_options: string[] = ['None', 'Cape', 'Elytra'];

  $effect(() => {
    animation;
    if (mainViewer) {
      mainViewer.animation = animations.find((a) => a.key === animation)!.value;
    }
  });

  $effect(() => {
    account;
    mainViewer;
    if (mainViewer && account && account[1]) {
      let url = account[1].skins.find((s) => s.state === State.Active)?.url;
      if (url) {
        account_get_skin(url).then((data) => {
          if (data && mainViewer) {
            mainViewer.loadSkin(`data:image/png;base64, ${data.data}`);
          }
        });
      }
    }
  });

  $effect(() => {
    account;
    mainViewer;
    cape_option;
    if (mainViewer && account && account[1]) {
      let url = account[1].capes.find((s) => s.state === State.Active)?.url;
      if (url) {
        account_get_cape(url).then((data) => {
          if (data && mainViewer) {
            let backEquipment: BackEquipment | undefined;

            switch (cape_option) {
              case 'None':
                mainViewer.loadCape(null);
                break;
              case 'Elytra':
                backEquipment = 'elytra';
              case 'Cape':
                mainViewer.loadCape(`data:image/png;base64, ${data.data}`, {
                  backEquipment
                });
                break;
            }
          }
        });
      } else {
        mainViewer.loadCape(null);
      }
    }
  });

  const init = () => {
    mainViewer = new SkinViewer({
      canvas,
      width: 280,
      height: 400,
      zoom: 0.8
    });

    mainViewer.controls.enableZoom = false;
    if (flipped) {
      mainViewer.camera.position.z = -50;
    }
  };

  onMount(() => {
    setTimeout(init);
  });
</script>

<div class="flex h-full w-70 flex-col">
  <p class="my-2 w-full text-center">Active</p>
  <div class="relative h-100 w-full">
    <canvas bind:this={canvas} class="h-100 w-full select-none"></canvas>
    {#if !mainViewer}
      <div
        class="absolute top-0 left-0 flex h-100 w-full items-center justify-center"
      >
        <LoaderCircle class="size-16 animate-spin" />
      </div>
    {/if}
  </div>
  <p class="w-full text-center">Display Options</p>
  <Label class="my-2">Animation</Label>
  <Select.Root bind:value={animation} type="single">
    <Select.Trigger class="w-full">{animation}</Select.Trigger>
    <Select.Content>
      {#each animations as animation}
        <Select.Item value={animation.key} label={animation.key}
          >{animation.key}</Select.Item
        >
      {/each}
    </Select.Content>
  </Select.Root>
  {#if account?.[1]?.capes.find((c) => c.state === State.Active)}
    <Label class="my-2">Back Equipment</Label>
    <Select.Root bind:value={cape_option} type="single">
      <Select.Trigger class="w-full">{cape_option}</Select.Trigger>
      <Select.Content>
        {#each cape_options as option}
          <Select.Item value={option} label={option}>{option}</Select.Item>
        {/each}
      </Select.Content>
    </Select.Root>
  {/if}
</div>
