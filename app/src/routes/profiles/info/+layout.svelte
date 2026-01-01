<script lang="ts">
  import { goto } from '$app/navigation';
  import {
    profile_launch,
    profile_list,
    profile_open_path,
    profile_remove,
    profile_repair
  } from '$lib/tauri/profile.svelte.js';
  import { toast } from 'positron-components/components/util/general';
  import { Button } from 'positron-components/components/ui/button';
  // @ts-ignore
  import * as DropdownMenu from 'positron-components/components/ui/dropdown-menu';
  import { setProfile } from './store.svelte.js';
  import { FolderOpen, Menu, Play, Star, Trash, Wrench } from '@lucide/svelte';
  import ProfileIcon from '$lib/components/profile/ProfileIcon.svelte';
  import { DateTime } from 'positron-components/util/time.svelte';
  import { account_active } from '$lib/tauri/account.svelte.js';
  import { profile_favorites_set } from '$lib/tauri/home.svelte.js';
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
  let deleteOpen = $state(false);

  $effect(() => {
    if (profile === undefined) {
      goto('/profiles');
    } else if (profile) {
      setProfile(profile);
    }
  });

  let items = $derived([
    {
      href: `/profiles/info/quick_play?id=${profile?.id}`,
      title: 'Quick Play'
    },
    {
      href: `/profiles/info/instances?id=${profile?.id}`,
      title: 'Instances'
    },
    {
      href: `/profiles/info/logs?id=${profile?.id}`,
      title: 'Old Logs'
    },
    {
      href: `/profiles/info/settings?id=${profile?.id}`,
      title: 'Settings'
    }
  ]);
</script>

{#if profile}
  <div class="flex h-full flex-col">
    <div class="mt-2 ml-2 flex">
      <ProfileIcon
        id={profile.id}
        class="size-24 border-2"
        classFallback="size-20"
      />
      <div class="my-2 ml-4 flex flex-col gap-1">
        <p class="text-xl">{profile.name}</p>
        <p class="text-muted-foreground whitespace-nowrap">
          {profile.loader}
          {profile.version}
        </p>
        <div class="flex flex-wrap">
          <p class="text-muted-foreground mr-4 whitespace-nowrap">
            Created at: {DateTime.fromISO(profile.created_at)
              .setLocale('de')
              .toLocaleString(DateTime.DATETIME_SHORT)}
          </p>
          <p class="text-muted-foreground whitespace-nowrap">
            Last Played: {profile.last_played
              ? DateTime.fromISO(profile.last_played)
                  .setLocale('de')
                  .toLocaleString(DateTime.DATETIME_SHORT)
              : 'Never'}
          </p>
        </div>
      </div>
      <div class="mr-2 ml-auto flex items-center gap-2">
        <Button
          onclick={() =>
            profile_launch(profile.id, profile.name, active_account)}
          class="cursor-pointer"
        >
          <Play />
          Play
        </Button>
        <Button
          size="icon"
          variant="outline"
          onclick={() => {
            profile_favorites_set(profile.id, !profile.favorite);
          }}
          class="cursor-pointer"
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
                {...props}
                class="cursor-pointer"
              >
                <Menu />
              </Button>
            {/snippet}
          </DropdownMenu.Trigger>
          <DropdownMenu.Content>
            <DropdownMenu.Item
              onclick={() => profile_open_path(profile.id)}
              class="cursor-pointer"
            >
              <FolderOpen />
              Open Directory</DropdownMenu.Item
            >
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
              class="cursor-pointer"
            >
              <Wrench />
              Repair Profile
            </DropdownMenu.Item>
            <DropdownMenu.Item
              variant="destructive"
              class="cursor-pointer"
              onclick={() => (deleteOpen = true)}
            >
              <Trash />
              Delete Profile
            </DropdownMenu.Item>
          </DropdownMenu.Content>
        </DropdownMenu.Root>
      </div>
    </div>
    <div class="mt-2 flex min-h-0 grow flex-col gap-2 lg:flex-row">
      <aside class="lg:w-52 lg:max-w-32 lg:min-w-32">
        <SimpleSidebar {items} class="" />
      </aside>
      <div
        class="flex min-h-0 flex-1 space-y-8 lg:h-full lg:space-y-0 lg:space-x-12"
      >
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
{:else}
  <p class="mt-2 ml-2">Loading...</p>
{/if}
