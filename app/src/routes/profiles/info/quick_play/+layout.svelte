<script lang="ts">
  import { Tabs } from 'positron-components/components/ui';
  import {
    multiplayer_list,
    quick_play_updater,
    realms_list,
    singleplayer_list
  } from './store.svelte';
  import { getProfile } from '../store.svelte';
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';
  import { page } from '$app/state';

  let { children } = $props();

  let singleplayer = $derived(singleplayer_list());
  let multiplayer = $derived(multiplayer_list());
  let realms = $derived(realms_list());
  let profile = $derived(getProfile());

  let quick_play_updater_ = $derived(quick_play_updater(profile));
  let x = $derived(quick_play_updater_?.value);

  let value = $state('singleplayer');

  onMount(() => {
    let route = page.url.pathname;
    if (route.includes('/quick_play/singleplayer')) {
      value = 'singleplayer';
    } else if (route.includes('/quick_play/multiplayer')) {
      value = 'multiplayer';
    } else if (route.includes('/quick_play/realms')) {
      value = 'realms';
    } else {
      value = 'singleplayer';
    }
  });

  $effect(() => {
    if (
      page.url.pathname.includes('/quick_play') &&
      !page.url.pathname.includes(value)
    ) {
      goto(`/profiles/info/quick_play/${value}?id=${profile?.id}`);
    }
  });
</script>

<!-- we need to use x so the updater is active or the icon will not be updated -->
<p class="absolute top-1000 left-1000 hidden">{x}</p>
<div class="flex w-full flex-col gap-2">
  <Tabs.Root bind:value>
    <Tabs.List>
      <Tabs.Trigger
        value="singleplayer"
        onclick={() => {
          goto(`/profiles/info/quick_play/singleplayer?id=${profile?.id}`);
        }}>Singleplayer</Tabs.Trigger
      >
      <Tabs.Trigger
        value="multiplayer"
        onclick={() => {
          goto(`/profiles/info/quick_play/multiplayer?id=${profile?.id}`);
        }}>Multiplayer</Tabs.Trigger
      >
      <Tabs.Trigger
        value="realms"
        onclick={() => {
          goto(`/profiles/info/quick_play/realms?id=${profile?.id}`);
        }}>Realms</Tabs.Trigger
      >
    </Tabs.List>

    {@render children()}
  </Tabs.Root>
</div>
