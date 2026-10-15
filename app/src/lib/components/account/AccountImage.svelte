<script lang="ts">
  import { account_get_skin, type SkinData } from '$lib/tauri/account.svelte';
  import { CircleHelp } from '@lucide/svelte';
  import { Skeleton } from '@profidev/pleiades/components/ui/skeleton';
  import * as Tooltip from '@profidev/pleiades/components/ui/tooltip';

  interface Props {
    skin_url?: string;
  }

  let { skin_url }: Props = $props();

  // State instead of {#await}, so account refreshes do not flash the skeleton
  let skin = $state<SkinData>();
  let loading = $state(true);

  $effect(() => {
    const url = skin_url;
    if (!url) {
      skin = undefined;
      loading = false;
      return;
    }
    account_get_skin(url).then((data) => {
      skin = data;
      loading = false;
    });
  });
</script>

{#if loading}
  <Skeleton class="size-8" />
{:else}
  {#if skin}
    <img
      class="size-8 shrink-0 rounded"
      style="image-rendering: pixelated;"
      src={`data:image/png;base64, ${skin.head}`}
      alt=""
    />
  {:else}
    <Tooltip.Provider>
      <Tooltip.Root>
        <Tooltip.Trigger
          class="flex size-8 shrink-0 items-center justify-center"
        >
          <CircleHelp />
        </Tooltip.Trigger>
        <Tooltip.Content>
          <p>Image could not be loaded</p>
        </Tooltip.Content>
      </Tooltip.Root>
    </Tooltip.Provider>
  {/if}
{/if}
