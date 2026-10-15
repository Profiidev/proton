<script lang="ts">
  import { Input } from '@profidev/pleiades/components/ui/input';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import { LoaderCircle, Search } from '@lucide/svelte';
  import ProjectIcon from '$lib/components/content/ProjectIcon.svelte';
  import {
    content_search,
    content_sources,
    type ProjectSummary
  } from '$lib/tauri/content.svelte';
  import { debounce } from '$lib/util.svelte';

  interface Props {
    onselect: (project: ProjectSummary) => void;
  }

  let { onselect }: Props = $props();

  let text = $state('');
  let query = $state('');
  let hits = $state<ProjectSummary[]>();
  let failed = $state(false);

  const set_query = debounce((value: string) => (query = value.trim()), 300);
  const downloads = new Intl.NumberFormat('en', { notation: 'compact' });

  $effect(() => {
    const sources = content_sources.value;
    if (!sources) return;
    let stale = false;
    const _ = content_search({
      categories: [],
      kind: 'modpack',
      limit: 10,
      loaders: [],
      offset: 0,
      query,
      sort: query ? 'relevance' : 'downloads',
      sources
    }).then((res) => {
      if (stale) return;
      hits = res?.hits;
      failed = !res;
    });
    return () => (stale = true);
  });
</script>

<div class="flex min-w-0 flex-col gap-3 pt-4">
  <div class="relative">
    <Search
      class="text-muted-foreground pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2"
    />
    <Input
      type="search"
      placeholder="Search modpacks"
      aria-label="Search modpacks"
      class="pl-8"
      bind:value={text}
      oninput={() => set_query(text)}
    />
  </div>
  <div class="flex h-80 flex-col gap-1 overflow-y-auto">
    {#if failed}
      <p class="text-muted-foreground m-auto text-sm">
        Search failed. Check your connection and try again.
      </p>
    {:else if !hits}
      <LoaderCircle class="text-muted-foreground m-auto animate-spin" />
    {:else if hits.length === 0}
      <p class="text-muted-foreground m-auto text-sm">No modpacks found.</p>
    {:else}
      {#each hits as hit (`${hit.source}-${hit.id}`)}
        <div class="hover:bg-accent/50 flex items-center gap-3 rounded-lg p-2">
          <ProjectIcon url={hit.iconUrl} name={hit.name} class="size-9" />
          <div class="flex min-w-0 flex-1 flex-col">
            <span class="truncate text-sm font-medium">{hit.name}</span>
            <span class="text-muted-foreground truncate text-xs capitalize">
              {[...hit.loaders, hit.gameVersions.at(-1)]
                .filter(Boolean)
                .join(' · ')}
            </span>
          </div>
          <span class="text-muted-foreground shrink-0 text-xs">
            {downloads.format(hit.downloads)} downloads
          </span>
          <Button size="sm" variant="outline" onclick={() => onselect(hit)}>
            Select
          </Button>
        </div>
      {/each}
    {/if}
  </div>
</div>
