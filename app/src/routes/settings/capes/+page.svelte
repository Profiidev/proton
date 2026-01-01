<script lang="ts">
  import MainSkinViewer from '$lib/components/account/MainSkinViewer.svelte';
  import MiniSkinViewer from '$lib/components/account/MiniSkinViewer.svelte';
  import {
    account_active,
    account_change_cape,
    account_get_cape,
    account_get_skin,
    account_list,
    REMOVE_CAPE,
    State
  } from '$lib/tauri/account.svelte';
  import { is_offline } from '$lib/tauri/offline.svelte';
  import { LoaderCircle } from '@lucide/svelte';
  import { Separator } from 'positron-components/components/ui/separator';
  import { toast } from 'positron-components/components/util/general';
  import { ScrollArea } from 'positron-components/components/ui/scroll-area';

  let offline = $derived(is_offline.value);
  let accounts = $derived(account_list.value);
  let active_account = $derived(account_active.value);
  let account = $derived(
    accounts && Object.entries(accounts).find((a) => a[0] === active_account)
  );
  let selected_cape = $derived(
    account?.[1]?.capes.find((s) => s.state === State.Active)
  );
  let selected_skin = $derived(
    account?.[1]?.skins.find((s) => s.state === State.Active)
  );

  const change = async (id: string) => {
    if (!active_account) return;
    if (offline) {
      toast.warning(
        'You are currently offline, please reconnect to the internet to change Cape'
      );
      return;
    }

    if (!(await account_change_cape(id))) {
      toast.success('Successfully changed Cape');
    } else {
      toast.error('Failed to change Cape');
    }
  };
</script>

<div class="mt-2 ml-4 flex min-h-0 flex-1 flex-col">
  <div class="flex items-center">
    <p class="text-xl">Capes</p>
  </div>
  <div class="flex min-h-0 flex-1">
    <MainSkinViewer flipped={true} />
    <Separator class="mx-3" orientation="vertical" />
    <div class="flex h-full min-h-0 flex-1 flex-col">
      <p class="my-2 w-full text-center">Library</p>
      {#if account && account[1] && account[1].capes.length > 0 && selected_skin}
        <div class="min-h-0 flex-1">
          <ScrollArea class="h-full">
            <div class="grid w-full grid-cols-[repeat(auto-fill,9rem)] gap-3">
              {#each account[1].capes as cape}
                {#await Promise.all( [account_get_cape(cape.url), account_get_skin(selected_skin.url)] )}
                  {@render loader()}
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
                    {@render loader()}
                  {/if}
                {/await}
              {/each}
              {#await account_get_skin(selected_skin.url)}
                {@render loader()}
              {:then skin_data}
                {#if skin_data}
                  <MiniSkinViewer
                    id={REMOVE_CAPE}
                    skin={skin_data.data}
                    selected={!selected_cape}
                    change_fn={change}
                    flipped={true}
                    delete_disabled={true}
                  />
                {:else}
                  {@render loader()}
                {/if}
              {/await}
            </div>
          </ScrollArea>
        </div>
      {:else}
        <div class="flex flex-1 items-center justify-center">
          <p>No Capes available</p>
        </div>
      {/if}
    </div>
  </div>
</div>

{#snippet loader()}
  <div class="flex h-55 w-37 items-center justify-center">
    <LoaderCircle class="size-10 animate-spin" />
  </div>
{/snippet}
