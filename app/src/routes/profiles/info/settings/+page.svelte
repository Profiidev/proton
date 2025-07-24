<script lang="ts">
  import { goto } from '$app/navigation';
  import { type Profile } from '$lib/tauri/profile.svelte';
  import type { PageServerData } from './$types';
  import { BaseForm, FormInput } from 'positron-components/components/form';
  import { profileEditSchema } from './schema.svelte';
  import type { SvelteComponent } from 'svelte';
  import FormImage from '$lib/components/form/FormImage.svelte';
  import { getProfile } from '../store.svelte';

  interface Props {
    data: PageServerData;
  }

  let { data }: Props = $props();

  let profile = $derived(getProfile());
  let isLoading = $state(false);
  let form = $state<SvelteComponent>();
  let setValue: (value: Profile) => void = $derived(
    form?.setValue || (() => undefined)
  );

  const profileEdit = {
    form: data.profileEdit,
    schema: profileEditSchema
  };

  $effect(() => {
    if (profile) {
      setValue(profile);
    }
  });
</script>

{#if profile}
  <BaseForm
    form={profileEdit}
    confirm="Save"
    bind:isLoading
    bind:this={form}
    onsubmit={() => undefined}
  >
    {#snippet children({ props })}
      <FormImage
        key="icon"
        class="size-20"
        type="file"
        label="Icon"
        {...props}
      />
      <FormInput
        label="Name"
        placeholder="Profile Name"
        key="name"
        {...props}
      />
    {/snippet}
    {#snippet footer({ children })}
      {@render children()}
    {/snippet}
  </BaseForm>
{/if}
