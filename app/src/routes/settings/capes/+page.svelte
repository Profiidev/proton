<script lang="ts">
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
  import { toast } from '@profidev/pleiades/components/util/general';

  let offline = $derived(is_offline.value);
  let active_account = $derived(account_active.value);
  let account = $derived(
    active_account ? account_list.value?.[active_account] : undefined
  );
  let skin_url = $derived(
    account?.skins.find((s) => s.state === State.Active)?.url
  );
  let selected_cape = $derived(
    account?.capes.find((s) => s.state === State.Active)
  );

  // Textures live in state instead of {#await} blocks: account refreshes would
  // re-run those, flash their pending branch and rebuild every 3D tile
  let skin = $state<string>();
  let capes = $state<Record<string, string>>({});

  $effect(() => {
    const url = skin_url;
    if (url) {
      account_get_skin(url).then((data) => {
        skin = data?.data;
      });
    }
  });

  $effect(() => {
    for (const { url } of account?.capes ?? []) {
      account_get_cape(url).then((data) => {
        if (data && capes[url] !== data.data) {
          capes[url] = data.data;
        }
      });
    }
  });

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

<div>
  <h1 class="text-xl font-semibold tracking-tight">Capes</h1>
  <p class="text-muted-foreground mt-0.5">
    Capes owned by {account?.name ?? 'this account'}. Choose none to hide your
    cape.
  </p>
</div>
{#if !account}
  <p class="text-muted-foreground">No capes available</p>
{:else if skin}
  <div class="grid grid-cols-[repeat(auto-fill,minmax(140px,1fr))] gap-2">
    <MiniSkinViewer
      id={REMOVE_CAPE}
      label="No cape"
      {skin}
      selected={!selected_cape}
      change_fn={change}
      flipped
      delete_disabled
      height={130}
    />
    {#each account.capes as cape (cape.id)}
      {#if capes[cape.url]}
        <MiniSkinViewer
          id={cape.id}
          label="Cape"
          {skin}
          cape={capes[cape.url]}
          selected={cape.url === selected_cape?.url}
          change_fn={change}
          flipped
          delete_disabled
          height={130}
        />
      {:else}
        {@render loader()}
      {/if}
    {/each}
  </div>
{:else}
  {@render loader()}
{/if}

{#snippet loader()}
  <div class="flex h-37 items-center justify-center">
    <LoaderCircle class="size-8 animate-spin" />
  </div>
{/snippet}
