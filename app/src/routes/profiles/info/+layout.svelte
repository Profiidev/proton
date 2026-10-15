<script lang="ts">
  import { goto } from '$app/navigation';
  import {
    profile_launch,
    profile_list,
    profile_open_path,
    profile_remove,
    profile_repair
  } from '$lib/tauri/profile.svelte.js';
  import { toast } from '@profidev/pleiades/components/util/general';
  import { Button } from '@profidev/pleiades/components/ui/button';
  // @ts-ignore
  import * as DropdownMenu from '@profidev/pleiades/components/ui/dropdown-menu';
  import { setProfile } from './store.svelte.js';
  import {
    Ellipsis,
    FastForward,
    FolderOpen,
    Monitor,
    Play,
    Plus,
    SlidersHorizontal,
    Square,
    Star,
    Terminal,
    Trash,
    Wrench
  } from '@lucide/svelte';
  import ProfileIcon from '$lib/components/profile/ProfileIcon.svelte';
  import { DateTime as D } from '@profidev/pleiades/util/time.svelte';
  import { account_active } from '$lib/tauri/account.svelte.js';
  import { profile_favorites_set } from '$lib/tauri/home.svelte.js';
  import {
    instance_list,
    instance_stop_all
  } from '$lib/tauri/instance.svelte.js';
  import { running_label } from '$lib/instances';
  import DestroyDialog from '$lib/components/form/DestroyDialog.svelte';
  import { is_offline } from '$lib/tauri/offline.svelte.js';
  import SimpleSidebar from '$lib/components/nav/SimpleSidebar.svelte';

  let { data, children } = $props();

  let offline = $derived(is_offline.value);
  let active_account = $derived(account_active.value);
  let profiles = $derived(profile_list.value);
  let profile = $derived(
    profiles ? profiles.find((p) => p.id === data.id) : null
  );
  let running = $derived(
    instance_list.value?.filter((i) => i.profile_id === data.id).length ?? 0
  );
  let deleteOpen = $state(false);
  let stopOpen = $state(false);

  $effect(() => {
    if (profile === undefined) {
      goto('/profiles');
    } else if (profile) {
      setProfile(profile);
    }
  });

  const format_date = (iso: string) =>
    D.DateTime?.fromISO(iso)
      .setLocale('de')
      .toLocaleString(D.DateTime.DATETIME_SHORT);

  let items = $derived([
    {
      href: `/profiles/info/quick_play?id=${profile?.id}`,
      title: 'Quick play',
      icon: FastForward
    },
    {
      href: `/profiles/info/instances?id=${profile?.id}`,
      title: 'Instances',
      icon: Monitor
    },
    {
      href: `/profiles/info/logs?id=${profile?.id}`,
      title: 'Logs',
      icon: Terminal
    },
    {
      href: `/profiles/info/settings?id=${profile?.id}`,
      title: 'Settings',
      icon: SlidersHorizontal
    }
  ]);
</script>

{#if profile}
  <div class="flex h-full min-h-0 flex-col gap-4 px-5 pt-4 pb-6">
    <header
      class="flex shrink-0 flex-wrap items-center gap-4 border-b pt-1 pb-4"
    >
      <ProfileIcon
        id={profile.id}
        class="bg-muted size-20 rounded-lg"
        classFallback="size-10"
      />
      <div class="flex min-w-0 flex-[1_1_260px] flex-col gap-1">
        <div class="flex flex-wrap items-center gap-2">
          <h1 class="text-2xl font-semibold tracking-tight">
            {profile.name}
          </h1>
          {#if running > 0}
            <span
              class="inline-flex h-5 items-center gap-1 rounded-full bg-green-500/15 px-2 text-xs font-medium text-green-500"
            >
              <span class="size-1.5 rounded-full bg-current"></span>
              {running_label(running)}
            </span>
          {/if}
        </div>
        <p class="text-muted-foreground text-sm">
          {profile.loader}
          {profile.version}
        </p>
        <p class="text-muted-foreground text-xs">
          Created {format_date(profile.created_at)} · Last played {profile.last_played
            ? format_date(profile.last_played)
            : 'Never'}
        </p>
      </div>
      <div class="flex items-center gap-2">
        {#if running > 0}
          <Button
            variant="outline"
            onclick={() =>
              profile_launch(profile.id, profile.name, active_account)}
          >
            <Plus data-icon="inline-start" />
            Launch another
          </Button>
          <Button variant="destructive" onclick={() => (stopOpen = true)}>
            <Square data-icon="inline-start" />
            {running > 1 ? 'Stop all' : 'Stop'}
          </Button>
        {:else}
          <Button
            onclick={() =>
              profile_launch(profile.id, profile.name, active_account)}
          >
            <Play data-icon="inline-start" />
            Play
          </Button>
        {/if}
        <Button
          size="icon"
          variant="outline"
          aria-label="Toggle favorite"
          onclick={() => profile_favorites_set(profile.id, !profile.favorite)}
        >
          <Star
            class={profile.favorite
              ? 'fill-yellow-500 text-yellow-500'
              : 'text-muted-foreground'}
          />
        </Button>
        <DropdownMenu.Root>
          <DropdownMenu.Trigger>
            {#snippet child({ props }: { props: Record<string, any> })}
              <Button
                variant="outline"
                size="icon"
                aria-label="Profile menu"
                {...props}
              >
                <Ellipsis />
              </Button>
            {/snippet}
          </DropdownMenu.Trigger>
          <DropdownMenu.Content align="end" class="w-[210px]">
            <DropdownMenu.Item onclick={() => profile_open_path(profile.id)}>
              <FolderOpen />
              Open directory
            </DropdownMenu.Item>
            <DropdownMenu.Item
              onclick={() => {
                if (offline) {
                  toast.warning(
                    'You are currently offline, please reconnect to the internet to repair a profile'
                  );
                  return;
                }
                profile_repair(profile.id, profile.name);
              }}
            >
              <Wrench />
              Repair profile
            </DropdownMenu.Item>
            <DropdownMenu.Item
              variant="destructive"
              onclick={() => (deleteOpen = true)}
            >
              <Trash />
              Delete profile
            </DropdownMenu.Item>
          </DropdownMenu.Content>
        </DropdownMenu.Root>
      </div>
    </header>
    <div class="flex min-h-0 grow gap-4">
      <aside class="w-40 shrink-0">
        <SimpleSidebar {items} />
      </aside>
      <div class="flex min-h-0 min-w-0 flex-1">
        {@render children()}
      </div>
    </div>
  </div>
  <DestroyDialog
    bind:open={deleteOpen}
    title="Delete Profile"
    description={`Are you sure you want to delete the profile "${profile?.name}"? This action cannot be undone.`}
    btnText="Delete"
    onclick={() => profile_remove(profile.id)}
  />
  <DestroyDialog
    bind:open={stopOpen}
    title={running > 1 ? 'Stop All Instances' : 'Stop Instance'}
    description={running > 1
      ? `Are you sure you want to stop all ${running} instances of profile "${profile?.name}"?`
      : `Are you sure you want to stop the instance of profile "${profile?.name}"?`}
    btnText={running > 1 ? 'Stop all' : 'Stop'}
    onclick={() => {
      instance_stop_all(profile.id);
      stopOpen = false;
    }}
  />
{:else}
  <p class="mt-2 ml-2">Loading...</p>
{/if}
