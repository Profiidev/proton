<script lang="ts">
  import { create_data_state, UpdateType } from '$lib/data_state.svelte';
  import { instance_logs } from '$lib/tauri/instance.svelte';
  import { getInstance } from '../store.svelte';
  import LogWindow from '$lib/components/LogWindow.svelte';

  let instance = $derived(getInstance());

  let logs_updater = $derived(
    instance
      ? create_data_state(async () => {
          return await instance_logs(instance.profile_id, instance.id);
        }, UpdateType.InstanceLogs)
      : undefined
  );
  let logs = $derived(logs_updater?.value);
</script>

<LogWindow {logs} class="mb-2" />
