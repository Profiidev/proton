<script lang="ts" generics="S extends FormRecord = FormRecord">
  import * as Form from 'positron-components/components/ui/form';
  import {
    type FormPath,
    type FormRecord,
    type SuperForm
  } from 'positron-components/components/form/types';
  import { get } from 'svelte/store';
  import ImageInput from './ImageInput.svelte';

  interface Props {
    formData: SuperForm<S>;
    key: FormPath<S>;
    disabled?: boolean;
    class?: string;
    label?: string;
    previewSrc?: string;
  }

  let { formData: form, key, ...restProps }: Props = $props();

  const { form: formData } = $derived(form);

  const fileChange = (file?: File) => {
    let store = get(formData);
    // @ts-ignore
    store[key] = file;
    formData.set(store);
  };
</script>

<Form.Field {form} name={key} class="gap-1/2 grid">
  <Form.Control>
    {#snippet children()}
      <ImageInput {key} {...restProps} {fileChange} labelComp={Form.Label} />
    {/snippet}
  </Form.Control>
  <Form.FieldErrors />
</Form.Field>
