<script lang="ts">
  import { goto } from '$app/navigation';
  import {
    SimpleSidebar,
    Button,
    Dialog
  } from 'positron-components/components';
  import { StopCircle } from '@lucide/svelte';
  import ProfileIcon from '$lib/components/profile/ProfileIcon.svelte';
  import { instance_list, instance_stop } from '$lib/tauri/instance.svelte.js';
  import { setInstance } from './store.svelte.js';

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
  <div class="flex items-center gap-2"></div>
  <div class="mt-2 ml-2 flex">
    <ProfileIcon
      id={instance.id}
      class="size-24 border-2"
      classFallback="size-20"
    />
    <div class="my-4 ml-4 flex flex-col gap-3">
      <p class="text-xl">{instance.profile_name}</p>
      <p class="text-muted-foreground whitespace-nowrap">
        {instance.loader}
        {instance.version}
      </p>
    </div>
    <div class="mr-2 ml-auto flex items-center gap-2">
      <Button variant="destructive" onclick={() => (stopOpen = true)}>
        <StopCircle />
        Stop
      </Button>
    </div>
  </div>
  <div class="mt-2 flex h-full flex-col lg:flex-row">
    <aside class="lg:w-52 lg:max-w-32">
      <SimpleSidebar {items} class="" />
    </aside>
    <div
      class="flex min-h-0 flex-1 space-y-8 p-2 lg:h-full lg:space-y-0 lg:space-x-12"
    >
      {@render children()}
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
