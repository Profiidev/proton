<script lang="ts">
  import {
    CircleArrowUp,
    Ellipsis,
    ExternalLink,
    FolderOpen,
    GripVertical,
    Plus,
    RefreshCw,
    Search,
    Trash
  } from '@lucide/svelte';
  import { goto } from '$app/navigation';
  import * as Alert from '@profidev/pleiades/components/ui/alert';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import { Checkbox } from '@profidev/pleiades/components/ui/checkbox';
  // @ts-ignore
  import * as DropdownMenu from '@profidev/pleiades/components/ui/dropdown-menu';
  import { Input } from '@profidev/pleiades/components/ui/input';
  import * as RadioGroup from '@profidev/pleiades/components/ui/radio-group';
  import * as Select from '@profidev/pleiades/components/ui/select';
  import { Spinner } from '@profidev/pleiades/components/ui/spinner';
  import { Switch } from '@profidev/pleiades/components/ui/switch';
  import { toast } from '@profidev/pleiades/components/util/general';
  import { cn } from '@profidev/pleiades/utils';
  import DestroyDialog from '$lib/components/form/DestroyDialog.svelte';
  import {
    blocking_dependents,
    filter_rows,
    KIND_ROUTE,
    move,
    type RowStatus
  } from '$lib/content';
  import { content_target } from '$lib/content-target.svelte';
  import { plural } from '$lib/profiles';
  import {
    content_check_updates,
    content_remove,
    content_set_active_shader,
    content_set_enabled,
    content_set_order,
    content_update,
    content_updates,
    type ContentError,
    type Installed
  } from '$lib/tauri/content.svelte';
  import { profile_open_path, type Profile } from '$lib/tauri/profile.svelte';
  import { installed } from './installed';
  import ContentDropZone from './ContentDropZone.svelte';
  import ProjectIcon from './ProjectIcon.svelte';
  import {
    error_message,
    KIND_WORD,
    sort_rows,
    SOURCE_LABEL,
    type ListKind
  } from './content-list';

  interface Props {
    kind: ListKind;
    profile: Profile;
  }

  let { kind, profile }: Props = $props();

  const STATUS: { value: RowStatus; label: string }[] = [
    { value: 'all', label: 'All status' },
    { value: 'enabled', label: 'Enabled' },
    { value: 'disabled', label: 'Disabled' },
    { value: 'updates', label: 'Updates' }
  ];

  let content = $derived(installed(profile.id, kind));
  let rows = $derived(sort_rows(kind, content.value ?? []));
  let updates = $derived(
    new Map(
      (content_updates.value?.[profile.id] ?? [])
        .filter((u) => u.kind === kind)
        .map((u) => [u.fileName, u])
    )
  );

  let search = $state('');
  let status = $state<RowStatus>('all');
  let visible = $derived(
    filter_rows(rows, search, status, new Set(updates.keys()))
  );

  let selected = $state<string[]>([]);
  let chosen = $derived(rows.filter((r) => selected.includes(r.fileName)));
  let all_selected = $derived(
    visible.length > 0 && visible.every((r) => selected.includes(r.fileName))
  );

  let checking = $state(false);
  let dragging = $state<string>();
  let removeOpen = $state(false);
  let removing = $state<Installed[]>([]);
  let remove_text = $state('');

  let word = $derived(KIND_WORD[kind]);
  let toggles = $derived(kind !== 'shader');
  let active = $derived(rows.find((r) => r.active)?.fileName ?? '');

  const report = (error?: ContentError) => {
    if (error) {
      toast.error(error_message(error));
    }
  };

  const select = (file: string, on: boolean) => {
    selected = on ? [...selected, file] : selected.filter((f) => f !== file);
  };

  const check_updates = async () => {
    checking = true;
    const result = await content_check_updates(profile.id);
    checking = false;
    if (typeof result === 'string') {
      report(result);
    } else {
      content_updates.update();
    }
  };

  const update = (files: string[] | null) =>
    content_update(profile.id, profile.name, files);

  const set_enabled = async (entries: Installed[], enabled: boolean) => {
    for (const entry of entries) {
      report(
        await content_set_enabled(profile.id, kind, entry.fileName, enabled)
      );
    }
    content.update();
  };

  const remove = async (entries: Installed[]) => {
    const files = entries.map((e) => e.fileName);
    report(await content_remove(profile.id, kind, files));
    selected = selected.filter((f) => !files.includes(f));
  };

  /** Confirms first when dependents or manual files are affected */
  const ask_remove = (entries: Installed[]) => {
    const blockers = [
      ...new Set(
        entries
          .flatMap((e) => blocking_dependents(e, rows))
          .filter((d) => !entries.includes(d))
          .map((d) => d.name ?? d.fileName)
      )
    ];
    const manual = entries.filter((e) => e.manual).map((e) => e.fileName);
    if (blockers.length === 0 && manual.length === 0) {
      void remove(entries);
      return;
    }
    remove_text = [
      blockers.length > 0 &&
        `Still needed by ${blockers.join(', ')}, which may stop working.`,
      manual.length > 0 &&
        `${manual.join(', ')} ${manual.length === 1 ? 'was' : 'were'} added manually and cannot be re-downloaded.`
    ]
      .filter(Boolean)
      .join(' ');
    removing = entries;
    removeOpen = true;
  };

  const drop_on = async (target: Installed) => {
    const order = rows.filter((r) => r.enabled).map((r) => r.fileName);
    const from = order.indexOf(dragging ?? '');
    const to = order.indexOf(target.fileName);
    dragging = undefined;
    if (from >= 0 && to >= 0 && from !== to) {
      report(await content_set_order(profile.id, move(order, from, to)));
    }
  };

  const set_shader = async (file: string) => {
    const error = await content_set_active_shader(profile.id, file || null);
    report(error);
    if (error) {
      content.update();
    }
  };

  const add = () => {
    content_target.id = profile.id;
    goto(`/discover/${KIND_ROUTE[kind]}`);
  };
