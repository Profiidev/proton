<script lang="ts">
  import {
    LoaderType,
    profile_get_icon,
    profile_update,
    profile_update_icon,
    ProfileError,
    type Profile
  } from '$lib/tauri/profile.svelte';
  import Multiselect from 'positron-components/components/table/multiselect.svelte';
  import { getProfile } from '../../store.svelte';
  import { toast } from 'svelte-sonner';
  import {
    loader_version_list,
    version_list
  } from '$lib/tauri/versions.svelte';
  import { Input } from 'positron-components/components/ui/input';
  import { Label } from 'positron-components/components/ui/label';
  import ImageInput from '$lib/components/form/ImageInput.svelte';

  let profile = $derived(getProfile());

  let versions = $state<{ label: string; value: string }[]>([]);
  let selectedVersion = $state<[string]>(['']);
  let loader_versions = $state<{ label: string; value: string }[]>([]);
  let selectedLoaderVersion = $state<[string]>(['']);

  $effect(() => {
    if (!profile) {
      return;
    }

    version_list(profile?.loader)
      .catch(() => [] as string[])
      .then((v) => {
        versions =
          v?.map((version) => ({
            label: version,
            value: version
          })) ?? [];
      });
  });

  $effect(() => {
    if (
      !profile ||
      profile.loader === LoaderType.Vanilla ||
      !selectedVersion ||
      selectedVersion.length !== 1
    )
      return;

    loader_version_list(profile.loader, selectedVersion[0])
      .catch(() => [] as string[])
      .then((v) => {
        loader_versions =
          v?.map((version) => ({
            label: version,
            value: version
          })) ?? [];

        if (
          v &&
          (!selectedLoaderVersion || !v.includes(selectedLoaderVersion[0]))
        ) {
          selectedLoaderVersion = [v[0]];
        }
      });
  });

  let formFilePreview = $state('');
  $effect(() => {
    if (profile) {
      (async () => {
        selectedVersion = [profile.version];
        if (profile.loader !== LoaderType.Vanilla && profile.loader_version) {
          selectedLoaderVersion = [profile.loader_version];
        }

        let icon = await profile_get_icon(profile.id);
        if (icon) {
          formFilePreview = `data:image/png;base64,${icon}`;
        }
      })();
    }
  });

  $effect(() => {
    selectedVersion;
    selectedLoaderVersion;

    if (!profile) return;
    let update = false;
    let updated_profile = { ...profile };

    if (profile.version !== selectedVersion[0] && selectedVersion[0]) {
      update = true;
      updated_profile.version = selectedVersion[0];
    }

    if (
      profile?.loader !== LoaderType.Vanilla &&
      profile?.loader_version !== selectedLoaderVersion[0] &&
      selectedLoaderVersion[0]
    ) {
      update = true;
      updated_profile.loader_version = selectedLoaderVersion[0];
    }

    if (update) {
      send_update(updated_profile);
    }
  });

  const send_update = (profile: Profile) => {
    profile_update(profile).then((res) => {
      if (res === ProfileError.Other) {
        toast.error('Failed to update profile');
      }
    });
  };

  const update_icon = async (file?: File) => {
    if (!profile || !file) return;
    let bytes = new Uint8Array(await file.arrayBuffer());

    let res = await profile_update_icon(profile.id, bytes);
    if (res === ProfileError.Other) {
      toast.error('Failed to update profile icon');
    }
  };
</script>

{#if profile}
  <div class="flex gap-4">
    <div class="flex w-64 flex-col gap-2">
      <Label>Profile Name</Label>
      <Input
        placeholder="Profile Name"
        value={profile.name}
        onfocusout={(e) => {
          let name = (e.target as HTMLInputElement).value;
          if (!name) {
            toast.error('Profile name cannot be empty');
            return;
          }
          if (name !== profile.name) {
            send_update({ ...profile, name });
          }
        }}
      />
      <Label>Version</Label>
      <Multiselect
        label="Version"
        single={true}
        data={versions}
        bind:selected={selectedVersion}
      />
      {#if profile.loader !== LoaderType.Vanilla}
        <Label>Loader Version</Label>
        <Multiselect
          single={true}
          data={loader_versions}
          label="Loader Version"
          bind:selected={selectedLoaderVersion}
        />
      {/if}
    </div>
    <div class="flex flex-col">
      <ImageInput
        class="mt-3"
        key="profile-icon"
        label="Profile Icon"
        fileChange={update_icon}
        previewSrc={formFilePreview}
      />
    </div>
  </div>
{/if}
