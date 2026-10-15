<script lang="ts">
  import { Button } from '@profidev/pleiades/components/ui/button';
  import { Check, Download, LoaderCircle } from '@lucide/svelte';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import {
    content_find_manual,
    type ManualFile
  } from '$lib/tauri/content.svelte';
  import { settings_get } from '$lib/tauri/settings.svelte';

  interface Props {
    files: ManualFile[];
    /** File id -> path of the downloaded file, filled while the download folders are watched */
    found?: Record<string, string>;
  }

  let { files, found = $bindable({}) }: Props = $props();

  let missing = $derived(files.filter((f) => !(f.fileId in found)));
  let dirs = $derived(settings_get.value?.download_dirs ?? []);

  // ponytail: polls every 2s, a fs watcher if this ever shows up in profiles
  $effect(() => {
    if (missing.length === 0) return;
    const look = missing;
    let stale = false;
    const check = async () => {
      const result = await content_find_manual(look);
      if (stale || typeof result === 'string') return;
      if (Object.keys(result).length > 0) found = { ...found, ...result };
    };
    void check();
    const timer = setInterval(check, 2000);
    return () => {
      stale = true;
      clearInterval(timer);
    };
  });
</script>

<div class="flex flex-col gap-1.5">
  <span class="text-muted-foreground text-xs">
    {#if dirs.length > 0}
      Watching {dirs.join(', ')}. Change the folders in Settings, Minecraft.
    {:else}
      No download folders set, add one in Settings, Minecraft.
    {/if}
  </span>
  <div
    class="bg-card/50 flex max-h-64 shrink-0 flex-col overflow-y-auto rounded-xl border"
  >
    {#each files as file (file.fileId)}
      <div
        class="flex flex-wrap items-center gap-3 border-b px-3 py-2.5 last:border-0"
      >
        <span class="min-w-0 flex-1 truncate font-medium">{file.fileName}</span>
        {#if file.fileId in found}
          <span
            class="inline-flex h-5 items-center gap-1 rounded-full bg-green-500/15 px-2 text-xs font-medium text-green-500"
          >
            <Check class="size-3" />
            Found
          </span>
        {:else}
          <span
            class="text-muted-foreground inline-flex items-center gap-1 text-xs"
          >
            <LoaderCircle class="size-3 animate-spin" />
            Waiting
          </span>
          <Button
            size="sm"
            variant="outline"
            onclick={() => openUrl(file.downloadUrl)}
          >
            <Download />
            Download
          </Button>
        {/if}
      </div>
    {/each}
  </div>
</div>
