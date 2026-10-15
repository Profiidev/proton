<script lang="ts">
  import {
    LoaderType,
    profile_update,
    profile_update_icon,
    ProfileError,
    type Profile
  } from '$lib/tauri/profile.svelte';
  import { getProfile } from '../../store.svelte';
  import { toast } from 'svelte-sonner';
  import {
    loader_version_list,
    version_list
  } from '$lib/tauri/versions.svelte';
  import { Input } from '@profidev/pleiades/components/ui/input';
  import { Label } from '@profidev/pleiades/components/ui/label';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import VersionSelect from '$lib/components/form/VersionSelect.svelte';
  import * as Alert from '@profidev/pleiades/components/ui/alert';
  import { TriangleAlert } from '@lucide/svelte';
  import ProfileIcon from '$lib/components/profile/ProfileIcon.svelte';

  let profile = $derived(getProfile());
  let modded = $derived(profile && profile.loader !== LoaderType.Vanilla);

  let versions = $state<string[]>([]);
  let loader_versions = $state<string[]>([]);
  let iconInput = $state<HTMLInputElement>();

  $effect(() => {
    if (!profile) return;
    version_list(profile.loader)
      .catch(() => undefined)
      .then((v) => (versions = v ?? []));
  });

  $effect(() => {
    if (!profile || !modded) return;
    loader_version_list(profile.loader, profile.version)
      .catch(() => undefined)
      .then((v) => (loader_versions = v ?? []));
  });

  const send_update = async (profile: Profile) => {
    if ((await profile_update(profile)) === ProfileError.Other) {
      toast.error('Failed to update profile');
    }
  };

  const change_version = async (version: string) => {
    if (!profile || version === profile.version) return;
    let loader_version = profile.loader_version;
    if (modded) {
      const available = (await loader_version_list(
        profile.loader,
        version
      ).catch(() => undefined)) ?? [loader_version];
      if (!loader_version || !available.includes(loader_version)) {
        loader_version = available[0];
      }
    }
    send_update({ ...profile, version, loader_version });
  };

  const update_icon = async (file: File | undefined) => {
    if (!profile || !file) return;
    let bytes = new Uint8Array(await file.arrayBuffer());

    let res = await profile_update_icon(profile.id, bytes);
    if (res === ProfileError.Other) {
      toast.error('Failed to update profile icon');
    }
  };
</script>

{#if profile}
  <div class="flex items-center gap-3.5">
    <ProfileIcon
      id={profile.id}
      class="bg-muted size-16 rounded-md"
      classFallback="size-8"
    />
    <Button variant="outline" onclick={() => iconInput?.click()}>
      Change icon
    </Button>
    <input
      bind:this={iconInput}
      type="file"
      class="hidden"
      accept="image/png, image/jpeg, image/jpg"
      onchange={(e) => update_icon(e.currentTarget.files?.[0])}
    />
  </div>
  <div class="flex flex-col gap-1.5">
    <Label for="profile-name">Name</Label>
    <Input
      id="profile-name"
      value={profile.name}
      onchange={(e) => {
        let name = e.currentTarget.value.trim();
        if (!name) {
          toast.error('Profile name cannot be empty');
          return;
        }
        if (name !== profile.name) {
          send_update({ ...profile, name });
        }
      }}
    />
  </div>
  <div class="flex flex-wrap gap-3">
    <div class="flex flex-[1_1_180px] flex-col gap-1.5">
      <Label>Minecraft version</Label>
      <VersionSelect
        label="Minecraft version"
        options={versions.includes(profile.version)
          ? versions
          : [profile.version, ...versions]}
        value={profile.version}
        onchange={change_version}
      />
    </div>
    {#if modded}
      <div class="flex flex-[1_1_180px] flex-col gap-1.5">
        <Label>{profile.loader} loader version</Label>
        <VersionSelect
          label="Loader version"
          options={loader_versions}
          value={profile.loader_version}
          onchange={(loader_version) =>
            send_update({ ...profile, loader_version })}
        />
      </div>
    {/if}
  </div>
  {#if modded}
    <Alert.Root>
      <TriangleAlert />
      <Alert.Title>Changing the version can break mods</Alert.Title>
      <Alert.Description>
        Mods built for another version may fail to load.
      </Alert.Description>
    </Alert.Root>
  {/if}
{/if}
