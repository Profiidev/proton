<script lang="ts">
  import { Search, TriangleAlert } from '@lucide/svelte';
  import * as Alert from '@profidev/pleiades/components/ui/alert';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import { Checkbox } from '@profidev/pleiades/components/ui/checkbox';
  import * as Empty from '@profidev/pleiades/components/ui/empty';
  import { Input } from '@profidev/pleiades/components/ui/input';
  import { Label } from '@profidev/pleiades/components/ui/label';
  import * as Select from '@profidev/pleiades/components/ui/select';
  import { Separator } from '@profidev/pleiades/components/ui/separator';
  import { Switch } from '@profidev/pleiades/components/ui/switch';
  import VersionSelect from '$lib/components/form/VersionSelect.svelte';
  import {
    type Filters,
    build_query,
    card_action,
    pick_target
  } from '$lib/content';
  import { content_target } from '$lib/content-target.svelte';
  import {
    type Category,
    type ContentKind,
    type Environment,
    type ProjectSummary,
    type SearchSort,
    type Source,
    content_categories,
    content_search,
    content_sources,
    content_updates,
    installed_state
  } from '$lib/tauri/content.svelte';
  import { profile_list } from '$lib/tauri/profile.svelte';
  import { vanilla_version_list } from '$lib/tauri/versions.svelte';
  import { debounce } from '$lib/util.svelte';
  import InstallDialog from './InstallDialog.svelte';
  import ModpackInstallDialog from './ModpackInstallDialog.svelte';
  import ProjectCard from './ProjectCard.svelte';
  import TargetPicker from './TargetPicker.svelte';
  import {
    SOURCE_LABEL,
    category_groups,
    shader_ready,
    tag_label
  } from './discover';

  interface Props {
    kind: ContentKind;
    title: string;
    subtitle: string;
    /** Lowercase name for the search placeholder, e.g. `mods` */
    noun: string;
  }

  let { kind, title, subtitle, noun }: Props = $props();

  const SORTS: Record<SearchSort, string> = {
    downloads: 'Downloads',
    newest: 'Newest',
    relevance: 'Relevance',
    updated: 'Updated'
  };
  const LOADERS = ['fabric', 'quilt', 'forge', 'neoforge'];
  const ANY_VERSION = 'Any version';

  // svelte-ignore state_referenced_locally
  let filters: Filters = $state({
    categories: [],
    compatible: kind === 'mod' || kind === 'resourcepack',
    loaders: [],
    query: '',
    sort: 'relevance',
    sources: ['modrinth', 'curseforge']
  });
  let text = $state('');
  const set_query = debounce((value: string) => (filters.query = value), 300);

  let profiles = $derived(profile_list.value);
  let profiles_loaded = $derived(profiles !== undefined);
  let has_target = $derived(kind !== 'modpack');
  let target = $derived(
    has_target ? pick_target(profiles ?? [], content_target.id) : undefined
  );
  let can_filter_compatible = $derived(
    kind === 'mod' || kind === 'resourcepack'
  );
  let compatible = $derived(
    can_filter_compatible && filters.compatible && target !== undefined
  );

  let sources = $derived(content_sources.value);
  let shown_sources = $derived(
    (['modrinth', 'curseforge'] as Source[]).filter(
      (s) => sources?.includes(s) ?? s === 'modrinth'
    )
  );

  let categories = $state<Category[]>([]);
  $effect(() => {
    let stale = false;
    content_categories(kind).then((c) => {
      if (!stale) {
        categories = c ?? [];
      }
    });
    return () => (stale = true);
  });
  let groups = $derived(category_groups(kind, categories));

  let installed = $derived(
    target ? installed_state(target.id, kind) : undefined
  );
  let installed_mods = $derived(
    target && kind === 'shader' ? installed_state(target.id, 'mod') : undefined
  );
  let installed_ids = $derived(
    new Set(
      (installed?.value ?? []).flatMap((i) =>
        i.projectId ? [i.projectId] : []
      )
    )
  );
  let target_updates = $derived(
    target ? content_updates.value?.[target.id] : undefined
  );

  // Serialized so unrelated profile changes do not search again
  let query_key = $derived(
    JSON.stringify(
      build_query(
        kind,
        {
          ...filters,
          compatible,
          sources: filters.sources.filter((s) => shown_sources.includes(s))
        },
        target
      )
    )
  );

  let hits = $state<ProjectSummary[]>([]);
  let total = $state(0);
  let warnings = $state<string[]>([]);
  let loading = $state(true);
  let failed = $state(false);
  let more_loading = $state(false);

  const search = async (offset: number) =>
    content_search({ ...JSON.parse(query_key), offset });

  $effect(() => {
    // Wait for the profiles, the query depends on the target
    if (!profiles_loaded) {
      return;
    }
    const key = query_key;
    loading = true;
    search(0).then((result) => {
      if (key !== query_key) {
        return;
      }
      loading = false;
      failed = result === undefined;
      hits = result?.hits ?? [];
      total = result?.total ?? 0;
      warnings = result?.warnings ?? [];
    });
  });

  const load_more = async () => {
    const key = query_key;
    more_loading = true;
    const result = await search(hits.length);
    more_loading = false;
    if (key === query_key && result) {
      hits = [...hits, ...result.hits];
      total = result.total;
    }
  };

  const toggle = (list: string[], value: string, on: boolean) =>
    on ? [...list, value] : list.filter((v) => v !== value);

  let dialog_project = $state<ProjectSummary>();
  let dialog_open = $state(false);
