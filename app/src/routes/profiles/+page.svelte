<script lang="ts">
  import {
    profile_create,
    profile_launch,
    profile_list,
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
  import { Input, ScrollArea } from 'positron-components/components/ui';
  import { Plus } from '@lucide/svelte';
  import FormImage from '../../lib/components/form/FormImage.svelte';
  import { compareProfiles, file_to_bytes } from '$lib/util.svelte';
  import { Multiselect } from 'positron-components/components/table';
  import Fuse from 'fuse.js';
  import { goto } from '$app/navigation';
  import { account_active } from '$lib/tauri/account.svelte';
  import ProfileListButton from '$lib/components/profile/ProfileListButton.svelte';

  interface Props {
    data: PageServerData;
  }

  let { data }: Props = $props();

  let version_filter = $state<string[]>([]);
  let loader_filter = $state<string[]>([]);
  let text_filter = $state<string>('');

  let active_account = $derived(account_active.value);
  let profiles = $derived(profile_list.value);
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
    )
      .filter(
        (p) =>
          (version_filter.length > 0
            ? version_filter.includes(p.version)
            : true) &&
          (loader_filter.length > 0 ? loader_filter.includes(p.loader) : true)
      )
      .toSorted(compareProfiles)
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
            <FormImage key="icon" class="mt-6 size-20" type="file" {...props} />
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
          <ProfileListButton
            onclick={() => goto(`/profiles/info/quick_play?id=${profile.id}`)}
            onclickInner={() => {
              profile_launch(profile.id, profile.name, active_account);
            }}
            item={profile}
          />
        {/each}
      </div>
    </ScrollArea.ScrollArea>
  {:else}
    <p class="text-muted-foreground mt-2 text-center">
      No profiles found. Adjust your filters or create a new profile.
    </p>
  {/if}
</div>
