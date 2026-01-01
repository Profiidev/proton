<script lang="ts">
  import { profile_get_icon } from '$lib/tauri/profile.svelte';
  import { Box } from '@lucide/svelte';
  import { cn } from 'positron-components/utils';
  import * as Avatar from 'positron-components/components/ui/avatar';
  import { create_data_state, UpdateType } from '$lib/data_state.svelte';

  interface Props {
    id: string;
    class?: string;
    classFallback?: string;
  }

  let { id, class: className, classFallback }: Props = $props();

  let icon = $state<string>();
  let icon_updater = $derived(
    id
      ? create_data_state(async () => {
          icon = await profile_get_icon(id);
          return icon;
        }, UpdateType.Profiles)
      : undefined
  );
  let x = $derived(icon_updater?.value);
</script>

<!-- we need to use x so the updater is active or the icon will not be updated -->
<p class="absolute top-1000 left-1000 hidden">{x?.charAt(0)}</p>
<Avatar.Root class={cn('size-12 rounded-md', className)}>
  {#if icon}
    <Avatar.Image class="object-cover" src={`data:image/png;base64, ${icon}`} />
  {:else}
    <div class="flex size-full items-center justify-center">
      <Box class={cn('size-10', classFallback)} />
    </div>
  {/if}
</Avatar.Root>
