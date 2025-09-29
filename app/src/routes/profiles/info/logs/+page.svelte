<script lang="ts">
  import { create_data_state, UpdateType } from '$lib/data_state.svelte';
  import { getProfile } from '../store.svelte';
  import LogWindow from '$lib/components/LogWindow.svelte';
  import {
    profile_runs_list,
    profile_logs,
    profile_clear_logs
  } from '$lib/tauri/logs.svelte';
  import { DateTime } from 'positron-components/util';
  import { Multiselect } from 'positron-components/components/table';
  import { compareDateTimes } from '$lib/util.svelte';
  import {
    Button,
    Dialog,
    DropdownMenu
  } from 'positron-components/components/ui';
  import { Menu, Trash } from '@lucide/svelte';

  let profile = $derived(getProfile());
  let selected_run = $state<string[]>([]);
  let clearOpen = $state(false);

  let logs_list_updater = $derived(
    profile
      ? create_data_state(async () => {
          return await profile_runs_list(profile.id);
        }, UpdateType.ProfileLogs)
      : undefined
  );
  let logs_list = $derived(logs_list_updater?.value);
  let logs_list_select = $derived(
    logs_list
      ?.reduce((a: { label: string; value: string }[], run) => {
        let label = DateTime.fromISO(run)
          .setLocale('de')
          .toLocaleString(DateTime.DATETIME_SHORT);

        let count: number = a.filter(
          (item) => item.label.trim() === label
        ).length;

        a.push({
          label: label + ' '.repeat(count),
          value: run
        });

        return a;
      }, [])
      .toSorted((a, b) => compareDateTimes(a.label, b.label))
      .reverse() ?? []
  );
  let logs = $state<string[]>([]);

  $effect(() => {
    if (selected_run && selected_run.length === 1 && profile) {
      profile_logs(profile.id, selected_run[0]).then((newLogs) => {
        if (newLogs) {
          logs = newLogs;
        }
      });
    }
  });

  $effect(() => {});
</script>

<LogWindow {logs} class="mb-2">
  <Multiselect
    data={logs_list_select}
    label="Run"
    bind:selected={selected_run}
    buttonPrefix="Select"
    class="w-40"
    single
  />
  <DropdownMenu.Root>
    <DropdownMenu.Trigger>
      {#snippet child({ props }: { props: Record<string, any> })}
        <Button variant="outline" size="icon" {...props} class="cursor-pointer">
          <Menu />
        </Button>
      {/snippet}
    </DropdownMenu.Trigger>
    <DropdownMenu.Content>
      <DropdownMenu.Item
        variant="destructive"
        class="cursor-pointer"
        onclick={() => (clearOpen = true)}
      >
        <Trash />
        Clear Runs
      </DropdownMenu.Item>
    </DropdownMenu.Content>
  </DropdownMenu.Root>
</LogWindow>
<Dialog.Root bind:open={clearOpen}>
  <Dialog.Content>
    <Dialog.Header>
      <Dialog.Title>Clear Logs</Dialog.Title>
      <Dialog.Description>
        Are you sure you want to clear the logs of this profile?
      </Dialog.Description>
    </Dialog.Header>
    <Dialog.Footer>
      <Button
        type="submit"
        variant="destructive"
        class="cursor-pointer"
        onclick={() => {
          profile_clear_logs(profile!.id);
          clearOpen = false;
        }}
      >
        Clear
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
