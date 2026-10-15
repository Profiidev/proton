<script lang="ts">
  import * as Tabs from '@profidev/pleiades/components/ui/tabs';
  import {
    multiplayer_list,
    quick_play_updater,
    singleplayer_list
  } from './store.svelte';
  import { getProfile } from '../store.svelte';
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import { show_note } from './quick-play';

  let { children } = $props();

  let profile = $derived(getProfile());

  let quick_play_updater_ = $derived(quick_play_updater(profile));
  let x = $derived(quick_play_updater_?.value);

  const tabs = [
    { value: 'singleplayer', label: 'Singleplayer' },
    { value: 'multiplayer', label: 'Multiplayer' },
    { value: 'realms', label: 'Realms' }
  ];

  let value = $derived(
    tabs.find((t) => page.url.pathname.includes(`/quick_play/${t.value}`))
      ?.value ?? 'singleplayer'
  );
</script>

<!-- we need to use x so the updater is active or the icon will not be updated -->
<p class="absolute top-1000 left-1000 hidden">{x}</p>
<div class="flex w-full min-w-0 flex-col gap-3">
  <Tabs.Root bind:value>
    <Tabs.List>
      {#each tabs as tab (tab.value)}
        <Tabs.Trigger
          value={tab.value}
          onclick={() =>
            goto(`/profiles/info/quick_play/${tab.value}?id=${profile?.id}`)}
        >
          {tab.label}
        </Tabs.Trigger>
      {/each}
    </Tabs.List>

    {@render children()}
  </Tabs.Root>
  {#if show_note(singleplayer_list(), multiplayer_list())}
    <p class="text-muted-foreground text-xs">
      Quick play needs Minecraft 1.20 (23w14a) or newer. Join a world once to
      add it here.
    </p>
  {/if}
</div>
