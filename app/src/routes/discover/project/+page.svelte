<script lang="ts">
  import { Check, ExternalLink } from '@lucide/svelte';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import * as Empty from '@profidev/pleiades/components/ui/empty';
  import { Label } from '@profidev/pleiades/components/ui/label';
  import { Separator } from '@profidev/pleiades/components/ui/separator';
  import { Switch } from '@profidev/pleiades/components/ui/switch';
  import * as Table from '@profidev/pleiades/components/ui/table';
  import * as Tabs from '@profidev/pleiades/components/ui/tabs';
  import { DateTime as D } from '@profidev/pleiades/util/time.svelte';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import InstallDialog from '$lib/components/content/InstallDialog.svelte';
  import ModpackInstallDialog from '$lib/components/content/ModpackInstallDialog.svelte';
  import ProjectIcon from '$lib/components/content/ProjectIcon.svelte';
  import {
    SOURCE_LABEL,
    project_href,
    runs_on,
    short_list,
    sort_versions,
    tag_label
  } from '$lib/components/content/discover';
  import { pick_target } from '$lib/content';
  import { content_target } from '$lib/content-target.svelte';
  import { project_titles } from '$lib/project-titles.svelte';
  import {
    type ProjectDetail,
    type VersionInfo,
    content_project,
    content_updates,
    content_versions,
    installed_state
  } from '$lib/tauri/content.svelte';
  import { profile_list } from '$lib/tauri/profile.svelte';
  import { vanilla_version_list } from '$lib/tauri/versions.svelte';

  let { data } = $props();

  const chip =
    'inline-flex h-5 items-center gap-1 rounded-full px-2 text-xs font-medium';
  const card =
    'bg-card ring-foreground/10 flex flex-col gap-3 rounded-xl p-4 text-sm ring-1';

  let is_pack = $derived(data.kind === 'modpack');
  let profiles = $derived(profile_list.value ?? []);
  let target = $derived(
    is_pack ? undefined : pick_target(profiles, content_target.id)
  );
  let known_versions = $derived(vanilla_version_list.value);

  let detail = $state<ProjectDetail>();
  let failed = $state(false);
  $effect(() => {
    const { source, id } = data;
    let stale = false;
    failed = false;
    content_project(source, id).then((d) => {
      if (stale) {
        return;
      }
      detail = d;
      failed = d === undefined;
      if (d) {
        project_titles[id] = d.summary.name;
      }
    });
    return () => (stale = true);
  });
  let summary = $derived(detail?.summary);

  // Versions for the target profile, all versions only when asked for
  let show_all = $state(false);
  let compat_versions = $state<VersionInfo[]>();
  let all_versions = $state<VersionInfo[]>();
  let target_id = $derived(target?.id);
  $effect(() => {
    const { source, id } = data;
    const profile = target_id;
    let stale = false;
    compat_versions = undefined;
    if (profile) {
      content_versions(source, id, profile).then((v) => {
        if (!stale) {
          compat_versions = v ?? [];
        }
      });
    }
    return () => (stale = true);
  });
  $effect(() => {
    const { source, id } = data;
    let stale = false;
    if (show_all || is_pack) {
      content_versions(source, id).then((v) => {
        if (!stale) {
          all_versions = v ?? [];
        }
      });
    }
    return () => (stale = true);
  });
  let filtered = $derived(!is_pack && !show_all);
  let versions = $derived(filtered ? compat_versions : all_versions);

  let installed = $derived(
    target ? installed_state(target.id, data.kind) : undefined
  );
  let installed_mods = $derived(
    target && data.kind !== 'mod'
      ? installed_state(target.id, 'mod')
      : installed
  );
  let entry = $derived(installed?.value?.find((i) => i.projectId === data.id));
  let update = $derived(
    target
      ? content_updates.value?.[target.id]?.find((u) => u.projectId === data.id)
      : undefined
  );

  // Required dependencies of the newest version for the target
  let needs = $state<{ id: string; name: string }[]>([]);
  let required = $derived(
    (compat_versions?.[0]?.dependencies ?? [])
      .filter((d) => d.kind === 'required')
      .map((d) => d.projectId)
  );
  $effect(() => {
    const ids = required;
    const { source } = data;
    let stale = false;
    Promise.all(ids.map(async (id) => content_project(source, id))).then(
      (projects) => {
        if (!stale) {
          needs = ids.map((id, i) => ({
            id,
            name: projects[i]?.summary.name ?? id
          }));
        }
      }
    );
    return () => (stale = true);
  });
  let installed_ids = $derived(
    new Set((installed_mods?.value ?? []).map((i) => i.projectId))
  );

  // Other profiles with this project, only checked once the page loaded
  let others = $derived(
    detail && !is_pack
      ? profiles
          .filter((p) => p.id !== target?.id)
          .map((p) => ({ profile: p, state: installed_state(p.id, data.kind) }))
      : []
  );
  let also_installed = $derived(
    others.flatMap(({ profile, state }) => {
      const found = state.value?.find((i) => i.projectId === data.id);
      return found
        ? [
            {
              id: profile.id,
              name: profile.name,
              version: found.versionNumber ?? ''
            }
          ]
        : [];
    })
  );

  let dialog_open = $state(false);
  let dialog_version = $state<string>();
  const install = (version?: string) => {
    dialog_version = version;
    dialog_open = true;
  };

  const relative = (iso: string) => D.DateTime?.fromISO(iso).toRelative();

  // Links in the description open in the browser, not in the app
  const open_link = (e: MouseEvent) => {
    if (!(e.target instanceof Element)) {
      return;
    }
    const link = e.target.closest('a');
    if (link?.href.startsWith('http')) {
      e.preventDefault();
      openUrl(link.href);
    }
  };
