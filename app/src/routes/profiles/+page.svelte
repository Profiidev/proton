<script lang="ts">
  import {
    profile_create,
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
    Button,
    ScrollArea,
    Select
  } from 'positron-components/components/ui';
  import { CirclePlay, Wrench, X } from '@lucide/svelte';
  import FormImage from './FormImage.svelte';
  import { instance_list, instance_logs } from '$lib/tauri/instance.svelte';
  import { create_data_state, UpdateType } from '$lib/data_state.svelte';
  import { file_to_bytes } from '$lib/util.svelte';

  interface Props {
    data: PageServerData;
  }

  let { data }: Props = $props();

  let profiles = $derived(profile_list.value);
  let instances = $derived(instance_list.value);
  let versions = $derived(
    (version_list.value ?? []).map((v) => ({
      label: v,
      value: v
    }))
  );

  const profileCreate = {
    form: data.profileCreate,
    schema: profileCreateSchema
  };

  const createProfile = async (form: FormType<any>) => {
    form.data.version = form.data.version[0];
    form.data.icon = await file_to_bytes(form.data.icon);

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

<FormDialog
  title="Create Profile"
  confirm="Create"
  trigger={{
    text: 'Create'
  }}
  form={profileCreate}
  onsubmit={createProfile}
  open={false}
  class="w-100"
>
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
{#if profiles}
  {#each profiles as profile}
    <div>
      {profile.name || 'unknown'}
      {profile.version || 'unknown'}
      <Button size="icon" onclick={() => profile_remove(profile.id)}>
        <X />
      </Button>
      <Button
        size="icon"
        onclick={() => profile_launch(profile.id, profile.name)}
      >
        <CirclePlay />
      </Button>
      <Button
        size="icon"
        onclick={() => profile_repair(profile.id, profile.name)}
      >
        <Wrench />
      </Button>
    </div>
  {/each}
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
