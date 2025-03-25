<script lang="ts">
  import { profile_create, ProfileError } from '$lib/tauri/profile.svelte';
  import { FormDialog, FormInput } from 'positron-components/components/form';
  import type { PageServerData } from './$types';
  import { profileCreateSchema } from './schema.svelte';
  import type { SuperValidated } from 'sveltekit-superforms';

  interface Props {
    data: PageServerData;
  }

  let { data }: Props = $props();

  const profileCreate = {
    form: data.profileCreate as SuperValidated<any>,
    schema: profileCreateSchema
  };

  const createProfile = async (form: SuperValidated<any>) => {
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
    text: 'Test'
  }}
  form={profileCreate}
  onsubmit={createProfile}
>
  {#snippet children({ props })}
    <FormInput label="Name" placeholder="Name" key="name" {...props} />
  {/snippet}
</FormDialog>