</script>

<div class="flex flex-col gap-4 px-5 pt-4 pb-6">
  {#if summary && detail}
    <header class="flex shrink-0 flex-wrap items-center gap-4">
      <ProjectIcon
        url={summary.iconUrl}
        name={summary.name}
        class="size-22 rounded-xl"
      />
      <div class="flex min-w-0 flex-[1_1_320px] flex-col gap-1.5">
        <div class="flex items-center gap-2">
          <h1 class="truncate text-xl font-semibold tracking-tight">
            {summary.name}
          </h1>
          {#if update}
            <span class={[chip, 'bg-amber-500/15 text-amber-500']}>
              Update available
            </span>
          {/if}
        </div>
        <p class="text-muted-foreground max-w-140 text-sm">
          {summary.description}
        </p>
        <div class="flex flex-wrap gap-1.5">
          {#each new Set( [...summary.categories, ...summary.loaders] ) as tag (tag)}
            <span class={[chip, 'bg-secondary']}>{tag_label(tag)}</span>
          {/each}
        </div>
      </div>
      <div class="flex w-55 flex-col items-stretch gap-2">
        {#if update}
          <Button onclick={() => install(update.latest.id)}>
            Update to {update.latest.versionNumber}
          </Button>
        {:else if entry}
          <Button variant="secondary" disabled>
            <Check data-icon="inline-start" />
            Installed
          </Button>
        {:else}
          <Button onclick={() => install()} disabled={!is_pack && !target}>
            Install
          </Button>
        {/if}
        {#if entry && target}
          <span class="text-muted-foreground text-center text-xs">
            Installed in {target.name}: {entry.versionNumber ?? entry.fileName}
          </span>
        {/if}
      </div>
    </header>

    <div class="flex flex-wrap items-start gap-5">
      <Tabs.Root value="description" class="min-w-0 flex-[999_1_520px]">
        <Tabs.List>
          <Tabs.Trigger value="description">Description</Tabs.Trigger>
          <Tabs.Trigger value="versions">Versions</Tabs.Trigger>
          <Tabs.Trigger value="gallery">Gallery</Tabs.Trigger>
        </Tabs.List>
        <Tabs.Content value="description" class="pt-3">
          <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
          <div class="prose-content max-w-160" onclick={open_link}>
            <!-- eslint-disable-next-line svelte/no-at-html-tags -- sanitized by the backend -->
            {@html detail.body}
          </div>
        </Tabs.Content>
        <Tabs.Content value="versions" class="flex flex-col gap-3 pt-2">
          {#if !is_pack && target}
            <Label class="font-normal">
              <Switch bind:checked={show_all} />
              Show all versions, not only those for {target.name}
            </Label>
          {/if}
          {#if versions && versions.length > 0}
            <div
              class="bg-card ring-foreground/10 overflow-hidden rounded-xl ring-1"
            >
              <Table.Root>
                <Table.Header>
                  <Table.Row>
                    <Table.Head>Version</Table.Head>
                    <Table.Head>Channel</Table.Head>
                    <Table.Head>Game version</Table.Head>
                    <Table.Head>Loaders</Table.Head>
                    <Table.Head>Published</Table.Head>
                    <Table.Head class="text-right">Action</Table.Head>
                  </Table.Row>
                </Table.Header>
                <Table.Body>
                  {#each versions as v (v.id)}
                    <Table.Row>
                      <Table.Cell class="font-medium">
                        {v.versionNumber}
                      </Table.Cell>
                      <Table.Cell>{tag_label(v.channel)}</Table.Cell>
                      <Table.Cell>
                        {short_list(
                          sort_versions(v.gameVersions, known_versions),
                          2
                        )}
                      </Table.Cell>
                      <Table.Cell>
                        {v.loaders.map(tag_label).join(', ')}
                      </Table.Cell>
                      <Table.Cell>{relative(v.publishedAt)}</Table.Cell>
                      <Table.Cell class="text-right">
                        {#if entry && v.id === entry.versionId}
                          <span
                            class={[chip, 'bg-green-500/15 text-green-500']}
                          >
                            Installed
                          </span>
                        {:else}
                          <Button
                            variant="outline"
                            size="sm"
                            onclick={() => install(v.id)}
                          >
                            Install
                          </Button>
                        {/if}
                      </Table.Cell>
                    </Table.Row>
                  {/each}
                </Table.Body>
              </Table.Root>
            </div>
          {:else if versions}
            <Empty.Root class="w-full flex-none border border-dashed">
              <Empty.Header>
                <Empty.Title>No versions</Empty.Title>
                <Empty.Description>
                  {filtered && target
                    ? `None made for ${target.loader} ${target.version}. Show all versions to see the rest.`
                    : 'This project has no files to install.'}
                </Empty.Description>
              </Empty.Header>
            </Empty.Root>
          {/if}
        </Tabs.Content>
        <Tabs.Content value="gallery" class="pt-3">
          {#if detail.gallery.length > 0}
            <div
              class="grid grid-cols-[repeat(auto-fill,minmax(240px,1fr))] gap-2"
            >
              {#each detail.gallery as image (image.url)}
                <figure class="flex flex-col gap-1">
                  <img
                    src={image.url}
                    alt={image.title ?? summary.name}
                    loading="lazy"
                    class="bg-muted aspect-video w-full rounded-lg object-cover"
                  />
                  {#if image.title}
                    <figcaption class="text-muted-foreground truncate text-xs">
                      {image.title}
                    </figcaption>
                  {/if}
                </figure>
              {/each}
            </div>
          {:else}
            <Empty.Root class="w-full flex-none border border-dashed">
              <Empty.Header>
                <Empty.Title>No screenshots</Empty.Title>
                <Empty.Description>
                  This project has no gallery images.
                </Empty.Description>
              </Empty.Header>
            </Empty.Root>
          {/if}
        </Tabs.Content>
      </Tabs.Root>

      <aside class="flex flex-[0_0_260px] flex-col gap-3">
        <div class={card}>
          <span class="font-medium">Compatibility</span>
          {#if summary.gameVersions.length > 0}
            {@render kv(
              'Game version',
              short_list(sort_versions(summary.gameVersions, known_versions))
            )}
          {/if}
          {#if summary.loaders.length > 0}
            {@render kv('Loaders', summary.loaders.map(tag_label).join(', '))}
          {/if}
          {#if runs_on(summary)}
            {@render kv('Runs on', runs_on(summary) ?? '')}
          {/if}
          <div class="flex items-center justify-between gap-3">
            <span class="text-muted-foreground">Source</span>
            <button
              class="flex items-center gap-1 hover:underline"
              onclick={() => openUrl(summary.url)}
            >
              {SOURCE_LABEL[summary.source]}
              <ExternalLink class="size-3.5" />
            </button>
          </div>
          {#if needs.length > 0}
            <Separator />
            <span class="font-medium">Needs</span>
            {#each needs as need (need.id)}
              <div class="flex items-center justify-between gap-3">
                <a
                  class="truncate hover:underline"
                  href={project_href({
                    id: need.id,
                    kind: 'mod',
                    source: summary.source
                  })}
                >
                  {need.name}
                </a>
                {#if installed_ids.has(need.id)}
                  <span class={[chip, 'bg-green-500/15 text-green-500']}>
                    Installed
                  </span>
                {/if}
              </div>
            {/each}
          {/if}
        </div>
        {#if also_installed.length > 0}
          <div class={card}>
            <span class="font-medium">Also installed from this page</span>
            {#each also_installed as other (other.id)}
              {@render kv(other.name, other.version)}
            {/each}
          </div>
        {/if}
      </aside>
    </div>
  {:else if failed}
    <Empty.Root class="w-full flex-none border border-dashed">
      <Empty.Header>
        <Empty.Title>Project not found</Empty.Title>
        <Empty.Description>
          It may have been removed, or the source is not reachable right now.
        </Empty.Description>
      </Empty.Header>
    </Empty.Root>
  {/if}
</div>

{#if summary}
  {#if is_pack}
    <ModpackInstallDialog bind:open={dialog_open} project={summary} />
  {:else}
    <InstallDialog
      bind:open={dialog_open}
      project={summary}
      versionId={dialog_version}
    />
  {/if}
{/if}

{#snippet kv(key: string, value: string)}
  <div class="flex items-center justify-between gap-3">
    <span class="text-muted-foreground shrink-0">{key}</span>
    <span class="truncate text-right">{value}</span>
  </div>
{/snippet}
