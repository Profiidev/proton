<script lang="ts">
  import { setMode, ModeWatcher } from "mode-watcher";
  import "../app.css";
  import Header from "$lib/components/Header.svelte";
    import ProfileSelector from "$lib/components/ProfileSelector.svelte";
  let { children } = $props();

  setMode("dark");
</script>

<ModeWatcher />

<div class="h-full flex flex-col">
  <Header />
  <ProfileSelector />
  <main class="flex-grow-1">
    {@render children()}
  </main>
</div>
