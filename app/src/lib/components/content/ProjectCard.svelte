<script lang="ts">
  import { Check, Download } from '@lucide/svelte';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import { DateTime as D } from '@profidev/pleiades/util/time.svelte';
  import type { CardAction } from '$lib/content';
  import type { ProjectSummary } from '$lib/tauri/content.svelte';
  import ProjectIcon from './ProjectIcon.svelte';
  import {
    SOURCE_LABEL,
    format_count,
    project_href,
    runs_on,
    tag_label
  } from './discover';

  interface Props {
    project: ProjectSummary;
    action: CardAction;
    oninstall: () => void;
  }

  let { project, action, oninstall }: Props = $props();

  let href = $derived(project_href(project));
  let tags = $derived([
    ...new Set(
      [
        ...project.categories.map(tag_label),
        ...(project.kind === 'mod' || project.kind === 'modpack'
          ? project.loaders.map(tag_label)
          : []),
        runs_on(project)
      ].filter((t) => t !== undefined)
    )
  ]);
</script>

<article
  class="bg-card ring-foreground/10 flex items-center gap-4 rounded-xl px-4 py-3.5 ring-1"
>
  <a {href} class="shrink-0" tabindex="-1" aria-hidden="true">
    <ProjectIcon url={project.iconUrl} name={project.name} class="size-18" />
  </a>
  <div class="flex min-w-0 flex-1 flex-col gap-1">
    <div class="flex min-w-0 items-baseline gap-2">
      <a {href} class="truncate text-base font-medium hover:underline">
        {project.name}
      </a>
      <span class="text-muted-foreground truncate text-xs">
        {SOURCE_LABEL[project.source]} · {project.author}
      </span>
    </div>
    <p class="text-muted-foreground line-clamp-2 text-sm">
      {project.description}
    </p>
    <div class="mt-0.5 flex flex-wrap gap-1.5">
      {#each tags as tag (tag)}
        <span
          class="bg-secondary inline-flex h-5 items-center rounded-full px-2 text-xs font-medium"
        >
          {tag}
        </span>
      {/each}
    </div>
  </div>
  <div class="flex w-32 flex-none flex-col items-end gap-2">
    <span class="text-muted-foreground flex items-center gap-1 text-xs">
      <Download class="size-3.5" />
      {format_count(project.downloads)}
    </span>
    <span class="text-muted-foreground text-xs">
      Updated {D.DateTime?.fromISO(project.updatedAt).toRelative()}
    </span>
    {#if action === 'installed'}
      <Button variant="secondary" size="sm" class="w-full" disabled>
        <Check data-icon="inline-start" />
        Installed
      </Button>
    {:else}
      <Button
        variant={action === 'update' ? 'outline' : 'default'}
        size="sm"
        class="w-full"
        onclick={oninstall}
      >
        {action === 'update' ? 'Update' : 'Install'}
      </Button>
    {/if}
  </div>
</article>
