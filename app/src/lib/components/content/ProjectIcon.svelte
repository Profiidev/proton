<script lang="ts">
  import { Box } from '@lucide/svelte';
  import { cn } from '@profidev/pleiades/utils';
  import * as Avatar from '@profidev/pleiades/components/ui/avatar';

  interface Props {
    url?: string;
    name: string;
    class?: string;
    classFallback?: string;
  }

  let { url, name, class: className, classFallback }: Props = $props();
</script>

<Avatar.Root
  class={cn(
    'bg-muted size-10 shrink-0 rounded-md after:rounded-[inherit]',
    className
  )}
>
  {#if url}
    <Avatar.Image class="rounded-[inherit] object-cover" src={url} alt={name} />
  {/if}
  <Avatar.Fallback class="rounded-[inherit]">
    <Box class={cn('text-muted-foreground size-1/2', classFallback)} />
  </Avatar.Fallback>
</Avatar.Root>
