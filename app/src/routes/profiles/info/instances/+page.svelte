<script lang="ts">
  import {
    instance_list,
    instance_stop,
    type InstanceInfo
  } from '$lib/tauri/instance.svelte';
  import * as Empty from '@profidev/pleiades/components/ui/empty';
  import { getProfile } from '../store.svelte';
  import { group_by_profile } from '$lib/instances';
  import DestroyDialog from '$lib/components/form/DestroyDialog.svelte';
  import InstanceRow from '$lib/components/instance/InstanceRow.svelte';

  let profile = $derived(getProfile());
  let profile_instances = $derived(
    (profile && group_by_profile(instance_list.value ?? []).get(profile.id)) ??
      []
  );

  let now = $state(Date.now());
  $effect(() => {
    const interval = setInterval(() => {
      instance_list.update();
      now = Date.now();
    }, 3000);
    return () => clearInterval(interval);
  });

  let stopOpen = $state(false);
  let stop_instance = $state<InstanceInfo>();
</script>

<div class="flex w-full min-w-0 flex-col gap-3">
  <div>
    <h2 class="text-base font-medium">Running instances</h2>
    <p class="text-muted-foreground mt-0.5 text-xs">
      Use Launch another above to start more. Pick a different account to play
      with two at once.
    </p>
  </div>
  {#each profile_instances as instance, index (instance.id)}
    <InstanceRow
      {instance}
      number={index + 1}
      {now}
      card
      onstop={() => {
        stop_instance = instance;
        stopOpen = true;
      }}
    />
  {:else}
    <Empty.Root class="flex-none border border-dashed">
      <Empty.Header>
        <Empty.Title>Not running</Empty.Title>
        <Empty.Description>
          This profile has no running instance. Press Play above to start one.
        </Empty.Description>
      </Empty.Header>
    </Empty.Root>
  {/each}
  <p class="text-muted-foreground text-xs">
    Worlds are shared between instances, so only open a world in one of them at
    a time.
  </p>
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
