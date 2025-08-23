<script lang="ts">
  import { Tabs } from 'positron-components/components/ui';
  import { getProfile } from '../store.svelte';
  import { goto } from '$app/navigation';
  import { page } from '$app/state';

  let { children } = $props();

  let profile = $derived(getProfile());
  let value = $state('general');

  $effect(() => {
    let route = page.url.pathname;
    if (route.includes('/settings/general')) {
      value = 'general';
    } else if (route.includes('/settings/minecraft')) {
      value = 'minecraft';
    } else if (route.includes('/settings/java')) {
      value = 'java';
    } else {
      value = 'general';
    }
  });
</script>

<div class="flex w-full flex-col gap-2">
  <Tabs.Root bind:value>
    <Tabs.List>
      <Tabs.Trigger
        value="general"
        onclick={() => {
          goto(`/profiles/info/settings/general?id=${profile?.id}`);
        }}>General</Tabs.Trigger
      >
      <Tabs.Trigger
        value="minecraft"
        onclick={() => {
          goto(`/profiles/info/settings/minecraft?id=${profile?.id}`);
        }}>Minecraft</Tabs.Trigger
      >
      <Tabs.Trigger
        value="java"
        onclick={() => {
          goto(`/profiles/info/settings/java?id=${profile?.id}`);
        }}>Java</Tabs.Trigger
      >
    </Tabs.List>

    <div class="mx-2">
      {@render children()}
    </div>
  </Tabs.Root>
</div>
