<script lang="ts">
  import { version_list } from '$lib/tauri/versions.svelte';
  import {
    Button,
    Dialog,
    Input,
    ScrollArea
  } from 'positron-components/components/ui';
  import { CircleStop, ExternalLink } from '@lucide/svelte';
  import {
    instance_list,
    instance_stop,
    type InstanceInfo
  } from '$lib/tauri/instance.svelte';
  import { Multiselect } from 'positron-components/components/table';
  import Fuse from 'fuse.js';
  import { goto } from '$app/navigation';
  import ProfileIcon from '$lib/components/profile/ProfileIcon.svelte';

  let profile_filter = $state<string[]>([]);
  let version_filter = $state<string[]>([]);
  let loader_filter = $state<string[]>([]);
  let text_filter = $state<string>('');
  let stopOpen = $state(false);
  let stop_instance = $state<InstanceInfo>();

  let instances = $derived(instance_list.value);
  let versions = $derived(
    (version_list.value ?? []).map((v) => ({
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
    ).filter(
      (p) =>
        (version_filter.length > 0
          ? version_filter.includes(p.version)
          : true) &&
        (loader_filter.length > 0 ? loader_filter.includes(p.loader) : true) &&
        (profile_filter.length > 0
          ? profile_filter.includes(p.profile_id)
          : true)
    )
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
          label: p,
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
          <Button
            variant="outline"
            class="group relative flex h-16 w-full max-w-86 cursor-pointer flex-row justify-start p-2"
            onclick={() => goto(`/instances/info/logs?id=${instance.id}`)}
          >
            <ProfileIcon id={instance.profile_id} />
            <div class="ml-2 flex min-w-0 flex-1 flex-col justify-start gap-2">
              <p class="truncate text-start text-sm">
                {instance.profile_name || 'unknown'}
              </p>
              <p class="text-muted-foreground truncate text-start text-sm">
                {instance.loader + ' ' + instance.version || 'unknown'}
              </p>
            </div>
            <div class="bg-background absolute hidden rounded group-hover:flex">
              <Button
                class="size-12 cursor-pointer"
                size="icon"
                variant="destructive"
                onclick={(e) => {
                  e.stopPropagation();
                  stop_instance = instance;
                  stopOpen = true;
                }}
              >
                <CircleStop class="size-8" />
              </Button>
            </div>
          </Button>
        {/each}
      </div>
    </ScrollArea.ScrollArea>
  {:else}
    <p class="text-muted-foreground mt-2 text-center">
      No instances found. Adjust your filters or launch a profile to create one.
    </p>
    <div class="flex justify-center mt-2">
      <Button
        variant="outline"
        onclick={() => goto('/profiles')}
        class="text-md inline-flex w-fit cursor-pointer p-0"
      >
        Profiles
        <ExternalLink />
      </Button>
    </div>
  {/if}
</div>
<Dialog.Root bind:open={stopOpen}>
  <Dialog.Content>
    <Dialog.Header>
      <Dialog.Title>Stop Instance</Dialog.Title>
      <Dialog.Description>
        Are you sure you want to stop the instance of profile "{stop_instance?.profile_name}"?
      </Dialog.Description>
    </Dialog.Header>
    <Dialog.Footer>
      <Button
        type="submit"
        variant="destructive"
        onclick={() => {
          stop_instance &&
            instance_stop(stop_instance.profile_id, stop_instance.id);
          stopOpen = false;
        }}
      >
        Stop
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
