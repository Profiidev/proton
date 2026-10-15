<script lang="ts">
  import { Button } from '@profidev/pleiades/components/ui/button';
  import { Check, ExternalLink } from '@lucide/svelte';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { open } from '@tauri-apps/plugin-dialog';
  import {
    ContentError,
    content_adopt_file,
    type ManualFile
  } from '$lib/tauri/content.svelte';

  interface Props {
    files: ManualFile[];
    /** Profile the picked files go into, without one only the download pages are offered */
    profile?: string;
    /** File ids already adopted */
    adopted?: string[];
  }

  let { files, profile, adopted = $bindable([]) }: Props = $props();

  let errors = $state<Record<string, string>>({});

  const adopt = async (file: ManualFile) => {
    if (!profile) return;
    const path = await open({
      filters: [
        {
          extensions: [file.fileName.split('.').pop() ?? '*'],
          name: file.fileName
        }
      ],
      title: `Select ${file.fileName}`
    });
    if (!path) return;
    const error = await content_adopt_file(profile, file, path);
    if (error === undefined) {
      delete errors[file.fileId];
      adopted = [...adopted, file.fileId];
    } else {
      errors[file.fileId] =
        error === ContentError.HashMismatch
          ? 'This is not the expected file, check that you downloaded the right version'
          : 'Failed to add the file';
    }
  };
</script>

<div class="bg-card/50 flex flex-col overflow-hidden rounded-xl border">
  {#each files as file (file.fileId)}
    <div
      class="flex flex-wrap items-center gap-3 border-b px-3 py-2.5 last:border-0"
    >
      <div class="flex min-w-0 flex-1 flex-col">
        <span class="truncate font-medium">{file.fileName}</span>
        {#if errors[file.fileId]}
          <span class="text-destructive text-xs">{errors[file.fileId]}</span>
        {/if}
      </div>
      {#if adopted.includes(file.fileId)}
        <span
          class="inline-flex h-5 items-center gap-1 rounded-full bg-green-500/15 px-2 text-xs font-medium text-green-500"
        >
          <Check class="size-3" />
          Added
        </span>
      {:else}
        <Button
          size="sm"
          variant="outline"
          onclick={() => openUrl(file.downloadUrl)}
        >
          <ExternalLink />
          Open download page
        </Button>
        {#if profile}
          <Button size="sm" onclick={() => adopt(file)}>I downloaded it</Button>
        {/if}
      {/if}
    </div>
  {/each}
</div>
