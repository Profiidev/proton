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
  import { file_to_bytes } from '$lib/util.svelte';
  import { LoaderCircle, Plus } from '@lucide/svelte';
  import {
    Button,
    Input,
    ScrollArea,
    Separator,
    toast
  } from 'positron-components/components/ui';

  let skins = $derived(account_list_skins.value);
  let accounts = $derived(account_list.value);
  let active_account = $derived(account_active.value);
  let account = $derived(
    accounts && Object.entries(accounts).find((a) => a[0] === active_account)
  );
  let selected = $derived(
    account?.[1]?.skins.find((s) => s.state === State.Active)
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

    if (!(await account_change_skin(id))) {
      toast.success('Successfully changed Skin');
    } else {
      toast.error('Failed to change Skin');
    }
  };
</script>

<div class="mt-2 ml-4 flex min-h-0 flex-1 flex-col">
  <div class="flex items-center">
    <p class="text-xl">Skins</p>
  </div>
  <div class="flex min-h-0 flex-1">
    <MainSkinViewer />
    <Separator class="mx-3" orientation="vertical" />
    <div class="flex h-full min-h-0 flex-1 flex-col">
      <div class="relative">
        <p class="my-2 w-full text-center">Library</p>
        <Button
          size="icon"
          class="absolute right-0 bottom-0 mr-1 mb-2 size-6 cursor-pointer"
          onclick={() => upload_input?.click()}
          disabled={add_loading}
        >
          {#if add_loading}
            <LoaderCircle class="animate-spin" />
          {:else}
            <Plus />
          {/if}
        </Button>
        <Input
          class="hidden"
          type="file"
          accept="image/png, image/jpeg"
          bind:ref={upload_input}
          onchange={upload}
        />
      </div>
      {#if skins && skins.length > 0}
        <div class="min-h-0 flex-1">
          <ScrollArea.ScrollArea class="h-full">
            <div class="grid w-full grid-cols-[repeat(auto-fill,9rem)] gap-3">
              {#each skins as skin}
                <MiniSkinViewer
                  id={skin.id}
                  skin={skin.data}
                  selected={selected?.url === skin.url}
                  change_fn={change}
                />
              {/each}
            </div>
          </ScrollArea.ScrollArea>
        </div>
      {:else}
        <div class="flex flex-1 items-center justify-center">
          <p>No Skins available</p>
        </div>
      {/if}
    </div>
  </div>
</div>
