<script lang="ts">
  import { Button } from 'positron-components/components/ui';
  import { Minus, Square, X } from '@lucide/svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';

  const close = () => {
    getCurrentWindow().close();
  };

  const minimize = () => {
    getCurrentWindow().minimize();
  };

  const maximize = () => {
    getCurrentWindow().toggleMaximize();
  };
</script>

<header data-tauri-drag-region class="flex h-10 items-center">
  <Button
    class="ml-auto size-8 rounded-full"
    size="icon"
    variant="ghost"
    onclick={minimize}
  >
    <Minus class="size-5!" />
  </Button>
  <Button
    class="size-8 rounded-full"
    size="icon"
    variant="ghost"
    onclick={maximize}
  >
    <Square />
  </Button>
  <Button
    class="hover:bg-destructive mr-1 size-8 rounded-full"
    size="icon"
    variant="ghost"
    onclick={close}
  >
    <X class="size-5!" />
  </Button>
</header>
