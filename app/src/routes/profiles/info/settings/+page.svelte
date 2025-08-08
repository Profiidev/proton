<script lang="ts">
  import {
    LoaderType,
    profile_get_icon,
    profile_update,
    profile_update_icon,
    ProfileError
  } from '$lib/tauri/profile.svelte';
  import type { PageServerData } from './$types';
  import {
    BaseForm,
    FormInput,
    FormSelect,
    type FormType
  } from 'positron-components/components/form';
  import { profileEditSchema } from './schema.svelte';
  import { type SvelteComponent } from 'svelte';
  import FormImage from '$lib/components/form/FormImage.svelte';
  import { getProfile } from '../store.svelte';
  import { file_to_bytes } from '$lib/util.svelte';
  import { toast } from 'svelte-sonner';
  import {
    loader_version_list,
    version_list
  } from '$lib/tauri/versions.svelte';
  import FormSelectUpdate from '$lib/components/form/FormSelectUpdate.svelte';

  interface Props {
    data: PageServerData;
  }

  let { data }: Props = $props();

  let profile = $derived(getProfile());
  let isLoading = $state(false);
  let form = $state<SvelteComponent>();
  let setValue: (value: any) => void = $derived(
    form?.setValue || (() => undefined)
  );
  let versions = $state<{ label: string; value: string }[]>([]);
  let selectedVersion = $state<[string]>();
  let loader_versions = $state<{ label: string; value: string }[]>([]);
  let selectedLoaderVersion = $state<[string]>();
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
    if (!profile || profile.loader === LoaderType.Vanilla || !selectedVersion)
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

  const profileEdit = {
    form: data.profileEdit,
    schema: profileEditSchema
  };

  let formFilePreview = $state('');
  $effect(() => {
    if (profile) {
      (async () => {
        let profileData = {
          ...profile,
          version: [profile.version],
          loader_version: [profile.loader_version]
        };
        setTimeout(() => {
          setValue(profileData);
        });

        let icon = await profile_get_icon(profile.id);
        if (icon) {
          formFilePreview = `data:image/png;base64,${icon}`;
        }
      })();
    }
  });

  const updateProfile = async (form: FormType<any>) => {
    if (!profile) {
      return { error: 'Profile not found' };
    }

    if (
      profile.loader !== LoaderType.Vanilla &&
      (!form.data.loader_version || form.data.loader_version.length !== 1)
    ) {
      return { field: 'loader_version', error: 'Loader version is required' };
    }

    form.data.id = profile.id;
    form.data.version = form.data.version[0];
    form.data.loader_version = form.data.loader_version
      ? form.data.loader_version[0]
      : undefined;

    if (form.data.icon) {
      let bytes = await file_to_bytes(form.data.icon);
      let res = await profile_update_icon(profile.id, bytes);

      if (res === ProfileError.Other) {
        return { error: 'Failed to update profile icon' };
      } else if (res === ProfileError.InvalidImage) {
        return { field: 'icon', error: 'Invalid image' };
      }
    }

    let res = await profile_update(form.data);
    if (res === ProfileError.Other) {
      return { error: 'Failed to update profile' };
    } else {
      toast.success('Profile updated successfully');
    }

    return undefined;
  };
</script>

{#if profile}
  <div>
    <BaseForm
      form={profileEdit}
      confirm="Save"
      bind:isLoading
      bind:this={form}
      onsubmit={updateProfile}
    >
      {#snippet children({ props })}
        <FormImage
          key="icon"
          class="size-20"
          type="file"
          previewSrc={formFilePreview}
          {...props}
        />
        <FormInput
          label="Name"
          placeholder="Profile Name"
          key="name"
          {...props}
        />
        <FormSelectUpdate
          bind:val={selectedVersion}
          label="Version"
          key="version"
          single={true}
          data={versions}
          {...props}
        />
        {#if profile.loader !== LoaderType.Vanilla}
          <FormSelectUpdate
            bind:val={selectedLoaderVersion}
            label="Loader Version"
            key="loader_version"
            single={true}
            data={loader_versions}
            {...props}
          />
        {/if}
      {/snippet}
      {#snippet footer({ children })}
        {@render children()}
      {/snippet}
    </BaseForm>
  </div>
{/if}
