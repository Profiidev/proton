<script lang="ts">
  import * as Tabs from '@profidev/pleiades/components/ui/tabs';
  import { getProfile } from '../store.svelte';
  import { goto } from '$app/navigation';
  import { page } from '$app/state';

  let { children } = $props();

  let profile = $derived(getProfile());
  let value = $derived(
    ['minecraft', 'java'].find((tab) =>
      page.url.pathname.includes(`/settings/${tab}`)
    ) ?? 'general'
  );
</script>

<div class="flex w-full max-w-180 flex-col gap-4 overflow-y-auto">
  <Tabs.Root
    {value}
    onValueChange={(tab) =>
      goto(`/profiles/info/settings/${tab}?id=${profile?.id}`)}
  >
    <Tabs.List>
      <Tabs.Trigger value="general">General</Tabs.Trigger>
      <Tabs.Trigger value="minecraft">Minecraft</Tabs.Trigger>
      <Tabs.Trigger value="java">Java</Tabs.Trigger>
    </Tabs.List>
  </Tabs.Root>
  {@render children()}
</div>
