<script lang="ts">
  import { goto } from '$app/navigation';
  import ProfileIcon from '$lib/components/profile/ProfileIcon.svelte';
  import { account_active } from '$lib/tauri/account.svelte';
  import {
    profile_favorites_list,
    profile_history_list,
    profile_launch
  } from '$lib/tauri/profile.svelte';
  import { compareProfiles } from '$lib/util.svelte';
  import { CirclePlay, ExternalLink } from '@lucide/svelte';
  import { Button, Separator } from 'positron-components/components/ui';

  let favorites = $derived(profile_favorites_list.value);
  let sorted_favorites = $derived(
    (favorites ?? []).toSorted((a, b) => compareProfiles(a.profile, b.profile))
  );
  let history = $derived(profile_history_list.value);
  let sorted_history = $derived(
    (history ?? []).toSorted((a, b) => compareProfiles(a.profile, b.profile))
  );
  let active_account = $derived(account_active.value);
</script>

<div class="size-full">
  <p class="text-md font-bold">Favorites</p>
  <Separator class="my-2" />
  {#if sorted_favorites.length > 0}
    <div
      class="grid h-auto w-full auto-rows-min grid-cols-[repeat(auto-fill,_minmax(14rem,_1fr))] gap-2"
    >
      {#each sorted_favorites as favorite}
        <Button
          variant="outline"
          class="group relative flex h-16 w-full max-w-86 cursor-pointer flex-row justify-start p-2"
          onclick={() =>
            goto(`/profiles/info/quick_play?id=${favorite.profile.id}`)}
        >
          <ProfileIcon id={favorite.profile.id} />
          <div class="ml-2 flex min-w-0 flex-1 flex-col justify-start gap-2">
            <p class="truncate text-start text-sm">
              {favorite.quick_play
                ? favorite.quick_play.name + ' (' + favorite.profile.name + ')'
                : favorite.profile.name || 'unknown'}
            </p>
            <p class="text-muted-foreground truncate text-start text-sm">
              {favorite.profile.loader + ' ' + favorite.profile.version ||
                'unknown'}
            </p>
          </div>
          <div class="bg-background absolute hidden rounded group-hover:flex">
            <Button
              class="size-12 cursor-pointer"
              size="icon"
              onclick={(e) => {
                e.stopPropagation();
                profile_launch(
                  favorite.profile.id,
                  favorite.profile.name,
                  active_account,
                  favorite.quick_play
                );
              }}
            >
              <CirclePlay class="size-8" />
            </Button>
          </div>
        </Button>
      {/each}
    </div>
  {:else}
    <p class="text-muted-foreground text-center">
      No favorites found. Add a profile or quick play option to your favorites
      in the profile settings.
    </p>
    <div class="mt-2 flex justify-center">
      <Button
        variant="outline"
        onclick={() => goto('/profiles')}
        class="text-md inline-flex w-fit cursor-pointer p-0"
      >
        Profiles
        <ExternalLink />
      </Button>
    </div>
  {/if}
  <div class="mt-4 flex">
    <p class="text-md font-bold">History</p>
  </div>
  <Separator class="my-2" />
  {#if sorted_history.length > 0}
    <div
      class="grid h-auto w-full auto-rows-min grid-cols-[repeat(auto-fill,_minmax(14rem,_1fr))] gap-2"
    >
      {#each sorted_history as item}
        <Button
          variant="outline"
          class="group relative flex h-16 w-full max-w-86 cursor-pointer flex-row justify-start p-2"
          onclick={() =>
            goto(`/profiles/info/quick_play?id=${item.profile.id}`)}
        >
          <ProfileIcon id={item.profile.id} />
          <div class="ml-2 flex min-w-0 flex-1 flex-col justify-start gap-2">
            <p class="truncate text-start text-sm">
              {item.quick_play
                ? item.quick_play.name + ' (' + item.profile.name + ')'
                : item.profile.name || 'unknown'}
            </p>
            <p class="text-muted-foreground truncate text-start text-sm">
              {item.profile.loader + ' ' + item.profile.version || 'unknown'}
            </p>
          </div>
          <div class="bg-background absolute hidden rounded group-hover:flex">
            <Button
              class="size-12 cursor-pointer"
              size="icon"
              onclick={(e) => {
                e.stopPropagation();
                profile_launch(
                  item.profile.id,
                  item.profile.name,
                  active_account,
                  item.quick_play
                );
              }}
            >
              <CirclePlay class="size-8" />
            </Button>
          </div>
        </Button>
      {/each}
    </div>
  {:else}
    <p class="text-muted-foreground text-center">
      No entries found in history. Launch a profile to create one.
    </p>
    <div class="mt-2 flex justify-center">
      <Button
        variant="outline"
        onclick={() => goto('/profiles')}
        class="text-md inline-flex w-fit cursor-pointer p-0"
      >
        Profiles
        <ExternalLink />
      </Button>
    </div>
  {/if}
</div>
