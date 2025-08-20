<script lang="ts">
  import { Tabs } from 'positron-components/components/ui';
  import { getProfile } from '../store.svelte';
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';
  import { page } from '$app/state';

  let { children } = $props();

  let profile = $derived(getProfile());
  let value = $state('general');

  onMount(() => {
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

  $effect(() => {
    if (
      page.url.pathname.includes('/settings') &&
      !page.url.pathname.includes(value)
    ) {
      goto(`/profiles/info/settings/${value}?id=${profile?.id}`);
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

    {@render children()}
  </Tabs.Root>
</div>
