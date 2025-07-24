<script lang="ts">
  import { create_data_state, UpdateType } from '$lib/data_state.svelte';
  import { getProfile } from '../store.svelte';
  import LogWindow from '$lib/components/LogWindow.svelte';
  import { profile_runs_list, profile_logs } from '$lib/tauri/profile.svelte';
  import { DateTime } from 'positron-components/util';
  import { Multiselect } from 'positron-components/components/table';
  import { compareDateTimes } from '$lib/util.svelte';

  let profile = $derived(getProfile());
  let selected_run = $state<string[]>([]);

  let logs_list_updater = $derived(
    profile
      ? create_data_state(async () => {
          console.log('Fetching profile runs');
          return await profile_runs_list(profile.id);
        }, UpdateType.ProfileLogs)
      : undefined
  );
  let logs_list = $derived(logs_list_updater?.value);
  let logs_list_select = $derived(
    (() => {
      let list = [];

      if (logs_list) {
        for (const run of logs_list) {
          let label = DateTime.fromISO(run)
            .setLocale('de')
            .toLocaleString(DateTime.DATETIME_SHORT);

          let count: number = list.filter(
            (item) => item.label.trim() === label
          ).length;

          list.push({
            label: label + ' '.repeat(count),
            value: run
          });
        }
      }

      return list.sort((a, b) => compareDateTimes(a.label, b.label)).reverse();
    })()
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
</LogWindow>
