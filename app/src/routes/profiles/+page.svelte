<script lang="ts">
  import {
    profile_create,
    profile_get_icon,
    profile_launch,
    profile_list,
    profile_remove,
    profile_repair,
    ProfileError
  } from '$lib/tauri/profile.svelte';
  import {
    FormDialog,
    FormInput,
    FormSelect,
    type FormType
  } from 'positron-components/components/form';
  import type { PageServerData } from './$types';
  import { profileCreateSchema } from './schema.svelte';
  import { version_list } from '$lib/tauri/versions.svelte';
  import {
    Avatar,
    Button,
    Input,
    ScrollArea,
    Select
  } from 'positron-components/components/ui';
  import { Box, CirclePlay, Plus, Wrench, X } from '@lucide/svelte';
  import FormImage from './FormImage.svelte';
  import { instance_list, instance_logs } from '$lib/tauri/instance.svelte';
  import { create_data_state, UpdateType } from '$lib/data_state.svelte';
  import { file_to_bytes } from '$lib/util.svelte';
  import { Multiselect } from 'positron-components/components/table';
  import Fuse from 'fuse.js';

  interface Props {
    data: PageServerData;
  }

  let { data }: Props = $props();

  let version_filter = $state<string[]>([]);
  let loader_filter = $state<string[]>([]);
  let text_filter = $state<string>('');
  let profiles = $derived(profile_list.value);
  let instances = $derived(instance_list.value);
  let versions = $derived(
    (version_list.value ?? []).map((v) => ({
      label: v,
      value: v
    }))
  );

  let profile_fuse = $derived(
    new Fuse(profiles ?? [], {
      keys: [
        {
          name: 'name',
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
  let filtered_profiles = $derived(
    (text_filter
      ? profile_fuse.search(text_filter).map((result) => result.item)
      : (profiles ?? [])
    ).filter(
      (p) =>
        (version_filter.length > 0
          ? version_filter.includes(p.version)
          : true) &&
        (loader_filter.length > 0 ? loader_filter.includes(p.loader) : true)
    )
  );
  let filtered_versions = $derived(
    versions?.filter((v) => profiles?.some((p) => p.version === v.value))
  );
  let filtered_loaders = $derived(
    profiles
      ? [...new Set(profiles.map((p) => p.loader))].map((l) => ({
          label: l,
          value: l
        }))
      : []
  );

  const profileCreate = {
    form: data.profileCreate,
    schema: profileCreateSchema
  };

  const createProfile = async (form: FormType<any>) => {
    form.data.version = form.data.version[0];
    if (form.data.icon) {
      form.data.icon = await file_to_bytes(form.data.icon);
    }

    let res = await profile_create(form.data);
    if (res === ProfileError.InvalidImage) {
      return { field: 'icon', error: 'Invalid image' };
    } else if (res === ProfileError.Other) {
      return { error: 'Failed to create profile' };
    }
  };

  let instance: string | undefined = $state();
  let profile = $derived(
    instances &&
      Object.entries(instances).find(([_, instances]) =>
        instances.some((i) => i.id === instance)
      )?.[0]
  );
  let logs_updater = $derived(
    profile && instance
      ? create_data_state(async () => {
          return (await instance_logs(profile, instance!))?.reverse();
        }, UpdateType.InstanceLogs)
      : undefined
  );
  let logs = $derived(logs_updater?.value);
</script>

<div class="flex size-full flex-col">
  <div class="flex w-full gap-2">
    <Input
      placeholder="Search profiles..."
      bind:value={text_filter}
      class="flex-grow-1"
      type="search"
    />
    <Multiselect
      data={filtered_loaders ?? []}
      label="Loader"
      bind:selected={loader_filter}
    />
    <Multiselect
      data={filtered_versions ?? []}
      label="Version"
      bind:selected={version_filter}
    />
    <FormDialog
      title="Create Profile"
      confirm="Create"
      trigger={{
        size: 'icon'
      }}
      form={profileCreate}
      onsubmit={createProfile}
      open={false}
      class="w-100"
    >
      {#snippet triggerInner()}
        <Plus />
      {/snippet}
      {#snippet children({ props })}
        <div class="flex w-full">
          <div>
            <FormImage
              key="icon"
              class="size-20"
              type="file"
              label="Icon"
              {...props}
            />
          </div>
          <div class="ml-auto">
            <FormInput label="Name" placeholder="Name" key="name" {...props} />
            <FormSelect
              label="Version"
              key="version"
              single={true}
              data={versions ?? []}
              {...props}
            />
          </div>
        </div>
      {/snippet}
    </FormDialog>
  </div>
  {#if filtered_profiles && filtered_profiles.length > 0}
    <ScrollArea.ScrollArea class="mt-2 min-h-0 flex-grow-1">
      <div
        class="grid size-full auto-rows-min grid-cols-[repeat(auto-fill,_minmax(14rem,_1fr))] gap-2"
      >
        {#each filtered_profiles as profile}
          <Button
            variant="outline"
            class="group relative flex h-16 w-full max-w-86 flex-row justify-start p-2"
          >
            <Avatar.Root class="size-12 rounded-md">
              {#await profile_get_icon(profile.id)}
                <Avatar.Fallback class="rounded-md">
                  <span class="sr-only">Profile Icon</span>
                </Avatar.Fallback>
              {:then icon}
                {#if icon}
                  <Avatar.Image
                    class="object-cover"
                    src={`data:image/png;base64, ${icon}`}
                  />
                {:else}
                  <div class="flex size-full items-center justify-center">
                    <Box class="size-10" />
                  </div>
                {/if}
              {/await}
            </Avatar.Root>
            <div class="ml-2 flex min-w-0 flex-1 flex-col justify-start gap-2">
              <p class="truncate text-start text-sm">
                {profile.name || 'unknown'}
              </p>
              <p class="text-muted-foreground truncate text-start text-sm">
                {profile.loader + ' ' + profile.version || 'unknown'}
              </p>
            </div>
            <Button
              class="absolute hidden size-12 group-hover:flex"
              size="icon"
              onclick={() => profile_launch(profile.id, profile.name)}
            >
              <CirclePlay class="size-8" />
            </Button>
            {#if false}
              <Button size="icon" onclick={() => profile_remove(profile.id)}>
                <X />
              </Button>
              <Button
                size="icon"
                onclick={() => profile_repair(profile.id, profile.name)}
              >
                <Wrench />
              </Button>
            {/if}
          </Button>
        {/each}
      </div>
    </ScrollArea.ScrollArea>
  {:else}
    <p class="text-muted-foreground mt-2 text-center">
      No profiles found. Adjust your filters or create a new profile.
    </p>
  {/if}
  {#if instances}
    {#each Object.entries(instances) as [profile, sub_instances]}
      <p>Profile: {profile}</p>
      {#each sub_instances as instance}
        <p>Instance: {instance.id}</p>
      {/each}
    {/each}
    <Select.Root type="single" bind:value={instance}>
      <Select.Trigger>Test</Select.Trigger>
      <Select.Content>
        {#each Object.entries(instances) as [_, sub_instances]}
          {#each sub_instances as instance}
            <Select.Item value={instance.id}>
              {instance.id}
            </Select.Item>
          {/each}
        {/each}
      </Select.Content>
    </Select.Root>
  {/if}
  {#if logs}
    <ScrollArea.ScrollArea class="h-100">
      {#each logs as log}
        <p>{log}</p>
      {/each}
    </ScrollArea.ScrollArea>
  {/if}
</div>
