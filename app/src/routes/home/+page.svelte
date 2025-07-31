<script lang="ts">
  import { goto } from '$app/navigation';
  import ProfileExternalLink from '$lib/components/profile/ProfileExternalLink.svelte';
  import ProfileListButton from '$lib/components/profile/ProfileListButton.svelte';
  import { account_active } from '$lib/tauri/account.svelte';
  import {
    profile_favorites_list,
    profile_history_list
  } from '$lib/tauri/home.svelte';
  import { profile_launch } from '$lib/tauri/profile.svelte';
  import { compareProfiles } from '$lib/util.svelte';
  import { Separator } from 'positron-components/components/ui';

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
        <ProfileListButton
          onclick={() =>
            goto(`/profiles/info/quick_play?id=${favorite.profile.id}`)}
          onclickInner={() => {
            profile_launch(
              favorite.profile.id,
              favorite.profile.name,
              active_account,
              favorite.quick_play
            );
          }}
          profile={favorite.profile}
          text={favorite.quick_play &&
            favorite.quick_play.name + ' (' + favorite.profile.name + ')'}
        />
      {/each}
    </div>
  {:else}
    <p class="text-muted-foreground text-center">
      No favorites found. Add a profile or quick play option to your favorites
      in the profile settings.
    </p>
    <ProfileExternalLink />
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
        <ProfileListButton
          onclick={() =>
            goto(`/profiles/info/quick_play?id=${item.profile.id}`)}
          onclickInner={() => {
            profile_launch(
              item.profile.id,
              item.profile.name,
              active_account,
              item.quick_play
            );
          }}
          profile={item.profile}
          text={item.quick_play &&
            item.quick_play.name + ' (' + item.profile.name + ')'}
        />
      {/each}
    </div>
  {:else}
    <p class="text-muted-foreground text-center">
      No entries found in history. Launch a profile to create one.
    </p>
    <ProfileExternalLink />
  {/if}
</div>
