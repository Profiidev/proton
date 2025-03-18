<script lang="ts">
  import { setMode } from "mode-watcher";
  import {
    ModeWatcher,
    Separator,
    Toaster,
  } from "positron-components/components/ui";
  import "../app.css";
  import Header from "$lib/components/Header.svelte";
  import Sidebar from "$lib/components/Sidebar.svelte";
  let { children } = $props();

  setMode("dark");
</script>

<ModeWatcher />
<Toaster position="top-right" closeButton={true} richColors={true} />

<div class="h-full flex flex-col">
  <Header />
  <Separator />
  <div class="flex-1 flex min-h-0">
    <Sidebar />
    <Separator orientation="vertical" />
    <main class="flex-1 m-2 min-h-0">
      {@render children()}
    </main>
  </div>
</div>
