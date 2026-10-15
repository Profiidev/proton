<script lang="ts">
  import MainSkinViewer from '$lib/components/account/MainSkinViewer.svelte';
  import MiniSkinViewer from '$lib/components/account/MiniSkinViewer.svelte';
  import {
    account_active,
    account_add_skin,
    account_change_skin,
    account_list,
    account_list_skins,
    State
  } from '$lib/tauri/account.svelte';
  import { is_offline } from '$lib/tauri/offline.svelte';
  import { file_to_bytes } from '$lib/util.svelte';
  import { LoaderCircle } from '@lucide/svelte';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import { toast } from '@profidev/pleiades/components/util/general';

  let offline = $derived(is_offline.value);
  let skins = $derived(account_list_skins.value);
  let accounts = $derived(account_list.value);
  let active_account = $derived(account_active.value);
  let selected = $derived(
    active_account
      ? accounts?.[active_account]?.skins.find((s) => s.state === State.Active)
      : undefined
  );

  let upload_input: null | HTMLInputElement = $state(null);
  let add_loading = $state(false);

  const upload = async () => {
    if (!upload_input || !upload_input.files || !upload_input.files[0]) return;

    add_loading = true;
    let file = upload_input.files[0];
    let bytes = await file_to_bytes(file);

    if (!(await account_add_skin(bytes))) {
      toast.success('Successfully added Skin');
    } else {
      toast.error('Failed to add Skin');
    }

    add_loading = false;
    upload_input.value = '';
  };

  const change = async (id: string) => {
    if (!active_account) return;
    if (offline) {
      toast.warning(
        'You are currently offline, please reconnect to the internet to change Skin'
      );
      return;
    }

    if (!(await account_change_skin(id))) {
      toast.success('Successfully changed Skin');
    } else {
      toast.error('Failed to change Skin');
    }
  };
</script>

<div class="flex flex-wrap items-start gap-5">
  <MainSkinViewer />
  <div class="flex min-w-0 flex-[1_1_320px] flex-col gap-3">
    <div class="flex items-center gap-2">
      <div class="flex-1">
        <h1 class="text-xl font-semibold tracking-tight">Skins</h1>
        <p class="text-muted-foreground mt-0.5">Click a skin to wear it.</p>
      </div>
      <Button onclick={() => upload_input?.click()} disabled={add_loading}>
        {#if add_loading}
          <LoaderCircle class="animate-spin" />
        {/if}
        Add skin
      </Button>
      <input
        class="hidden"
        type="file"
        accept="image/png, image/jpeg"
        bind:this={upload_input}
        onchange={upload}
      />
    </div>
    {#if skins && skins.length > 0}
      <div class="grid grid-cols-[repeat(auto-fill,minmax(120px,1fr))] gap-2">
        {#each skins as skin (skin.id)}
          <MiniSkinViewer
            id={skin.id}
            label="Skin"
            skin={skin.data}
            selected={selected?.url === skin.url}
            change_fn={change}
          />
        {/each}
      </div>
    {:else}
      <p class="text-muted-foreground">No skins available</p>
    {/if}
  </div>
</div>
