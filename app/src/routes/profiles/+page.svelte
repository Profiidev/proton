<script lang="ts">
  import { profile_list } from '$lib/tauri/profile.svelte';
  import { vanilla_version_list } from '$lib/tauri/versions.svelte';
  import { Input } from '@profidev/pleiades/components/ui/input';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import * as Dialog from '@profidev/pleiades/components/ui/dialog';
  import { Plus, Search } from '@lucide/svelte';
  import { compareProfiles } from '$lib/util.svelte';
  import Multiselect from '@profidev/pleiades/components/table/multiselect.svelte';
  import Fuse from 'fuse.js';
  import ProfileCard from '$lib/components/profile/ProfileCard.svelte';
  import { profiles_summary } from '$lib/profiles';
  import CreateProfileForm from './CreateProfileForm.svelte';
  import FromModpackTab from './FromModpackTab.svelte';
  import ImportFileTab from './ImportFileTab.svelte';
  import * as Tabs from '@profidev/pleiades/components/ui/tabs';
  import ModpackInstallDialog from '$lib/components/content/ModpackInstallDialog.svelte';
  import type { PackPlan, ProjectSummary } from '$lib/tauri/content.svelte';

  let version_filter = $state<string[]>([]);
  let loader_filter = $state<string[]>([]);
  let text_filter = $state('');
  let createOpen = $state(false);
  let createTab = $state('custom');
  let packOpen = $state(false);
  let pack = $state<{ project?: ProjectSummary; plan?: PackPlan }>({});

  // The pack dialog replaces the create dialog
  const install_pack = (next: typeof pack) => {
    pack = next;
    createOpen = false;
    packOpen = true;
  };

  let profiles = $derived(profile_list.value);

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
    (vanilla_version_list.value ?? [])
      .filter((v) => profiles?.some((p) => p.version === v))
      .map((v) => ({ label: v, value: v }))
  );
  let filtered_loaders = $derived(
    [...new Set((profiles ?? []).map((p) => p.loader))].map((l) => ({
      label: l,
      value: l
    }))
  );
</script>

<div class="flex flex-col gap-4 px-5 pt-4 pb-6">
  <div class="flex flex-wrap items-baseline gap-2.5">
    <h1 class="text-xl font-semibold tracking-tight">Profiles</h1>
    <span class="text-muted-foreground text-sm">
      {profiles_summary(profiles ?? [])}
    </span>
  </div>
  <div class="flex flex-wrap items-center gap-2">
    <div class="relative min-w-50 flex-[1_1_280px]">
      <Search
        class="text-muted-foreground pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2"
      />
      <Input
        placeholder="Search profiles by name, loader or version"
        aria-label="Search profiles"
        bind:value={text_filter}
        class="pl-8"
        type="search"
      />
    </div>
    <Multiselect
      data={filtered_loaders}
      label="Loader"
      bind:selected={loader_filter}
      buttonPrefix=""
    />
    <Multiselect
      data={filtered_versions}
      label="Version"
      bind:selected={version_filter}
      buttonPrefix=""
    />
    <Button onclick={() => (createOpen = true)}>
      <Plus data-icon="inline-start" />
      New profile
    </Button>
  </div>
  {#if filtered_profiles.length > 0}
    <div class="grid grid-cols-[repeat(auto-fill,minmax(264px,1fr))] gap-2">
      {#each filtered_profiles as profile (profile.id)}
        <ProfileCard {profile} />
      {/each}
    </div>
  {:else}
    <p class="text-muted-foreground text-center">
      No profiles found. Adjust your filters or create a new profile.
    </p>
  {/if}
</div>

<Dialog.Root bind:open={createOpen}>
  <Dialog.Content
    class="w-[min(620px,calc(100vw-32px))] max-w-none gap-5 sm:max-w-none"
  >
    <Dialog.Header>
      <Dialog.Title>Create profile</Dialog.Title>
      <Dialog.Description>
        Pick a loader and version, or start from a modpack.
      </Dialog.Description>
    </Dialog.Header>
    <Tabs.Root bind:value={createTab} class="min-w-0">
      <Tabs.List>
        <Tabs.Trigger value="custom">Custom</Tabs.Trigger>
        <Tabs.Trigger value="modpack">From modpack</Tabs.Trigger>
        <Tabs.Trigger value="import">Import file</Tabs.Trigger>
      </Tabs.List>
      <Tabs.Content value="custom" class="pt-4">
        <CreateProfileForm onclose={() => (createOpen = false)} />
      </Tabs.Content>
      <Tabs.Content value="modpack">
        {#if createTab === 'modpack'}
          <FromModpackTab onselect={(project) => install_pack({ project })} />
        {/if}
      </Tabs.Content>
      <Tabs.Content value="import">
        <!-- Mounted only while visible so it only listens for drops here -->
        {#if createTab === 'import'}
          <ImportFileTab onplan={(plan) => install_pack({ plan })} />
        {/if}
      </Tabs.Content>
    </Tabs.Root>
  </Dialog.Content>
</Dialog.Root>

<ModpackInstallDialog
  bind:open={packOpen}
  project={pack.project}
  plan={pack.plan}
/>
