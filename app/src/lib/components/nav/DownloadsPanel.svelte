<script lang="ts">
  import { Download, X } from '@lucide/svelte';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import * as Popover from '@profidev/pleiades/components/ui/popover';
  import { Progress } from '@profidev/pleiades/components/ui/progress';
  import { Separator } from '@profidev/pleiades/components/ui/separator';
  import {
    chip_label,
    detail,
    percent,
    percent_label,
    running_count,
    running_percent,
    speed_label
  } from '$lib/downloads';
  import {
    download_cancel,
    downloads,
    downloads_clear_finished
  } from '$lib/tauri/downloads.svelte';

  let list = $derived(downloads.list);
  let running = $derived(running_count(list));
</script>

{#if list.length > 0}
  <Popover.Root>
    <Popover.Trigger
      class="bg-muted mr-2 flex h-7 items-center gap-2 rounded-full px-2.5 text-xs font-medium"
      aria-label="Open downloads"
    >
      <Download class="size-3.5" />
      <span>{chip_label(running)}</span>
      {#if running > 0}
        <Progress value={running_percent(list)} class="w-10" />
      {/if}
    </Popover.Trigger>
    <Popover.Content
      align="end"
      class="w-100 gap-0 overflow-hidden p-0"
      aria-label="Downloads"
    >
      <div class="flex items-center gap-2 px-4 py-3">
        <h2 class="flex-1 text-base font-medium">Downloads</h2>
        <Button
          variant="ghost"
          size="xs"
          disabled={running === list.length}
          onclick={downloads_clear_finished}>Clear finished</Button
        >
      </div>
      <Separator />
      <div class="flex max-h-96 flex-col overflow-y-auto">
        {#each list as d (d.id)}
          <div class="flex flex-col gap-2 border-b px-4 py-3">
            <div class="flex items-center gap-2">
              <span class="flex-1 truncate font-medium">{d.title}</span>
              <span class="text-muted-foreground text-xs"
                >{percent_label(d)}</span
              >
              {#if d.status === 'running'}
                <Button
                  variant="ghost"
                  size="icon-xs"
                  aria-label="Cancel download"
                  disabled={d.cancel_requested}
                  onclick={() => download_cancel(d.id)}
                >
                  <X />
                </Button>
              {/if}
            </div>
            <Progress value={percent(d)} />
            <div
              class="text-muted-foreground flex justify-between gap-2 text-xs"
            >
              <span class="truncate">{detail(d)}</span>
              <span class="shrink-0">{speed_label(d)}</span>
            </div>
          </div>
        {/each}
      </div>
      <p class="text-muted-foreground px-4 py-2.5 text-xs">
        Downloads continue while you browse. Games start as soon as their files
        are ready.
      </p>
    </Popover.Content>
  </Popover.Root>
{/if}
