<script lang="ts">
  import AccountImage from '$lib/components/account/AccountImage.svelte';
  import {
    account_active,
    account_list,
    account_login,
    account_remove,
    account_set_active,
    State,
    type Accounts
  } from '$lib/tauri/account.svelte';
  import {
    ACCOUNT_LOGIN_STATUS_EVENT,
    LoginStatus
  } from '$lib/tauri/events.svelte';
  import { listen } from '@tauri-apps/api/event';
  import { LoaderCircle, Plus, Trash } from 'lucide-svelte';
  import {
    toast,
    Badge,
    Button,
    Separator
  } from 'positron-components/components/ui';

  let accounts: Accounts | undefined = $derived(account_list.value);
  let active = $derived(account_active.value);
  let add_loading = $state(false);
  let login_toast: string | number | undefined;
  //10 minutes
  const LOGIN_TOAST_DURATION = 600000;

  const change = async (id: string) => {
    if (!(await account_set_active(id))) {
      toast.error('Failed to switch Account');
    }
  };

  const remove = async (id: string) => {
    if (!(await account_remove(id))) {
      toast.success('Successfully removed Account');
    } else {
      toast.error('Failed to remove Account');
    }
  };

  const add = async () => {
    add_loading = true;
    login_toast = toast.loading('Waiting for Microsoft Login', {
      duration: LOGIN_TOAST_DURATION,
      id: login_toast
    });

    if (!(await account_login())) {
      toast.success('Successfully added Account');
    } else {
      toast.error('Failed to add Account');
    }

    toast.dismiss(login_toast);
    login_toast = undefined;
    add_loading = false;
  };

  listen(ACCOUNT_LOGIN_STATUS_EVENT, (e) => {
    if (!login_toast) return;

    switch (e.payload as LoginStatus) {
      case LoginStatus.Ms:
        toast.loading('Logging in to Xbox', {
          id: login_toast,
          duration: LOGIN_TOAST_DURATION
        });
        break;
      case LoginStatus.Xbox:
        toast.loading('Logging in to Xbox Security', {
          id: login_toast,
          duration: LOGIN_TOAST_DURATION
        });
      case LoginStatus.XboxSecurity:
        toast.loading('Logging in to Minecraft', {
          id: login_toast,
          duration: LOGIN_TOAST_DURATION
        });
      case LoginStatus.Mc:
        toast.loading('Retrieving Minecraft Profile', {
          id: login_toast,
          duration: LOGIN_TOAST_DURATION
        });
    }
  });
</script>

<div class="mt-2 ml-4 flex-1">
  <div class="flex items-center">
    <p class="text-xl">Accounts</p>
    <Button
      size="icon"
      class="mr-3.5 ml-auto size-8"
      onclick={add}
      disabled={add_loading}
    >
      {#if add_loading}
        <LoaderCircle class="animate-spin" />
      {:else}
        <Plus />
      {/if}
    </Button>
  </div>
  <div class="mt-2 rounded-lg border">
    {#if accounts && Object.entries(accounts).length > 0}
      {#each Object.entries(accounts).sort( (a, b) => a[0].localeCompare(b[0]) ) as [id, info], i}
        <Button
          class="h-14 w-full gap-2 p-3"
          variant="ghost"
          onclick={() => change(id)}
        >
          {#if info}
            {@const skin_url = info.skins.find(
              (s) => s.state === State.Active
            )?.url}
            <AccountImage {skin_url} />
            <div class="flex min-w-0 flex-1 flex-col justify-start">
              <p class="truncate text-start text-sm">{info.name}</p>
              <p class="text-muted-foreground truncate text-start text-sm">
                {info.id}
              </p>
            </div>
            {#if id === active}
              <Badge class="mr-2">Selected</Badge>
            {/if}
            <Button
              variant="destructive"
              size="icon"
              class="size-8"
              onclick={(e) => {
                e.stopPropagation();
                remove(id);
              }}
            >
              <Trash />
            </Button>
          {:else}
            <p>{id}</p>
          {/if}
        </Button>
        {#if i !== Object.entries(accounts).length - 1}
          <Separator />
        {/if}
      {/each}
    {:else}
      <p class="p-4">No Accounts available</p>
    {/if}
  </div>
</div>
