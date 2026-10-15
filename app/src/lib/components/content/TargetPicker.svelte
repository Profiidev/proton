<script lang="ts">
  import { ChevronDown } from '@lucide/svelte';
  import { buttonVariants } from '@profidev/pleiades/components/ui/button';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import * as Popover from '@profidev/pleiades/components/ui/popover';
  import { Input } from '@profidev/pleiades/components/ui/input';
  import ProfileIcon from '$lib/components/profile/ProfileIcon.svelte';
  import { content_target } from '$lib/content-target.svelte';
  import type { Profile } from '$lib/tauri/profile.svelte';

  interface Props {
    target?: Profile;
    profiles: Profile[];
  }

  let { target, profiles }: Props = $props();
  let open = $state(false);
  let query = $state('');
  let shown = $derived(
    profiles.filter((p) =>
      p.name.toLowerCase().includes(query.trim().toLowerCase())
    )
  );
</script>

<Popover.Root bind:open>
  <Popover.Trigger
    class={buttonVariants({ variant: 'outline', class: 'h-10 max-w-full' })}
    disabled={profiles.length === 0}
  >
    {#if target}
      <ProfileIcon
        id={target.id}
        class="size-6 rounded-sm"
        classFallback="size-4"
      />
      <span class="text-muted-foreground">Installing to</span>
      <span class="truncate">{target.name}</span>
      <span class="text-muted-foreground">
        {target.loader}
        {target.version}
      </span>
    {:else}
      <span class="text-muted-foreground">No profile to install to</span>
    {/if}
    <ChevronDown data-icon="inline-end" />
  </Popover.Trigger>
  <Popover.Content align="end" class="flex w-72 flex-col gap-1 p-1">
    <Input placeholder="Search profiles" bind:value={query} />
    <div class="flex max-h-80 flex-col overflow-y-auto">
      {#each shown as profile (profile.id)}
        <Button
          variant={profile.id === target?.id ? 'secondary' : 'ghost'}
          class="h-auto justify-start gap-2.5 p-2"
          onclick={() => {
            content_target.id = profile.id;
            open = false;
          }}
        >
          <ProfileIcon id={profile.id} class="size-8" classFallback="size-5" />
          <span class="flex min-w-0 flex-col text-start">
            <span class="truncate text-sm">{profile.name}</span>
            <span class="text-muted-foreground text-xs">
              {profile.loader}
              {profile.version}
            </span>
          </span>
        </Button>
      {:else}
        <p class="text-muted-foreground p-2 text-sm">No matching profiles.</p>
      {/each}
    </div>
  </Popover.Content>
</Popover.Root>
