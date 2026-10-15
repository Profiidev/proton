<script lang="ts">
  import { cn } from '@profidev/pleiades/utils';
  import { LEVEL_COLORS, type LogEntry } from './logs';

  interface Props {
    lines: LogEntry[];
    /** Show the source column, otherwise the source prefixes the message */
    source?: boolean;
    /** Keep scrolled to the newest line, turns off when scrolling up */
    follow?: boolean;
    class?: string;
  }

  let {
    lines,
    source = false,
    follow = $bindable(true),
    class: className
  }: Props = $props();

  let el = $state<HTMLElement>();

  $effect(() => {
    lines;
    if (follow && el) {
      el.scrollTop = el.scrollHeight;
    }
  });
</script>

<div
  bind:this={el}
  onscroll={() => {
    if (el) follow = el.scrollHeight - el.scrollTop <= el.clientHeight + 1;
  }}
  class={cn(
    'ring-foreground/10 min-h-[420px] overflow-auto rounded-xl bg-[color-mix(in_oklab,var(--background)_60%,black)] px-3.5 py-3 font-mono text-[12.5px] leading-5 ring-1',
    className
  )}
>
  {#each lines as line, i (i)}
    {#if line.continuation}
      <div class="w-max whitespace-pre">{line.message}</div>
    {:else}
      <div class="flex w-max gap-2.5 whitespace-nowrap">
        <span class="text-muted-foreground">{line.time}</span>
        <span
          class={cn('w-[42px] shrink-0 font-medium', LEVEL_COLORS[line.level])}
          >{line.level}</span
        >
        {#if source}
          <span class="text-muted-foreground"
            >{line.location}{line.subLocation
              ? ` (${line.subLocation})`
              : ''}</span
          >
          <span>{line.message}</span>
        {:else}
          <span>{line.location ? `${line.location}: ` : ''}{line.message}</span>
        {/if}
      </div>
    {/if}
  {:else}
    <p class="text-muted-foreground font-sans text-sm">
      No logs found for the current filter settings
    </p>
  {/each}
</div>
