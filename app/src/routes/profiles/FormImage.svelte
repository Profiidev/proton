<script lang="ts">
  import { type SuperForm } from 'sveltekit-superforms';
  import { Form, Avatar } from 'positron-components/components/ui';
  import type { HTMLInputAttributes } from 'svelte/elements';
  import { Upload } from 'lucide-svelte';

  interface Props {
    formData: SuperForm<any>;
    key: string;
    disabled?: boolean;
    class?: string;
  }

  let {
    formData: form,
    key,
    disabled,
    class: className,
    ...restProps
  }: HTMLInputAttributes & Props = $props();

  const { form: formData } = $derived(form);

  let src = $state('');
</script>

<Form.Field {form} name={key} class="gap-1/2 grid">
  <Form.Control>
    {#snippet children({ props })}
      <label for={key} class="hover:cursor-pointer">
        <Avatar.Root
          class="ring-accent ring-offset-background size-20 rounded-md ring-2 ring-offset-2"
        >
          <Avatar.Image class="object-cover" {src} />
          <Avatar.Fallback class="rounded-md">
            <Upload class="size-4" />
            <span class="sr-only">Upload image</span>
          </Avatar.Fallback>
        </Avatar.Root>
      </label>
      <input
        onchange={(e) => {
          const file = e.currentTarget.files?.[0];
          if (!file) return;
          src = URL.createObjectURL(file);
          (e.target! as HTMLInputElement).value = '';
        }}
        type="file"
        id={key}
        style="display: none;"
      />
    {/snippet}
  </Form.Control>
  <Form.FieldErrors />
</Form.Field>
