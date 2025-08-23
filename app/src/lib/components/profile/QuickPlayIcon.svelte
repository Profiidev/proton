<script lang="ts">
  import {
    profile_quick_play_icon,
    type QuickPlayInfo
  } from '$lib/tauri/quick_play.svelte';
  import { Box } from '@lucide/svelte';
  import { cn } from 'positron-components/utils';
  import { Avatar } from 'positron-components/components/ui';

  interface Props {
    profileId: string;
    quickPlay: QuickPlayInfo;
    class?: string;
    iconExists?: boolean;
    noFallback?: boolean;
  }

  let {
    profileId,
    quickPlay,
    class: className,
    iconExists = $bindable(),
    noFallback
  }: Props = $props();

  let icon = $derived(
    profile_quick_play_icon(profileId, quickPlay).then((i) => {
      iconExists = !!i;
      return i;
    })
  );
</script>

<Avatar.Root
  class={cn(
    'size-12 rounded-md',
    className,
    !iconExists && noFallback && 'hidden'
  )}
>
  {#await icon}
    <div class="flex size-full items-center justify-center">
      <Box class="size-10" />
    </div>
  {:then icon}
    {#if icon}
      <Avatar.Image
        class="object-cover"
        src={`data:image/png;base64, ${icon}`}
      />
    {:else if !noFallback}
      <div class="flex size-full items-center justify-center">
        <Box class="size-10" />
      </div>
    {/if}
  {/await}
</Avatar.Root>
