<script lang="ts">
  import { Upload } from '@lucide/svelte';
  import { cn } from 'positron-components/utils';
  import { Input } from 'positron-components/components/ui/input';
  import * as Avatar from 'positron-components/components/ui/avatar';
  import { Label } from 'positron-components/components/ui/label';
  import type { Component } from 'svelte';

  interface Props {
    label?: string;
    key: string;
    class?: string;
    disabled?: boolean;
    previewSrc?: string;
    labelComp?: Component;
    fileChange: (files: File | undefined) => void;
  }

  let {
    label,
    key,
    class: className,
    disabled,
    previewSrc,
    labelComp,
    fileChange
  }: Props = $props();

  let files = $state<FileList | undefined>();
  let src = $state('');
  let LabelComp = labelComp ?? Label;

  $effect(() => {
    if (files && files.length > 0) {
      src = URL.createObjectURL(files[0]);
    } else if (previewSrc) {
      src = previewSrc;
    }
  });

  const onChange = (e: Event) => {
    let input = e.target as HTMLInputElement;
    files = input.files ?? undefined;

    if (files && files.length > 0) {
      fileChange(files[0]);
    } else {
      fileChange(undefined);
    }
  };
</script>

{#if label}
  <LabelComp>{label}</LabelComp>
{/if}
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
  id={key}
  type="file"
  bind:files
  {disabled}
  class="hidden"
  accept="image/png, image/jpeg, image/jpg"
  onchange={onChange}
/>
