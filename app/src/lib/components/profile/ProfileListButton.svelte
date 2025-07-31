<script lang="ts">
  import {
    Button,
    type ButtonVariant
  } from 'positron-components/components/ui';
  import type { Component } from 'svelte';
  import ProfileIcon from './ProfileIcon.svelte';
  import { CirclePlay } from '@lucide/svelte';

  interface Props {
    onclick: () => void;
    onclickInner: () => void;
    innerIcon?: Component;
    innerVariant?: ButtonVariant;
    item: {
      id: string;
      name?: string;
      loader: string;
      version?: string;
    };
    text?: string;
  }

  let { onclick, onclickInner, item, text, innerVariant, ...restProps }: Props =
    $props();
  let innerIcon = $derived({ icon: restProps.innerIcon || CirclePlay });
</script>

<Button
  variant="outline"
  class="group relative flex h-16 w-full max-w-86 cursor-pointer flex-row justify-start p-2"
  {onclick}
>
  <ProfileIcon id={item.id} />
  <div class="ml-2 flex min-w-0 flex-1 flex-col justify-start gap-2">
    <p class="truncate text-start text-sm">
      {text ? text : item.name || 'unknown'}
    </p>
    <p class="text-muted-foreground truncate text-start text-sm">
      {item.loader + ' ' + item.version || 'unknown'}
    </p>
  </div>
  <div class="bg-background absolute hidden rounded group-hover:flex">
    <Button
      class="size-12 cursor-pointer"
      size="icon"
      variant={innerVariant || 'default'}
      onclick={(e) => {
        e.stopPropagation();
        onclickInner();
      }}
    >
      <innerIcon.icon class="size-8" />
    </Button>
  </div>
</Button>
