<script lang="ts">
  import MainSkinViewer from "$lib/components/account/MainSkinViewer.svelte";
  import MiniSkinViewer from "$lib/components/account/MiniSkinViewer.svelte";
  import { account_list_skins } from "$lib/tauri/account.svelte";
  import { ScrollArea, Separator } from "positron-components/components/ui";

  let skins = $derived(account_list_skins.value);
</script>

<div class="ml-4 mt-2 flex-1 flex flex-col min-h-0">
  <div class="flex items-center">
    <p class="text-xl">Skins</p>
  </div>
  <div class="flex flex-1 min-h-0">
    <MainSkinViewer />
    <Separator class="mx-3" orientation="vertical" />
    <div class="h-full flex-1 flex flex-col min-h-0">
      <p class="my-2 w-full text-center">Library</p>
      {#if skins && skins.length > 0}
        <div class="flex-1 min-h-0">
          <ScrollArea.ScrollArea class="h-full">
            <div class="grid w-full gap-3 grid-cols-[repeat(auto-fill,9rem)]">
              {#each skins as skin}
                <MiniSkinViewer skin={skin.data} />
              {/each}
            </div>
          </ScrollArea.ScrollArea>
        </div>
      {:else}
        <div class="flex items-center justify-center flex-1">
          <p>No Skins available</p>
        </div>
      {/if}
    </div>
  </div>
</div>