</script>

<div class="flex min-h-0 min-w-0 flex-1 flex-col gap-3">
  <div class="flex flex-wrap items-center gap-2">
    <div class="relative flex-[1_1_220px]">
      <Search
        class="text-muted-foreground pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2"
      />
      <Input
        type="search"
        placeholder={`Search installed ${word}s`}
        aria-label={`Search installed ${word}s`}
        class="pl-8"
        bind:value={search}
      />
    </div>
    {#if toggles}
      <Select.Root type="single" bind:value={status}>
        <Select.Trigger aria-label="Status" class="w-36">
          {STATUS.find((s) => s.value === status)?.label}
        </Select.Trigger>
        <Select.Content>
          {#each STATUS as s (s.value)}
            <Select.Item value={s.value} label={s.label} />
          {/each}
        </Select.Content>
      </Select.Root>
    {/if}
    <Button variant="outline" disabled={checking} onclick={check_updates}>
      {#if checking}
        <Spinner data-icon="inline-start" />
      {:else}
        <RefreshCw data-icon="inline-start" />
      {/if}
      Check for updates
    </Button>
    <Button onclick={add}>
      <Plus data-icon="inline-start" />
      Add {word}s
    </Button>
  </div>

  {#if updates.size > 0}
    <Alert.Root>
      <CircleArrowUp />
      <Alert.Title>
        {plural(updates.size, word)}
        can be updated
      </Alert.Title>
      <Alert.Description>
        Newer versions compatible with {profile.loader}
        {profile.version} are available.
      </Alert.Description>
      <Alert.Action>
        <Button size="sm" onclick={() => update(null)}>Update all</Button>
      </Alert.Action>
    </Alert.Root>
  {/if}

  {#if chosen.length > 0}
    <div class="flex flex-wrap items-center gap-2">
      <span class="text-muted-foreground flex-1 text-sm">
        {chosen.length} selected
      </span>
      {#if chosen.some((r) => updates.has(r.fileName))}
        <Button
          variant="outline"
          onclick={() =>
            update(chosen.map((r) => r.fileName).filter((f) => updates.has(f)))}
        >
          Update
        </Button>
      {/if}
      {#if toggles}
        <Button variant="outline" onclick={() => set_enabled(chosen, true)}>
          Enable
        </Button>
        <Button variant="outline" onclick={() => set_enabled(chosen, false)}>
          Disable
        </Button>
      {/if}
      <Button
        variant="outline"
        class="text-destructive"
        onclick={() => ask_remove(chosen)}
      >
        <Trash data-icon="inline-start" />
        Remove
      </Button>
    </div>
  {/if}

  {#if kind === 'shader'}
    <RadioGroup.Root
      value={active}
      onValueChange={set_shader}
      class="bg-card/50 flex min-h-0 flex-col gap-0 overflow-hidden rounded-xl border"
    >
      {@render list()}
    </RadioGroup.Root>
  {:else}
    <div
      class="bg-card/50 flex min-h-0 flex-col overflow-hidden rounded-xl border"
    >
      {@render list()}
    </div>
  {/if}

  <ContentDropZone {kind} profile={profile.id} />
</div>

{#snippet list()}
  <div class="bg-muted/40 flex h-9 shrink-0 items-center gap-3 border-b px-3">
    {#if kind === 'resourcepack'}
      <span class="size-4 shrink-0"></span>
    {/if}
    <Checkbox
      aria-label={`Select all ${word}s`}
      checked={all_selected}
      indeterminate={!all_selected && chosen.length > 0}
      onCheckedChange={(on) =>
        (selected = on ? visible.map((r) => r.fileName) : [])}
    />
    <span class="text-muted-foreground flex-1 pl-13 text-xs">
      {plural(visible.length, word)}
    </span>
    <span class="text-muted-foreground w-36 text-xs max-md:hidden">
      Version
    </span>
    <span class="text-muted-foreground w-26 text-xs max-md:hidden">
      Source
    </span>
    <span class="text-muted-foreground w-32 text-right text-xs">
      {toggles ? 'Enabled' : 'Active'}
    </span>
    <span class="size-7 shrink-0"></span>
  </div>
  <div role="list" class="min-h-0 overflow-y-auto">
    {#each visible as row (row.fileName)}
      {@const next = updates.get(row.fileName)}
      <div
        role="listitem"
        class={cn(
          'hover:bg-accent/40 flex min-h-14 items-center gap-3 border-b px-3 py-2.5 last:border-0',
          dragging === row.fileName && 'opacity-50'
        )}
        ondragover={(e) => {
          if (dragging && row.enabled) {
            e.preventDefault();
          }
        }}
        ondrop={(e) => {
          e.preventDefault();
          void drop_on(row);
        }}
      >
        {#if kind === 'resourcepack'}
          {#if row.enabled}
            <span
              role="button"
              tabindex="-1"
              aria-label="Drag to reorder"
              draggable="true"
              class="text-muted-foreground cursor-grab"
              ondragstart={(e) => {
                dragging = row.fileName;
                e.dataTransfer?.setData('text/plain', row.fileName);
              }}
              ondragend={() => (dragging = undefined)}
            >
              <GripVertical class="size-4" />
            </span>
          {:else}
            <span class="size-4 shrink-0"></span>
          {/if}
        {/if}
        <Checkbox
          aria-label={`Select ${row.name ?? row.fileName}`}
          checked={selected.includes(row.fileName)}
          onCheckedChange={(on) => select(row.fileName, on)}
        />
        <ProjectIcon url={row.iconUrl} name={row.name ?? row.fileName} />
        <div class="flex min-w-0 flex-1 flex-col">
          <span class="flex min-w-0 items-center gap-2">
            <span class="truncate font-medium">{row.name ?? row.fileName}</span>
            {#if row.dependencyOf.length > 0}
              {@render chip('Dependency')}
            {/if}
            {#if row.fromPack}
              {@render chip('From pack')}
            {/if}
          </span>
          <span class="text-muted-foreground truncate text-xs">
            {row.fileName}
          </span>
        </div>
        <span class="flex w-36 flex-col text-xs leading-4 max-md:hidden">
          <span class="truncate">{row.versionNumber ?? '-'}</span>
          {#if next}
            <span class="truncate text-yellow-500">
              Update: {next.latest.versionNumber}
            </span>
          {/if}
        </span>
        <span class="w-26 max-md:hidden">
          {@render chip(row.source ? SOURCE_LABEL[row.source] : 'Manual')}
        </span>
        <span class="flex w-32 items-center justify-end gap-2">
          {#if next}
            <Button
              size="xs"
              variant="outline"
              onclick={() => update([row.fileName])}
            >
              Update
            </Button>
          {/if}
          {#if toggles}
            <Switch
              aria-label={`Enable ${row.name ?? row.fileName}`}
              checked={row.enabled}
              onCheckedChange={(on) => set_enabled([row], on)}
            />
          {:else}
            <RadioGroup.Item
              value={row.fileName}
              aria-label={`Use ${row.name ?? row.fileName}`}
            />
          {/if}
        </span>
        <DropdownMenu.Root>
          <DropdownMenu.Trigger>
            {#snippet child({ props }: { props: Record<string, any> })}
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label="More actions"
                {...props}
              >
                <Ellipsis />
              </Button>
            {/snippet}
          </DropdownMenu.Trigger>
          <DropdownMenu.Content align="end">
            {#if row.projectId && row.source}
              <DropdownMenu.Item
                onclick={() =>
                  goto(
                    `/discover/project?kind=${kind}&source=${row.source}&id=${row.projectId}`
                  )}
              >
                <ExternalLink />
                Open project page
              </DropdownMenu.Item>
            {/if}
            <DropdownMenu.Item onclick={() => profile_open_path(profile.id)}>
              <FolderOpen />
              Show in folder
            </DropdownMenu.Item>
            <DropdownMenu.Item
              variant="destructive"
              onclick={() => ask_remove([row])}
            >
              <Trash />
              Remove
            </DropdownMenu.Item>
          </DropdownMenu.Content>
        </DropdownMenu.Root>
      </div>
    {:else}
      <p class="text-muted-foreground px-3 py-6 text-center text-sm">
        {rows.length === 0
          ? `No ${word}s installed yet`
          : `No ${word}s match the filters`}
      </p>
    {/each}
    {#if kind === 'shader'}
      <div class="flex min-h-14 items-center gap-3 px-3 py-2.5">
        <span class="size-4 shrink-0"></span>
        <span
          class="flex size-10 shrink-0 items-center justify-center rounded-md border border-dashed"
        ></span>
        <span class="flex-1 font-medium">None (shaders off)</span>
        <span class="flex w-32 justify-end">
          <RadioGroup.Item value="" aria-label="Turn shaders off" />
        </span>
        <span class="size-7 shrink-0"></span>
      </div>
    {/if}
  </div>
{/snippet}

{#snippet chip(label: string)}
  <span
    class="bg-secondary inline-flex h-5 shrink-0 items-center rounded-full px-2 text-xs font-medium"
  >
    {label}
  </span>
{/snippet}

<DestroyDialog
  bind:open={removeOpen}
  title={`Remove ${removing.length === 1 ? word : `${word}s`}`}
  description={remove_text}
  btnText="Remove"
  onclick={() => {
    void remove(removing);
    removeOpen = false;
  }}
/>
