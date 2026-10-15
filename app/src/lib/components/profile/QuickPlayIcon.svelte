<script lang="ts">
  import {
    profile_quick_play_icon,
    QuickPlayType,
    type QuickPlayInfo
  } from '$lib/tauri/quick-play.svelte';
  import { Globe, Server } from '@lucide/svelte';
  import { cn } from '@profidev/pleiades/utils';
  import * as Avatar from '@profidev/pleiades/components/ui/avatar';

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

{#snippet fallback()}
  <div class="bg-muted flex size-full items-center justify-center">
    {#if quickPlay.type === QuickPlayType.Singleplayer}
      <Globe class="size-5" />
    {:else}
      <Server class="size-5" />
    {/if}
  </div>
{/snippet}

<Avatar.Root
  class={cn(
    'size-12 shrink-0 rounded-md after:rounded-[inherit]',
    className,
    !iconExists && noFallback && 'hidden'
  )}
>
  {#await icon}
    {@render fallback()}
  {:then icon}
    {#if icon}
      <Avatar.Image
        class="rounded-[inherit] object-cover"
        src={`data:image/png;base64, ${icon}`}
      />
    {:else if !noFallback}
      {@render fallback()}
    {/if}
  {/await}
</Avatar.Root>
