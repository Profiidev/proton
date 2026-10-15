<script lang="ts">
  import * as Tabs from '@profidev/pleiades/components/ui/tabs';
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import { getProfile } from '../store.svelte';
  import { installed } from '$lib/components/content/installed';
  import { KIND_ROUTE } from '$lib/content';
  import type { ListKind } from '$lib/components/content/content-list';

  let { children } = $props();

  let profile = $derived(getProfile());

  const tabs: { kind: ListKind; label: string }[] = [
    { kind: 'mod', label: 'Mods' },
    { kind: 'resourcepack', label: 'Resource packs' },
    { kind: 'shader', label: 'Shaders' }
  ];

  let value = $derived(
    tabs.find((t) =>
      page.url.pathname.includes(`/content/${KIND_ROUTE[t.kind]}`)
    )?.kind ?? 'mod'
  );
</script>

<div class="flex w-full min-w-0 flex-col gap-3">
  <Tabs.Root {value}>
    <Tabs.List>
      {#each tabs as tab (tab.kind)}
        {@const count = profile
          ? installed(profile.id, tab.kind).value?.length
          : undefined}
        <Tabs.Trigger
          value={tab.kind}
          onclick={() =>
            goto(
              `/profiles/info/content/${KIND_ROUTE[tab.kind]}?id=${profile?.id}`
            )}
        >
          {tab.label}
          {#if count !== undefined}
            <span
              class="bg-secondary inline-flex h-5 items-center rounded-full px-2 text-xs font-medium"
            >
              {count}
            </span>
          {/if}
        </Tabs.Trigger>
      {/each}
    </Tabs.List>
  </Tabs.Root>
  {@render children()}
</div>
