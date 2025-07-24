<script lang="ts">
  import { goto } from '$app/navigation';
  import {
    SimpleSidebar,
    Button,
    Dialog
  } from 'positron-components/components';
  import { ExternalLink, StopCircle } from '@lucide/svelte';
  import ProfileIcon from '$lib/components/profile/ProfileIcon.svelte';
  import { instance_list, instance_stop } from '$lib/tauri/instance.svelte.js';
  import { setInstance } from './store.svelte.js';
  import { DateTime } from 'positron-components/util';

  let { data, children } = $props();

  let instances = $derived(instance_list.value);
  let instance = $derived(instances?.find((i) => i.id === data.id));
  let stopOpen = $state(false);

  $effect(() => {
    if (!instance) {
      goto('/instances');
    } else {
      setInstance(instance);
    }
  });

  let items = $derived([
    {
      href: `/instances/info/logs?id=${instance?.id}`,
      title: 'Logs'
    }
  ]);
</script>

{#if instance}
  <div class="flex h-full flex-col">
    <div class="mt-2 ml-2 flex">
      <ProfileIcon
        id={instance.id}
        class="size-24 border-2"
        classFallback="size-20"
      />
      <div class="my-2 ml-4 flex flex-col gap-1">
        <p class="text-xl whitespace-nowrap">
          Profile:
          <Button
            variant="outline"
            onclick={() =>
              goto(`/profiles/info/quick_play?id=${instance.profile_id}`)}
            class="inline-flex cursor-pointer text-xl"
          >
            {instance.profile_name}
            <ExternalLink />
          </Button>
        </p>
        <p class="text-muted-foreground whitespace-nowrap">
          {instance.loader}
          {instance.version}
        </p>
        <p class="text-muted-foreground whitespace-nowrap">
          Launched at: {DateTime.fromISO(instance.launched_at)
            .setLocale('de')
            .toLocaleString(DateTime.DATETIME_SHORT)}
        </p>
      </div>
      <div class="mr-2 ml-auto flex items-center gap-2">
        <Button
          variant="destructive"
          onclick={() => (stopOpen = true)}
          class="cursor-pointer"
        >
          <StopCircle />
          Stop
        </Button>
      </div>
    </div>
    <div class="mt-2 flex min-h-0 flex-grow-1 flex-col lg:flex-row gap-2">
      <aside class="lg:w-52 lg:max-w-32 lg:min-w-32">
        <SimpleSidebar {items} class="" />
      </aside>
      <div
        class="flex min-h-0 flex-grow-1 space-y-8 lg:h-full lg:space-y-0 lg:space-x-12"
      >
        {@render children()}
      </div>
    </div>
  </div>
  <Dialog.Root bind:open={stopOpen}>
    <Dialog.Content>
      <Dialog.Header>
        <Dialog.Title>Stop Instance</Dialog.Title>
        <Dialog.Description>
          Are you sure you want to stop the instance of profile "{instance?.profile_name}"?
        </Dialog.Description>
      </Dialog.Header>
      <Dialog.Footer>
        <Button
          type="submit"
          variant="destructive"
          class="cursor-pointer"
          onclick={() => instance_stop(instance.profile_id, instance.id)}
        >
          Stop
        </Button>
      </Dialog.Footer>
    </Dialog.Content>
  </Dialog.Root>
{:else}
  <p class="mt-2 ml-2">Loading...</p>
{/if}
