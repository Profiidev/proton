<script lang="ts">
  import {
    instance_list,
    instance_stop,
    type InstanceInfo
  } from '$lib/tauri/instance.svelte';
  import { Button, Dialog, Separator } from 'positron-components/components/ui';
  import { getProfile } from '../store.svelte';
  import { CircleStop } from '@lucide/svelte';
  import { compareDateTimes } from '$lib/util.svelte';
  import { goto } from '$app/navigation';
  import { DateTime } from 'positron-components/util';
  import DestroyDialog from '$lib/components/form/DestroyDialog.svelte';

  let profile = $derived(getProfile());
  let instances = $derived(instance_list.value);
  let profile_instances = $derived(
    instances
      ?.filter((i) => i.profile_id === profile?.id)
      .toSorted((a, b) => compareDateTimes(a.launched_at, b.launched_at)) ?? []
  );

  let stopOpen = $state(false);
  let stop_instance = $state<InstanceInfo>();
</script>

<div class="flex w-full flex-col gap-2">
  {#each profile_instances as instance, index}
    <Button
      variant="outline"
      class="h-fit w-full cursor-pointer p-2"
      onclick={() => goto(`/instances/info/logs?id=${instance.id}`)}
    >
      <p class="ml-2 text-base">Instance {index + 1}</p>
      <Separator orientation="vertical" class="mx-1 h-8!" />
      <p class="text-muted-foreground">
        Launched at {DateTime.fromISO(instance.launched_at)
          .setLocale('de')
          .toLocaleString(DateTime.DATETIME_SHORT)}
      </p>
      <Button
        variant="destructive"
        class="ml-auto cursor-pointer"
        onclick={(e) => {
          e.stopImmediatePropagation();
          stopOpen = true;
          stop_instance = instance;
        }}
      >
        <CircleStop />
        Stop
      </Button>
    </Button>
  {/each}
  {#if profile_instances.length === 0}
    <p class="text-muted-foreground mt-2 ml-2">
      No instances running instances found for this profile.
    </p>
  {/if}
</div>
<DestroyDialog
  bind:open={stopOpen}
  title="Stop Instance"
  description="Are you sure you want to stop this instance?"
  btnText="Stop"
  onclick={() => {
    stop_instance && instance_stop(stop_instance.profile_id, stop_instance.id);
    stopOpen = false;
  }}
/>
