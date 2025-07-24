<script lang="ts">
  import { profile_get_icon } from '$lib/tauri/profile.svelte';
  import { Box } from '@lucide/svelte';
  import { cn } from 'positron-components/utils';
  import { Avatar } from 'positron-components/components/ui';
  import { create_data_state, UpdateType } from '$lib/data_state.svelte';

  interface Props {
    id: string;
    class?: string;
    classFallback?: string;
  }

  let { id, class: className, classFallback }: Props = $props();

  let icon_updater = $derived(
    create_data_state(async () => {
      return await profile_get_icon(id);
    }, UpdateType.Profiles)
  );
  let icon = $derived(icon_updater?.value);
</script>

<Avatar.Root class={cn('size-12 rounded-md', className)}>
  {#if icon}
    <Avatar.Image class="object-cover" src={`data:image/png;base64, ${icon}`} />
  {:else}
    <div class="flex size-full items-center justify-center">
      <Box class={cn('size-10', classFallback)} />
    </div>
  {/if}
</Avatar.Root>
