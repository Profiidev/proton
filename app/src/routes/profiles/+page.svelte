<script lang="ts">
  import {
    LoaderType,
    profile_create,
    profile_launch,
    profile_list,
    ProfileError
  } from '$lib/tauri/profile.svelte';
  import FormDialog from 'positron-components/components/form/form-dialog.svelte';
  import FormInput from 'positron-components/components/form/form-input.svelte';
  import { profileCreateSchema } from './schema.svelte';
  import {
    vanilla_version_list,
    version_list
  } from '$lib/tauri/versions.svelte';
  import { Input } from 'positron-components/components/ui/input';
  import { ScrollArea } from 'positron-components/components/ui/scroll-area';
  import { Plus } from '@lucide/svelte';
  import FormImage from '../../lib/components/form/FormImage.svelte';
  import { compareProfiles, file_to_bytes } from '$lib/util.svelte';
  import Multiselect from 'positron-components/components/table/multiselect.svelte';
  import Fuse from 'fuse.js';
  import { goto } from '$app/navigation';
  import { account_active } from '$lib/tauri/account.svelte';
  import ProfileListButton from '$lib/components/profile/ProfileListButton.svelte';
  import FormSelectUpdate from '$lib/components/form/FormSelectUpdate.svelte';
  import type { FormValue } from 'positron-components/components/form/types';

  let version_filter = $state<string[]>([]);
  let loader_filter = $state<string[]>([]);
  let text_filter = $state('');
  let createOpen = $state(false);
  let createDialog = $state<FormDialog<typeof profileCreateSchema>>();
  let currentLoader = $state<[LoaderType]>([LoaderType.Vanilla]);
  let currentVersion = $state<[string]>();
  let new_version_list = $state<string[]>();
  $effect(() => {
    version_list(currentLoader[0])
      .catch(() => [] as string[])
      .then((versions) => {
        new_version_list = versions;
        if (
          versions &&
          currentVersion &&
          !versions.includes(currentVersion[0])
        ) {
          currentVersion = [versions[0]];
        }
      });
  });

  let active_account = $derived(account_active.value);
  let profiles = $derived(profile_list.value);
  let vanilla_versions = $derived(
    (vanilla_version_list.value ?? []).map((v) => ({
      label: v,
      value: v
    }))
  );

  $effect(() => {
    if (createOpen) {
      createDialog?.setValue({
        name: '',
        loader: [LoaderType.Vanilla],
        version: vanilla_versions?.length ? [vanilla_versions[0].value] : []
      });
    }
  });

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
    vanilla_versions?.filter((v) =>
      profiles?.some((p) => p.version === v.value)
    )
  );
  let filtered_loaders = $derived(
    profiles
      ? [...new Set(profiles.map((p) => p.loader))].map((l) => ({
          label: l,
          value: l
        }))
      : []
  );

  const createProfile = async (form: FormValue<typeof profileCreateSchema>) => {
    let data: any = { ...form };
    data.version = form.version[0];
    data.loader = form.loader[0];
    if (form.icon) {
      data.icon = await file_to_bytes(form.icon);
    }

    let res = await profile_create(data);
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
      class="grow"
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
      schema={profileCreateSchema}
      onsubmit={createProfile}
      bind:open={createOpen}
      bind:this={createDialog as any}
      class="w-100"
    >
      {#snippet triggerInner()}
        <Plus />
      {/snippet}
      {#snippet children({ props })}
        <div class="flex w-full">
          <div>
            <FormImage key="icon" class="mt-6 size-20" {...props} />
          </div>
          <div class="ml-auto">
            <FormInput label="Name" placeholder="Name" key="name" {...props} />
            <FormSelectUpdate
              bind:val={currentLoader}
              label="Loader"
              key="loader"
              single={true}
              data={Object.keys(LoaderType).map((l) => ({
                label: l,
                value: l as LoaderType
              })) ?? []}
              {...props}
            />
            <FormSelectUpdate
              bind:val={currentVersion}
              label="Version"
              key="version"
              single={true}
              data={new_version_list?.map((v) => ({
                label: v,
                value: v
              })) ?? []}
              {...props}
            />
          </div>
        </div>
      {/snippet}
    </FormDialog>
  </div>
  {#if filtered_profiles && filtered_profiles.length > 0}
    <ScrollArea class="mt-2 min-h-0 grow">
      <div
        class="grid size-full auto-rows-min grid-cols-[repeat(auto-fill,minmax(14rem,1fr))] gap-2"
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
    </ScrollArea>
  {:else}
    <p class="text-muted-foreground mt-2 text-center">
      No profiles found. Adjust your filters or create a new profile.
    </p>
  {/if}
</div>
