<script lang="ts">
  import { Button } from '@profidev/pleiades/components/ui/button';
  import { Progress } from '@profidev/pleiades/components/ui/progress';
  import type { InstanceInfo } from '$lib/tauri/instance.svelte';
  import { settings_get } from '$lib/tauri/settings.svelte';
  import { format_duration } from '$lib/instances';
  import { instance_subtitle, memory_usage } from './instance';

  interface Props {
    instance: InstanceInfo;
    number: number;
    /** Current time in ms, refreshed by the page's poll tick */
    now: number;
    /** Standalone card (profile tab) instead of a list row */
    card?: boolean;
    onstop: () => void;
  }

  let { instance, number, now, card = false, onstop }: Props = $props();

  let memory = $derived(
    memory_usage(instance.memory_used, settings_get.value?.system_max_mem)
  );
  let subtitle = $derived(
    instance_subtitle(
      format_duration(now - Date.parse(instance.launched_at)),
      instance.account_name,
      card
    )
  );
</script>

<div
  class={[
    'flex flex-wrap items-center gap-2',
    card
      ? 'bg-card ring-foreground/10 rounded-xl px-4 py-3.5 ring-1'
      : 'hover:bg-accent/40 border-b px-3.5 py-3 last:border-0'
  ]}
>
  <span class="size-2 shrink-0 rounded-full bg-green-500"></span>
  <div class="flex min-w-0 flex-[1_1_180px] flex-col">
    <span class={['font-semibold', card && 'text-[15px]']}>
      Instance {number}
    </span>
    <span class="text-muted-foreground text-xs">{subtitle}</span>
  </div>
  {#if memory}
    <div class="mr-1 flex w-45 flex-col gap-1.5">
      <div class="flex items-center justify-between gap-3 text-xs">
        <span class="text-muted-foreground">Memory</span>
        <span>{memory.label}</span>
      </div>
      <Progress value={memory.percent} />
    </div>
  {/if}
  <Button variant="outline" href={`/instances/info/logs?id=${instance.id}`}>
    Console
  </Button>
  <Button variant="destructive" onclick={onstop}>Stop</Button>
</div>
