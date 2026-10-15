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
    LoginStatus,
    TOAST_DURATION
  } from '$lib/tauri/events.svelte';
  import { listen, type Event } from '@tauri-apps/api/event';
  import { CircleHelp, LoaderCircle, Plus, Trash2 } from '@lucide/svelte';
  import { toast } from '@profidev/pleiades/components/util/general';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import { Badge } from '@profidev/pleiades/components/ui/badge';
  import { is_offline } from '$lib/tauri/offline.svelte';

  let accounts: Accounts | undefined = $derived(account_list.value);
  let active = $derived(account_active.value);
  let add_loading = $state(false);
  let offline = $derived(is_offline.value);
  let login_toast: string | number | undefined;

  const change = async (id: string) => {
    if (await account_set_active(id)) {
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
    if (offline) {
      toast.warning(
        'You are currently offline, please reconnect to the internet to add an Account'
      );
      return;
    }

    add_loading = true;
    login_toast = toast.loading('Waiting for Microsoft Login', {
      duration: TOAST_DURATION,
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

  $effect(() => {
    const unlisten = listen(
      ACCOUNT_LOGIN_STATUS_EVENT,
      (e: Event<LoginStatus>) => {
        if (login_toast === undefined) return;

        const message = {
          [LoginStatus.Ms]: 'Logging in to Xbox',
          [LoginStatus.Xbox]: 'Logging in to Xbox Security',
          [LoginStatus.XboxSecurity]: 'Logging in to Minecraft',
          [LoginStatus.Mc]: 'Retrieving Minecraft Profile'
        }[e.payload];
        if (message) {
          toast.loading(message, { id: login_toast, duration: TOAST_DURATION });
        }
      }
    );
    return () => unlisten.then((u) => u());
  });
</script>

<div class="flex items-center gap-2">
  <div class="flex-1">
    <h1 class="text-xl font-semibold tracking-tight">Accounts</h1>
    <p class="text-muted-foreground mt-0.5">
      The account marked Selected is used when you press Play.
    </p>
  </div>
  <Button onclick={add} disabled={add_loading}>
    {#if add_loading}
      <LoaderCircle class="animate-spin" />
    {:else}
      <Plus />
    {/if}
    Add account
  </Button>
</div>
{#if accounts && Object.keys(accounts).length > 0}
  {#each Object.entries(accounts).sort( (a, b) => a[0].localeCompare(b[0]) ) as [id, info] (id)}
    <div
      class="bg-card ring-foreground/10 flex items-center gap-3 rounded-xl px-3.5 py-3 ring-1"
    >
      {#if info}
        <AccountImage
          skin_url={info.skins.find((s) => s.state === State.Active)?.url}
        />
      {:else}
        <CircleHelp class="size-8 shrink-0 p-1" />
      {/if}
      <div class="flex min-w-0 flex-1 flex-col">
        <span class="truncate font-medium">{info?.name ?? 'Login failed'}</span>
        <span class="text-muted-foreground truncate font-mono text-xs">
          {info?.id ?? id}
        </span>
      </div>
      {#if id === active}
        <Badge>Selected</Badge>
      {:else if info}
        <Button variant="outline" onclick={() => change(id)}>Select</Button>
      {/if}
      <Button
        variant="ghost"
        size="icon"
        aria-label="Remove account"
        onclick={() => remove(id)}
      >
        <Trash2 class="text-destructive" />
      </Button>
    </div>
  {/each}
{:else}
  <p class="text-muted-foreground">No accounts available</p>
{/if}
