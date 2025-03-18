<script lang="ts">
  import { setMode } from "mode-watcher";
  import { ModeWatcher, Toaster } from "positron-components/components/ui";
  import "../app.css";
  import Header from "$lib/components/Header.svelte";
  import ProfileSelector from "$lib/components/ProfileSelector.svelte";
  let { children } = $props();

  setMode("dark");
</script>

<ModeWatcher />
<Toaster position="top-right" closeButton={true} richColors={true} />

<div class="h-full flex flex-col">
  <Header />
  <ProfileSelector />
  <main class="flex-grow-1">
    {@render children()}
  </main>
</div>
