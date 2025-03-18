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
  <div class="flex-grow-1 flex">
    <Sidebar />
    <Separator orientation="vertical" />
    <main class="flex-grow-1 m-2">
      {@render children()}
    </main>
  </div>
</div>
