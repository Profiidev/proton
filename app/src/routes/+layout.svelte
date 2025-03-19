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
  import { onMount } from "svelte";
  import { account_refresh } from "$lib/tauri/account.svelte";
  let { children } = $props();

  setMode("dark");

  onMount(account_refresh);
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
