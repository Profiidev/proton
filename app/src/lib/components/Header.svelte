<script lang="ts">
  import { Button } from "positron-components";
  import { Minus, Square, X } from "lucide-svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";

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

<header data-tauri-drag-region class="h-10 flex items-center">
  <Button
    class="ml-auto rounded-full size-8"
    size="icon"
    variant="ghost"
    onclick={minimize}
  >
    <Minus class="size-5!" />
  </Button>
  <Button
    class="rounded-full size-8"
    size="icon"
    variant="ghost"
    onclick={maximize}
  >
    <Square />
  </Button>
  <Button
    class="rounded-full hover:bg-destructive size-8 mr-1"
    size="icon"
    variant="ghost"
    onclick={close}
  >
    <X class="size-5!" />
  </Button>
</header>
