<script lang="ts">
  import { vanilla_version_list } from '$lib/tauri/versions.svelte';
  import { Input, ScrollArea } from 'positron-components/components/ui';
  import { CircleStop } from '@lucide/svelte';
  import {
    instance_list,
    instance_stop,
    type InstanceInfo
  } from '$lib/tauri/instance.svelte';
  import { Multiselect } from 'positron-components/components/table';
  import Fuse from 'fuse.js';
  import { goto } from '$app/navigation';
  import { compareDateTimes } from '$lib/util.svelte';
  import ProfileListButton from '$lib/components/profile/ProfileListButton.svelte';
  import ProfileExternalLink from '$lib/components/profile/ProfileExternalLink.svelte';
  import DestroyDialog from '$lib/components/form/DestroyDialog.svelte';

  let profile_filter = $state<string[]>([]);
  let version_filter = $state<string[]>([]);
  let loader_filter = $state<string[]>([]);
  let text_filter = $state<string>('');
  let stopOpen = $state(false);
  let stop_instance = $state<InstanceInfo>();

  let instances = $derived(instance_list.value);
  let versions = $derived(
    (vanilla_version_list.value ?? []).map((v) => ({
      label: v,
      value: v
    }))
  );

  let instance_fuse = $derived(
    new Fuse(instances ?? [], {
      keys: [
        {
          name: 'profile_name',
          weight: 1
        },
        {
          name: 'version',
          weight: 0.5
        },
        {
          name: 'loader',
          weight: 0.5
        }
      ],
      useExtendedSearch: true,
      threshold: 0.4
    })
  );

  let filtered_instances = $derived(
    (text_filter
      ? instance_fuse.search(text_filter).map((result) => result.item)
      : (instances ?? [])
    )
      .filter(
        (p) =>
          (version_filter.length > 0
            ? version_filter.includes(p.version)
            : true) &&
          (loader_filter.length > 0
            ? loader_filter.includes(p.loader)
            : true) &&
          (profile_filter.length > 0
            ? profile_filter.includes(p.profile_id)
            : true)
      )
      .toSorted((a, b) => compareDateTimes(a.launched_at, b.launched_at))
  );
  let filtered_versions = $derived(
    versions?.filter((v) => instances?.some((p) => p.version === v.value))
  );
  let filtered_loaders = $derived(
    instances
      ? [...new Set(instances.map((p) => p.loader))].map((l) => ({
          label: l,
          value: l
        }))
      : []
  );
  let filtered_profiles = $derived(
    instances
      ? [...new Set(instances.map((p) => p.profile_id))].map((p) => ({
          label:
            instances.find((i) => i.profile_id === p)?.profile_name ||
            'unknown',
          value: p
        }))
      : []
  );
</script>

<div class="flex size-full flex-col">
  <div class="flex w-full gap-2">
    <Input
      placeholder="Search instances..."
      bind:value={text_filter}
      class="flex-grow-1"
      type="search"
    />
    <Multiselect
      data={filtered_profiles ?? []}
      label="Profile"
      bind:selected={profile_filter}
      buttonPrefix="Search"
      class="w-35"
    />
    <Multiselect
      data={filtered_loaders ?? []}
      label="Loader"
      bind:selected={loader_filter}
      buttonPrefix="Search"
      class="w-35"
    />
    <Multiselect
      data={filtered_versions ?? []}
      label="Version"
      bind:selected={version_filter}
      buttonPrefix="Search"
      class="w-35"
    />
  </div>
  {#if filtered_instances && filtered_instances.length > 0}
    <ScrollArea.ScrollArea class="mt-2 min-h-0 flex-grow-1">
      <div
        class="grid size-full auto-rows-min grid-cols-[repeat(auto-fill,_minmax(14rem,_1fr))] gap-2"
      >
        {#each filtered_instances as instance}
          <ProfileListButton
            onclick={() => goto(`/instances/info/logs?id=${instance.id}`)}
            onclickInner={() => {
              stop_instance = instance;
              stopOpen = true;
            }}
            item={{
              ...instance,
              name: instance.profile_name
            }}
            innerIcon={CircleStop}
            innerVariant="destructive"
          />
        {/each}
      </div>
    </ScrollArea.ScrollArea>
  {:else}
    <p class="text-muted-foreground mt-2 text-center">
      No instances found. Adjust your filters or launch a profile to create one.
    </p>
    <ProfileExternalLink />
  {/if}
</div>
<DestroyDialog
  open={stopOpen}
  title="Stop Instance"
  description={`Are you sure you want to stop the instance of profile "${stop_instance?.profile_name}"?`}
  btnText="Stop"
  onclick={() => {
    stop_instance && instance_stop(stop_instance.profile_id, stop_instance.id);
    stopOpen = false;
  }}
/>
