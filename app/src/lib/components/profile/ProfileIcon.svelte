<script lang="ts">
  import { profile_get_icon } from '$lib/tauri/profile.svelte';
  import { Box } from '@lucide/svelte';
  import { cn } from 'positron-components/utils';
  import { Avatar } from 'positron-components/components/ui';

  interface Props {
    id: string;
    class?: string;
    classFallback?: string;
  }

  let { id, class: className, classFallback }: Props = $props();
</script>

<Avatar.Root class={cn('size-12 rounded-md', className)}>
  {#await profile_get_icon(id)}
    <Avatar.Fallback class="rounded-md">
      <span class="sr-only">Profile Icon</span>
    </Avatar.Fallback>
  {:then icon}
    {#if icon}
      <Avatar.Image
        class="object-cover"
        src={`data:image/png;base64, ${icon}`}
      />
    {:else}
      <div class="flex size-full items-center justify-center">
        <Box class={cn('size-10', classFallback)} />
      </div>
    {/if}
  {/await}
</Avatar.Root>
