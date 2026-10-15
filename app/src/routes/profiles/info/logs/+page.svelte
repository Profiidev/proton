<script lang="ts">
  import { create_data_state, UpdateType } from '$lib/data-state.svelte';
  import { getProfile } from '../store.svelte';
  import LogWindow from '$lib/components/LogWindow.svelte';
  import { filter_logs, LOG_LEVELS, parse_logs } from '$lib/components/logs';
  import {
    profile_runs_list,
    profile_logs,
    profile_clear_logs
  } from '$lib/tauri/logs.svelte';
  import { DateTime as D } from '@profidev/pleiades/util/time.svelte';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import { Input } from '@profidev/pleiades/components/ui/input';
  import DestroyDialog from '$lib/components/form/DestroyDialog.svelte';
  // @ts-ignore
  import * as DropdownMenu from '@profidev/pleiades/components/ui/dropdown-menu';
  import { ChevronDown, Search, Trash } from '@lucide/svelte';

  let profile = $derived(getProfile());
  let selected_run = $state<string>();
  let search = $state('');
  let levels = $state<string[]>([]);
  let clearOpen = $state(false);

  let logs_list_updater = $derived(
    profile
      ? create_data_state(async () => {
          return await profile_runs_list(profile.id);
        }, UpdateType.ProfileLogs)
      : undefined
  );
  // newest first, runs are ISO timestamps
  let runs = $derived(logs_list_updater?.value?.toSorted().reverse() ?? []);
  let run = $derived(
    selected_run && runs.includes(selected_run) ? selected_run : runs[0]
  );
  let logs = $state<string[]>([]);
  let lines = $derived(filter_logs(parse_logs(logs), search, levels));

  const format_run = (run: string) =>
    D.DateTime?.fromISO(run)
      .setLocale('de')
      .toLocaleString(D.DateTime.DATETIME_SHORT);

  $effect(() => {
    if (!run || !profile) {
      logs = [];
      return;
    }
    let stale = false;
    profile_logs(profile.id, run).then((newLogs) => {
      if (!stale) {
        logs = newLogs ?? [];
      }
    });
    return () => {
      stale = true;
    };
  });
</script>

<div class="flex min-w-0 flex-1 flex-col gap-2.5">
  <div class="flex flex-wrap gap-2">
    <div class="relative flex-[1_1_220px]">
      <Search
        class="text-muted-foreground pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2"
      />
      <Input
        type="search"
        placeholder="Search logs"
        aria-label="Search logs"
        class="pl-8"
        bind:value={search}
      />
    </div>
    <DropdownMenu.Root>
      <DropdownMenu.Trigger>
        {#snippet child({ props }: { props: Record<string, any> })}
          <Button variant="outline" {...props}>
            {levels.length ? levels.join(', ') : 'All levels'}
            <ChevronDown data-icon="inline-end" />
          </Button>
        {/snippet}
      </DropdownMenu.Trigger>
      <DropdownMenu.Content align="end">
        {#each LOG_LEVELS as level (level)}
          <DropdownMenu.CheckboxItem
            closeOnSelect={false}
            checked={levels.includes(level)}
            onCheckedChange={(checked: boolean) =>
              (levels = checked
                ? LOG_LEVELS.filter((l) => l === level || levels.includes(l))
                : levels.filter((l) => l !== level))}
          >
            {level}
          </DropdownMenu.CheckboxItem>
        {/each}
      </DropdownMenu.Content>
    </DropdownMenu.Root>
    <DropdownMenu.Root>
      <DropdownMenu.Trigger>
        {#snippet child({ props }: { props: Record<string, any> })}
          <Button variant="outline" {...props}>
            {run ? `Run: ${format_run(run)}` : 'No runs'}
            <ChevronDown data-icon="inline-end" />
          </Button>
        {/snippet}
      </DropdownMenu.Trigger>
      <DropdownMenu.Content align="end" class="max-h-80">
        <DropdownMenu.RadioGroup
          bind:value={() => run, (v) => (selected_run = v)}
        >
          {#each runs as r (r)}
            <DropdownMenu.RadioItem value={r}
              >{format_run(r)}</DropdownMenu.RadioItem
            >
          {/each}
        </DropdownMenu.RadioGroup>
        {#if runs.length}
          <DropdownMenu.Separator />
        {/if}
        <DropdownMenu.Item
          variant="destructive"
          onclick={() => (clearOpen = true)}
        >
          <Trash />
          Clear runs
        </DropdownMenu.Item>
      </DropdownMenu.Content>
    </DropdownMenu.Root>
  </div>
  <LogWindow {lines} source class="grow" />
</div>
<DestroyDialog
  bind:open={clearOpen}
  title="Clear Logs"
  description="Are you sure you want to clear the logs of this profile?"
  btnText="Clear"
  onclick={() => {
    profile_clear_logs(profile!.id);
    clearOpen = false;
  }}
/>