</script>

<div class="flex flex-col gap-4 px-5 pt-4 pb-6">
  <div class="flex shrink-0 flex-wrap items-center gap-3">
    <div class="flex-[1_1_240px]">
      <h1 class="text-xl font-semibold tracking-tight">{title}</h1>
      <p class="text-muted-foreground mt-0.5 text-sm">{subtitle}</p>
    </div>
    {#if has_target}
      <TargetPicker {target} profiles={profiles ?? []} />
    {:else}
      <span
        class="bg-primary/15 text-primary inline-flex h-7 items-center rounded-full px-3 text-xs font-medium"
      >
        Creates a new profile
      </span>
    {/if}
  </div>

  <div class="flex flex-wrap gap-2">
    <div class="relative min-w-50 flex-[1_1_280px]">
      <Search
        class="text-muted-foreground pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2"
      />
      <Input
        type="search"
        placeholder={`Search ${noun}`}
        aria-label={`Search ${noun}`}
        class="pl-8"
        bind:value={() => text, (v) => ((text = v), set_query(v))}
      />
    </div>
    <Select.Root type="single" bind:value={filters.sort}>
      <Select.Trigger aria-label="Sort" class="w-44">
        Sort: {SORTS[filters.sort]}
      </Select.Trigger>
      <Select.Content>
        {#each Object.entries(SORTS) as [value, label] (value)}
          <Select.Item {value} {label} />
        {/each}
      </Select.Content>
    </Select.Root>
  </div>

  <div class="flex flex-wrap items-start gap-5">
    <aside
      aria-label="Filters"
      class="bg-card ring-foreground/10 flex flex-[0_0_232px] flex-col gap-4.5 rounded-xl p-4 text-sm ring-1"
    >
      {#if can_filter_compatible}
        <Label class="flex items-start justify-between gap-3">
          <span class="flex flex-col gap-0.5">
            <span class="font-medium">Compatible only</span>
            <span class="text-muted-foreground text-xs font-normal">
              {#if !target}
                No profile selected
              {:else if kind === 'mod'}
                {target.loader} {target.version}, from {target.name}
              {:else}
                Made for {target.version}
              {/if}
            </span>
          </span>
          <Switch
            bind:checked={filters.compatible}
            disabled={!target}
            aria-label="Compatible only"
          />
        </Label>
        <Separator />
      {/if}
      {@render group(
        'Source',
        shown_sources.map((s) => ({
          checked: filters.sources.includes(s),
          label: SOURCE_LABEL[s],
          onchange: (on: boolean) =>
            // oxlint-disable-next-line no-unsafe-type-assertion
            (filters.sources = toggle(filters.sources, s, on) as Source[])
        }))
      )}
      {#if (kind === 'mod' || kind === 'modpack') && !compatible}
        {@render group(
          'Loader',
          LOADERS.map((l) => ({
            checked: filters.loaders.includes(l),
            label: tag_label(l === 'neoforge' ? 'NeoForge' : l),
            onchange: (on: boolean) =>
              (filters.loaders = toggle(filters.loaders, l, on))
          }))
        )}
      {/if}
      {#if kind !== 'shader' && !compatible}
        <div class="flex flex-col gap-1.5">
          <span class="font-medium">Game version</span>
          <VersionSelect
            label="Game version"
            options={[ANY_VERSION, ...(vanilla_version_list.value ?? [])]}
            bind:value={
              () => filters.gameVersion ?? ANY_VERSION,
              (v) => (filters.gameVersion = v === ANY_VERSION ? undefined : v)
            }
          />
        </div>
      {/if}
      {#each groups as g (g.header)}
        {@render group(
          g.label,
          g.names.map((name) => ({
            checked: filters.categories.includes(name),
            label: tag_label(name),
            onchange: (on: boolean) =>
              (filters.categories = toggle(filters.categories, name, on))
          }))
        )}
      {/each}
      {#if kind === 'mod'}
        {@render group(
          'Runs on',
          (['client', 'server'] as Environment[]).map((env) => ({
            checked: filters.environment === env,
            label: tag_label(env),
            onchange: (on: boolean) =>
              (filters.environment = on ? env : undefined)
          }))
        )}
      {/if}
    </aside>

    <section class="flex min-w-0 flex-[999_1_480px] flex-col gap-2.5">
      {#if kind === 'shader' && target}
        <Alert.Root>
          {#if shader_ready(installed_mods?.value ?? [])}
            <Alert.Title>{target.name} is ready for shaders</Alert.Title>
            <Alert.Description>
              Iris Shaders and Sodium are already installed. On profiles without
              them, Proton adds both when you install a shader.
            </Alert.Description>
          {:else}
            <Alert.Title>Shaders need Iris and Sodium</Alert.Title>
            <Alert.Description>
              Proton adds both to {target.name} when you install a shader.
            </Alert.Description>
          {/if}
        </Alert.Root>
      {/if}
      {#each warnings as warning (warning)}
        <Alert.Root>
          <TriangleAlert />
          <Alert.Title>Some results are missing</Alert.Title>
          <Alert.Description>{warning}</Alert.Description>
        </Alert.Root>
      {/each}
      {#if hits.length > 0}
        {#each hits as project (`${project.source}:${project.id}`)}
          <ProjectCard
            {project}
            action={kind === 'modpack'
              ? 'install'
              : card_action(project, installed_ids, target_updates)}
            oninstall={() => {
              dialog_project = project;
              dialog_open = true;
            }}
          />
        {/each}
        {#if hits.length < total}
          <div class="flex justify-center pt-1.5">
            <Button
              variant="outline"
              onclick={load_more}
              disabled={more_loading}
            >
              Load more
            </Button>
          </div>
        {/if}
      {:else if !loading}
        <Empty.Root class="w-full flex-none border border-dashed">
          <Empty.Header>
            <Empty.Title>
              {failed ? 'Search failed' : `No ${noun} found`}
            </Empty.Title>
            <Empty.Description>
              {failed
                ? 'Check your connection and try again.'
                : 'Try other search terms or fewer filters.'}
            </Empty.Description>
          </Empty.Header>
        </Empty.Root>
      {/if}
    </section>
  </div>
</div>

{#if dialog_project}
  {#if kind === 'modpack'}
    <ModpackInstallDialog bind:open={dialog_open} project={dialog_project} />
  {:else}
    <InstallDialog bind:open={dialog_open} project={dialog_project} />
  {/if}
{/if}

{#snippet group(
  label: string,
  options: {
    label: string;
    checked: boolean;
    onchange: (on: boolean) => void;
  }[]
)}
  <div class="flex flex-col gap-1.5">
    <span class="font-medium">{label}</span>
    {#each options as option (option.label)}
      <Label class="font-normal">
        <Checkbox checked={option.checked} onCheckedChange={option.onchange} />
        {option.label}
      </Label>
    {/each}
  </div>
{/snippet}
