<script lang="ts">
  import MainSkinViewer from "$lib/components/account/MainSkinViewer.svelte";
  import MiniSkinViewer from "$lib/components/account/MiniSkinViewer.svelte";
  import {
    account_active,
    account_change_cape,
    account_get_cape,
    account_get_skin,
    account_list,
    account_list_skins,
    State,
  } from "$lib/tauri/account.svelte";
  import {
    ScrollArea,
    Separator,
    Skeleton,
    toast,
  } from "positron-components/components/ui";

  let accounts = $derived(account_list.value);
  let active_account = $derived(account_active.value);
  let account = $derived(
    accounts && Object.entries(accounts).find((a) => a[0] === active_account),
  );
  let selected_cape = $derived(
    account?.[1]?.capes.find((s) => s.state === State.Active),
  );
  let selected_skin = $derived(
    account?.[1]?.skins.find((s) => s.state === State.Active),
  );

  const change = async (id: string) => {
    if (!active_account) return;

    if (await account_change_cape(id, active_account)) {
      await account_list_skins.update();
      await account_list.update();
      toast.success("Successfully changed Cape");
    } else {
      toast.error("Failed to change Cape");
    }
  };
</script>

<div class="ml-4 mt-2 flex-1 flex flex-col min-h-0">
  <div class="flex items-center">
    <p class="text-xl">Skins</p>
  </div>
  <div class="flex flex-1 min-h-0">
    <MainSkinViewer flipped={true} />
    <Separator class="mx-3" orientation="vertical" />
    <div class="h-full flex-1 flex flex-col min-h-0">
      <p class="my-2 w-full text-center">Library</p>
      {#if account && account[1] && account[1].capes.length > 0 && selected_skin}
        <div class="flex-1 min-h-0">
          <ScrollArea.ScrollArea class="h-full">
            <div class="grid w-full gap-3 grid-cols-[repeat(auto-fill,9rem)]">
              {#each account[1].capes as cape}
                {#await Promise.all( [account_get_cape(cape.url), account_get_skin(selected_skin.url)], )}
                  <Skeleton class="w-37 h-55" />
                {:then [cape_data, skin_data]}
                  {#if cape_data && skin_data}
                    <MiniSkinViewer
                      id={cape.id}
                      skin={skin_data.data}
                      cape={cape_data.data}
                      selected={cape.url === selected_cape?.url}
                      change_fn={change}
                      flipped={true}
                      delete_disabled={true}
                    />
                  {:else}
                    <Skeleton class="w-37 h-55" />
                  {/if}
                {/await}
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
