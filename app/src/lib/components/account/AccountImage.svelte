<script lang="ts">
  import { account_get_skin } from '$lib/tauri/account.svelte';
  import { CircleHelp } from '@lucide/svelte';
  import { Skeleton } from 'positron-components/components/ui/skeleton';
  import * as Tooltip from 'positron-components/components/ui/tooltip';

  interface Props {
    skin_url?: string;
  }

  let { skin_url }: Props = $props();
</script>

{#await skin_url ? account_get_skin(skin_url) : Promise.resolve(undefined)}
  <Skeleton class="size-8" />
{:then skin}
  {#if skin}
    <img
      class="size-8 rounded"
      style="image-rendering: pixelated;"
      src={`data:image/png;base64, ${skin.head}`}
      alt=""
    />
  {:else}
    <Tooltip.Provider>
      <Tooltip.Root>
        <Tooltip.Trigger
          class="flex size-10 min-w-10 items-center justify-center"
        >
          <CircleHelp />
        </Tooltip.Trigger>
        <Tooltip.Content>
          <p>Image could not be loaded</p>
        </Tooltip.Content>
      </Tooltip.Root>
    </Tooltip.Provider>
  {/if}
{/await}
