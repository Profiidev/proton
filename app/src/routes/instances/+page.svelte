<script lang="ts">
  import { Button } from '@profidev/pleiades/components/ui/button';
  import * as Empty from '@profidev/pleiades/components/ui/empty';
  import { Plus } from '@lucide/svelte';
  import {
    instance_list,
    instance_stop,
    instance_stop_all,
    type InstanceInfo
  } from '$lib/tauri/instance.svelte';
  import { profile_launch } from '$lib/tauri/profile.svelte';
  import { account_active } from '$lib/tauri/account.svelte';
  import { group_by_profile, running_label } from '$lib/instances';
  import { running_summary } from '$lib/components/instance/instance';
  import ProfileExternalLink from '$lib/components/profile/ProfileExternalLink.svelte';
  import DestroyDialog from '$lib/components/form/DestroyDialog.svelte';
  import InstanceRow from '$lib/components/instance/InstanceRow.svelte';

  let instances = $derived(instance_list.value ?? []);
  let groups = $derived([...group_by_profile(instances).values()]);

  let now = $state(Date.now());
  $effect(() => {
    const interval = setInterval(() => {
      instance_list.update();
      now = Date.now();
    }, 3000);
    return () => clearInterval(interval);
  });

  let stopOpen = $state(false);
  let stopAllOpen = $state(false);
  let stop_instance = $state<InstanceInfo>();
</script>

<div class="flex flex-col gap-4 px-5 pt-4 pb-6">
  <div class="flex items-center gap-2">
    <div class="flex-1">
      <h1 class="text-xl font-semibold tracking-tight">Instances</h1>
      {#if instances.length > 0}
        <p class="text-muted-foreground mt-0.5">
          {running_summary(instances.length, groups.length)}
        </p>
      {/if}
    </div>
    {#if instances.length > 0}
      <Button
        variant="destructive"
        size="sm"
        onclick={() => (stopAllOpen = true)}
      >
        Stop all
      </Button>
    {/if}
  </div>
  {#each groups as group (group[0].profile_id)}
    {@const first = group[0]}
    <section class="flex flex-col gap-2">
      <div class="flex flex-wrap items-center gap-2">
        <a
          href={`/profiles/info/instances?id=${first.profile_id}`}
          class="text-base font-medium"
        >
          {first.profile_name}
        </a>
        <span class="text-muted-foreground">{first.loader} {first.version}</span
        >
        <span
          class="inline-flex h-5 items-center gap-1 rounded-full bg-green-500/15 px-2 text-xs font-medium text-green-500"
        >
          {running_label(group.length)}
        </span>
        <span class="flex-1"></span>
        <Button
          variant="outline"
          size="sm"
          onclick={() =>
            profile_launch(
              first.profile_id,
              first.profile_name,
              account_active.value
            )}
        >
          <Plus data-icon="inline-start" />
          Launch another
        </Button>
      </div>
      <div class="bg-card/50 flex flex-col overflow-hidden rounded-xl border">
        {#each group as instance, index (instance.id)}
          <InstanceRow
            {instance}
            number={index + 1}
            {now}
            onstop={() => {
              stop_instance = instance;
              stopOpen = true;
            }}
          />
        {/each}
      </div>
    </section>
  {:else}
    <Empty.Root class="border border-dashed">
      <Empty.Header>
        <Empty.Title>No games running</Empty.Title>
        <Empty.Description>
          Launch a profile to see its instances here.
        </Empty.Description>
      </Empty.Header>
      <Empty.Content>
        <ProfileExternalLink />
      </Empty.Content>
    </Empty.Root>
  {/each}
</div>
<DestroyDialog
  bind:open={stopOpen}
  title="Stop Instance"
  description={`Are you sure you want to stop this instance of profile "${stop_instance?.profile_name}"?`}
  btnText="Stop"
  onclick={() => {
    stop_instance && instance_stop(stop_instance.profile_id, stop_instance.id);
    stopOpen = false;
  }}
/>
<DestroyDialog
  bind:open={stopAllOpen}
  title="Stop all instances"
  description="Are you sure you want to stop every running instance?"
  btnText="Stop all"
  onclick={() => {
    instance_stop_all();
    stopAllOpen = false;
  }}
/>
