<script lang="ts">
  import {
    profile_create,
    profile_launch,
    profile_list,
    ProfileError,
    type Profile
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
  import { Button, Input, ScrollArea } from 'positron-components/components/ui';
  import { CirclePlay, Plus } from '@lucide/svelte';
  import FormImage from '../../lib/components/form/FormImage.svelte';
  import { compareDateTimes, file_to_bytes } from '$lib/util.svelte';
  import { Multiselect } from 'positron-components/components/table';
  import Fuse from 'fuse.js';
  import { goto } from '$app/navigation';
  import ProfileIcon from '$lib/components/profile/ProfileIcon.svelte';

  interface Props {
    data: PageServerData;
  }

  let { data }: Props = $props();

  let version_filter = $state<string[]>([]);
  let loader_filter = $state<string[]>([]);
  let text_filter = $state<string>('');

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

  const compareProfiles = (a: Profile, b: Profile) => {
    if (!a.last_played && !b.last_played) {
      return compareDateTimes(a.created_at, b.created_at);
    }
    if (a.last_played && b.last_played) {
      return compareDateTimes(a.last_played, b.last_played);
    }
    if (a.last_played) {
      return -1;
    }
    return 1;
  };

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
      .sort(compareProfiles)
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
          <Button
            variant="outline"
            class="group relative flex h-16 w-full max-w-86 cursor-pointer flex-row justify-start p-2"
            onclick={() => goto(`/profiles/info/quick_play?id=${profile.id}`)}
          >
            <ProfileIcon id={profile.id} />
            <div class="ml-2 flex min-w-0 flex-1 flex-col justify-start gap-2">
              <p class="truncate text-start text-sm">
                {profile.name || 'unknown'}
              </p>
              <p class="text-muted-foreground truncate text-start text-sm">
                {profile.loader + ' ' + profile.version || 'unknown'}
              </p>
            </div>
            <div class="bg-background absolute hidden rounded group-hover:flex">
              <Button
                class="size-12 cursor-pointer"
                size="icon"
                onclick={(e) => {
                  e.stopPropagation();
                  profile_launch(profile.id, profile.name);
                }}
              >
                <CirclePlay class="size-8" />
              </Button>
            </div>
          </Button>
        {/each}
      </div>
    </ScrollArea.ScrollArea>
  {:else}
    <p class="text-muted-foreground mt-2 text-center">
      No profiles found. Adjust your filters or create a new profile.
    </p>
  {/if}
</div>
