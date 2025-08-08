<script lang="ts">
  import {
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
  import type { SvelteComponent } from 'svelte';
  import FormImage from '$lib/components/form/FormImage.svelte';
  import { getProfile } from '../store.svelte';
  import { file_to_bytes } from '$lib/util.svelte';
  import { toast } from 'svelte-sonner';
  import { vanilla_version_list } from '$lib/tauri/versions.svelte';

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
  let versions = $derived(
    (vanilla_version_list.value ?? []).map((v) => ({
      label: v,
      value: v
    }))
  );

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
          version: [profile.version]
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

    form.data.id = profile.id;
    form.data.version = form.data.version[0];

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
        <FormSelect
          label="Version"
          key="version"
          single={true}
          data={versions ?? []}
          {...props}
        />
      {/snippet}
      {#snippet footer({ children })}
        {@render children()}
      {/snippet}
    </BaseForm>
  </div>
{/if}
