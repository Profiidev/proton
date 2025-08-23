<script lang="ts">
  import { Tabs } from 'positron-components/components/ui';
  import { quick_play_updater } from './store.svelte';
  import { getProfile } from '../store.svelte';
  import { goto } from '$app/navigation';
  import { page } from '$app/state';

  let { children } = $props();

  let profile = $derived(getProfile());

  let quick_play_updater_ = $derived(quick_play_updater(profile));
  let x = $derived(quick_play_updater_?.value);

  let value = $state('singleplayer');

  $effect(() => {
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
