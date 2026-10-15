<script lang="ts">
  import { goto } from '$app/navigation';
  import { create_data_state, UpdateType } from '$lib/data-state.svelte';
  import {
    instance_list,
    instance_logs,
    instance_stop
  } from '$lib/tauri/instance.svelte';
  import { group_by_profile } from '$lib/instances';
  import LogWindow from '$lib/components/LogWindow.svelte';
  import {
    CONSOLE_LEVELS,
    filter_logs,
    parse_logs
  } from '$lib/components/logs';
  import DestroyDialog from '$lib/components/form/DestroyDialog.svelte';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import { Input } from '@profidev/pleiades/components/ui/input';
  import { Label } from '@profidev/pleiades/components/ui/label';
  import { Switch } from '@profidev/pleiades/components/ui/switch';
  import * as Select from '@profidev/pleiades/components/ui/select';
  import * as ToggleGroup from '@profidev/pleiades/components/ui/toggle-group';

  let { data } = $props();

  let instances = $derived(instance_list.value);
  let instance = $derived(instances?.find((i) => i.id === data.id));
  let siblings = $derived(
    (instances &&
      instance &&
      group_by_profile(instances).get(instance.profile_id)) ||
      []
  );

  const sibling_label = (i: number) => {
    const account = siblings[i]?.account_name;
    return `Instance ${i + 1}${account ? ` · ${account}` : ''}`;
  };

  let search = $state('');
  let mode = $state('all');
  let follow = $state(true);
  let stopOpen = $state(false);

  $effect(() => {
    if (instances && !instance) {
      goto('/instances');
    }
  });

  // primitives, so the 3s instance poll does not recreate the log state
  let profile_id = $derived(instance?.profile_id);
  let instance_id = $derived(instance?.id);
  let logs_updater = $derived(
    profile_id && instance_id
      ? create_data_state(
          async () => await instance_logs(profile_id, instance_id),
          UpdateType.InstanceLogs
        )
      : undefined
  );
  let lines = $derived(
    filter_logs(
      parse_logs(logs_updater?.value ?? []),
      search,
      CONSOLE_LEVELS[mode] ?? []
    )
  );
</script>

{#if instance}
  <div class="flex h-full min-h-0 flex-col gap-4 px-5 pt-4 pb-6">
    <div class="flex flex-wrap items-center gap-3">
      <span class="size-2 shrink-0 rounded-full bg-green-500"></span>
      <h1 class="flex-1 text-xl font-semibold tracking-tight">
        {instance.profile_name} console
      </h1>
      <Select.Root
        type="single"
        value={instance.id}
        onValueChange={(id) => goto(`/instances/info/logs?id=${id}`)}
      >
        <Select.Trigger aria-label="Instance" class="w-[220px]">
          {sibling_label(siblings.findIndex((s) => s.id === instance.id))}
        </Select.Trigger>
        <Select.Content>
          {#each siblings as sibling, i (sibling.id)}
            <Select.Item value={sibling.id} label={sibling_label(i)} />
          {/each}
        </Select.Content>
      </Select.Root>
      <Button variant="destructive" size="sm" onclick={() => (stopOpen = true)}>
        Stop
      </Button>
    </div>
    <div class="flex flex-wrap gap-2">
      <Input
        type="search"
        placeholder="Search console"
        aria-label="Search console"
        class="flex-[1_1_220px]"
        bind:value={search}
      />
      <ToggleGroup.Root
        type="single"
        variant="outline"
        bind:value={() => mode, (v) => (mode = v || mode)}
      >
        <ToggleGroup.Item value="all">All</ToggleGroup.Item>
        <ToggleGroup.Item value="warn">Warnings</ToggleGroup.Item>
        <ToggleGroup.Item value="err">Errors</ToggleGroup.Item>
      </ToggleGroup.Root>
    </div>
    <LogWindow {lines} bind:follow class="min-h-[480px] grow" />
    <div class="flex items-center gap-2">
      <Switch id="follow" bind:checked={follow} />
      <Label for="follow">Follow new output</Label>
    </div>
  </div>
  <DestroyDialog
    bind:open={stopOpen}
    title="Stop Instance"
    description={`Are you sure you want to stop the instance of profile "${instance.profile_name}"?`}
    btnText="Stop"
    onclick={() => {
      instance_stop(instance.profile_id, instance.id);
      stopOpen = false;
    }}
  />
{:else}
  <p class="mt-2 ml-2">Loading...</p>
{/if}
