<script lang="ts">
  import {
    profile_create,
    profile_launch,
    profile_list,
    profile_remove,
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
  import { Button } from 'positron-components/components/ui';
  import { CirclePlay, X } from '@lucide/svelte';

  interface Props {
    data: PageServerData;
  }

  let { data }: Props = $props();

  let profiles = $derived(profile_list.value);
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
    let res = await profile_create(form.data);
    if (res === ProfileError.InvalidImage) {
      return { field: 'icon', error: 'Invalid image' };
    }
  };
</script>

<FormDialog
  title="Create Profile"
  confirm="Create"
  trigger={{
    text: 'Create'
  }}
  form={profileCreate}
  onsubmit={createProfile}
>
  {#snippet children({ props })}
    <FormInput label="Name" placeholder="Name" key="name" {...props} />
    <FormSelect
      label="Version"
      key="version"
      single={true}
      data={versions ?? []}
      {...props}
    />
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
      <Button size="icon" onclick={() => profile_launch(profile.id)}>
        <CirclePlay />
      </Button>
    </div>
  {/each}
{/if}
