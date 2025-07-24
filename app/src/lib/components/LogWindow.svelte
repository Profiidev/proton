<script lang="ts">
  import { Input, ScrollArea } from 'positron-components/components/ui';
  import { Multiselect } from 'positron-components/components/table';
  import Fuse from 'fuse.js';
  import { cn } from 'positron-components/utils';
  import type { Snippet } from 'svelte';

  interface Props {
    logs: string[] | undefined;
    class?: string;
    children?: Snippet;
  }

  let { logs, class: className, children }: Props = $props();

  interface LogEntry {
    time: string;
    level: string;
    location: string;
    message: string;
  }

  const parseLine = (line: string): LogEntry => {
    //format: [timestamp] [level] message
    const match = line.match(/^\[(.+?)\] \[(.+?)\/(.+?)\]: (.+)$/);
    if (match) {
      const [, time, location, level, message] = match;
      return {
        time,
        level,
        message,
        location
      };
    }
    return {
      time: '',
      level: '',
      location: '',
      message: line
    };
  };

  const levelColors: Record<string, string> = {
    INFO: 'text-green-500',
    WARN: 'text-yellow-500',
    ERROR: 'text-red-500',
    DEBUG: 'text-blue-500'
  };

  let text_filter = $state('');
  let level_filter = $state<string[]>([]);
  let parsedLogs = $derived(logs?.map(parseLine) ?? []);

  let logsFuse = $derived(
    new Fuse(parsedLogs, {
      keys: [
        { name: 'location', weight: 0.5 },
        { name: 'message', weight: 1 }
      ],
      useExtendedSearch: true,
      threshold: 0.4
    })
  );

  let filteredLogs = $derived(
    (text_filter
      ? logsFuse.search(text_filter).map((result) => result.item)
      : parsedLogs
    ).filter((log) =>
      level_filter.length > 0 ? level_filter.includes(log.level) : true
    )
  );

  let auto_scroll = $state(true);
  let scrollAreaParent = $state<HTMLElement | null>(null);
  let scrollArea = $derived(
    scrollAreaParent?.querySelector('[data-slot="scroll-area-viewport"]')
  );

  $effect(() => {
    filteredLogs;
    if (auto_scroll && scrollArea) {
      scrollArea.scrollTop = scrollArea.scrollHeight;
    }
  });

  let listenerAdded = false;
  $effect(() => {
    if (!scrollArea || listenerAdded) return;

    listenerAdded = true;
    scrollArea.addEventListener('scroll', (e: Event) => {
      const target = e.target as HTMLElement;
      if (target) {
        if (target.scrollHeight - target.scrollTop <= target.clientHeight + 1) {
          auto_scroll = true;
        } else {
          auto_scroll = false;
        }
      }
    });
  });
</script>

<div class={cn('flex h-full w-full flex-col gap-2', className)}>
  <div class="flex gap-2">
    {@render children?.()}
    <Input
      placeholder="Search logs..."
      bind:value={text_filter}
      class="flex-grow-1"
      type="search"
    />
    <Multiselect
      data={Object.keys(levelColors).map((level) => ({
        label: level.charAt(0) + level.slice(1).toLowerCase(),
        value: level
      }))}
      label="Level"
      bind:selected={level_filter}
      buttonPrefix="Filter"
      class="w-35"
    />
  </div>
  <div class="min-h-0 w-full flex-grow-1 rounded-lg border-2 p-2">
    {#if parsedLogs && filteredLogs.length > 0}
      <ScrollArea.ScrollArea class="size-full" bind:ref={scrollAreaParent}>
        {#each filteredLogs as log}
          <p>
            <span class="text-muted-foreground">{log.time}</span>
            <span class={levelColors[log.level]}>{log.level}</span>
            <span class="text-muted-foreground">{log.location}</span>
            <span>: {log.message}</span>
          </p>
        {/each}
      </ScrollArea.ScrollArea>
    {:else}
      <p class="text-muted-foreground">
        No logs found for the current filter settings
      </p>
    {/if}
  </div>
</div>
