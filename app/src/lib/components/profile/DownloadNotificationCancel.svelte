<script lang="ts">
  import { profile_cancel_download } from '$lib/tauri/profile.svelte';
  import { X } from '@lucide/svelte';
  import { Button, toast, Tooltip } from 'positron-components/components/ui';

  interface Props {
    id: number;
  }

  let { id }: Props = $props();

  const onclick = async () => {
    await profile_cancel_download(id);
    toast.warning('Check/Download canceled', {
      id,
      cancel: undefined,
      duration: undefined
    });
  };
</script>

<Tooltip.Provider>
  <Tooltip.Root>
    <Tooltip.Trigger>
      <Button
        variant="destructive"
        size="icon"
        class="size-6 cursor-pointer"
        {onclick}
      >
        <X />
      </Button>
    </Tooltip.Trigger>
    <!-- z-index is one higher that toast z-index -->
    <Tooltip.Content class="z-1000000000">
      <p>Cancel Check/Download</p>
    </Tooltip.Content>
  </Tooltip.Root>
</Tooltip.Provider>
