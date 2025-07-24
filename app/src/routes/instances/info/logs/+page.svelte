<script lang="ts">
  import { create_data_state, UpdateType } from '$lib/data_state.svelte';
  import { instance_logs } from '$lib/tauri/instance.svelte';
  import { ScrollArea } from 'positron-components/components/ui';
  import { getInstance } from '../store.svelte';

  let instance = $derived(getInstance());

  let logs_updater = $derived(
    instance
      ? create_data_state(async () => {
          return (
            await instance_logs(instance.profile_id, instance.id)
          )?.reverse();
        }, UpdateType.InstanceLogs)
      : undefined
  );
  let logs = $derived(logs_updater?.value);
</script>

<p>Logs for instance</p>
{#if logs}
  <ScrollArea.ScrollArea class="h-full w-full">
    {#each logs as log}
      <div>{log}</div>
    {/each}
  </ScrollArea.ScrollArea>
{:else}
  <p>No logs available.</p>
{/if}
