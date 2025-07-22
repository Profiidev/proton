<script lang="ts" generics="S extends FormRecord = FormRecord">
  import { Form, Avatar, Input } from 'positron-components/components/ui';
  import type { HTMLInputAttributes } from 'svelte/elements';
  import { Upload } from '@lucide/svelte';
  import {
    type FormPath,
    type FormRecord,
    type SuperForm
  } from 'positron-components/components/form';
  import { get } from 'svelte/store';
  import { cn } from 'positron-components/utils';

  interface Props {
    formData: SuperForm<S>;
    key: FormPath<S>;
    disabled?: boolean;
    class?: string;
    label?: string;
  }

  let {
    formData: form,
    key,
    disabled,
    class: className,
    label,
    ...restProps
  }: HTMLInputAttributes & Props = $props();

  const { form: formData } = $derived(form);

  let files = $state<FileList | undefined>();
  let src = $derived(
    files && files.length > 0 ? URL.createObjectURL(files[0]) : ''
  );

  $effect(() => {
    let store = get(formData);
    if (files && files.length > 0) {
      // @ts-ignore
      store[key] = files[0];
    } else {
      // @ts-ignore
      store[key] = undefined;
    }
    formData.set(store);
  });
</script>

<Form.Field {form} name={key} class="gap-1/2 grid">
  <Form.Control>
    {#snippet children({ props })}
      <Form.Label>{label}</Form.Label>
      <label for={key} class="hover:cursor-pointer">
        <Avatar.Root
          class={cn(
            'ring-accent ring-offset-background size-20 rounded-md ring-2 ring-offset-2',
            className
          )}
        >
          <Avatar.Image class="object-cover" {src} />
          <Avatar.Fallback class="rounded-md">
            <Upload class="size-4" />
            <span class="sr-only">Upload image</span>
          </Avatar.Fallback>
        </Avatar.Root>
      </label>
      <Input
        {...restProps}
        id={key}
        type="file"
        bind:files
        {disabled}
        class="hidden"
        accept="image/png, image/jpeg, image/jpg"
      />
    {/snippet}
  </Form.Control>
  <Form.FieldErrors />
</Form.Field>
