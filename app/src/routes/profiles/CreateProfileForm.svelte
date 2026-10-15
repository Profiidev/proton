<script lang="ts">
  import { defaults, setError, superForm } from 'sveltekit-superforms';
  import { zod4 } from 'sveltekit-superforms/adapters';
  import { toast } from 'svelte-sonner';
  import * as Dialog from '@profidev/pleiades/components/ui/dialog';
  import * as Form from '@profidev/pleiades/components/ui/form';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import { Switch } from '@profidev/pleiades/components/ui/switch';
  import VersionSelect from '$lib/components/form/VersionSelect.svelte';
  import FormInput from '@profidev/pleiades/components/form/form-input.svelte';
  import { LoaderCircle } from '@lucide/svelte';
  import FormImage from '$lib/components/form/FormImage.svelte';
  import {
    LoaderType,
    profile_create,
    ProfileError
  } from '$lib/tauri/profile.svelte';
  import {
    loader_version_list,
    version_list
  } from '$lib/tauri/versions.svelte';
  import { settings_get, settings_set } from '$lib/tauri/settings.svelte';
  import { file_to_bytes } from '$lib/util.svelte';
  import { profileCreateSchema } from './schema.svelte';

  interface Props {
    onclose: () => void;
  }

  let { onclose }: Props = $props();

  const form = superForm(
    defaults({ loader: LoaderType.Vanilla }, zod4(profileCreateSchema)),
    {
      validators: zod4(profileCreateSchema),
      SPA: true,
      invalidateAll: false,
      onUpdate: async ({ form: f }) => {
        if (!f.valid) return;
        const { icon, ...data } = f.data;
        const res = await profile_create({
          ...data,
          icon: icon ? await file_to_bytes(icon) : undefined
        });
        if (res === ProfileError.InvalidImage) {
          setError(f, 'icon', 'Invalid image');
        } else if (res) {
          toast.error('Failed to create profile');
        } else {
          onclose();
        }
      }
    }
  );
  const { form: formData, enhance, submitting } = form;

  let settings = $derived(settings_get.value);
  let show_snapshots = $derived(settings?.minecraft.show_snapshots ?? false);
  let loader = $derived($formData.loader);
  let version = $derived($formData.version);
  let versions = $state<string[]>([]);
  let loader_versions = $state<string[]>([]);

  // the backend filters snapshots by the global setting, so refetch when it changes
  $effect(() => {
    show_snapshots;
    let stale = false;
    version_list(loader).then((v) => {
      if (stale) return;
      versions = v ?? [];
      if (!versions.includes($formData.version)) {
        $formData.version = versions[0] ?? '';
      }
    });
    return () => (stale = true);
  });

  $effect(() => {
    if (loader === LoaderType.Vanilla || !version) {
      loader_versions = [];
      if ($formData.loader_version !== undefined) {
        $formData.loader_version = undefined;
      }
      return;
    }
    let stale = false;
    loader_version_list(loader, version).then((v) => {
      if (stale) return;
      loader_versions = v ?? [];
      if (!loader_versions.includes($formData.loader_version ?? '')) {
        $formData.loader_version = loader_versions[0];
      }
    });
    return () => (stale = true);
  });

  const setShowSnapshots = async (value: boolean) => {
    if (
      !settings ||
      (await settings_set({
        ...settings,
        minecraft: { ...settings.minecraft, show_snapshots: value }
      }))
    ) {
      toast.error('Failed to save Minecraft settings');
    }
  };
</script>

<form method="POST" use:enhance class="flex min-w-0 flex-col gap-5">
  <div class="flex min-w-0 flex-col gap-4">
    <div class="flex min-w-0 items-end gap-3.5">
      <FormImage
        key="icon"
        formData={form}
        placeholder="Icon"
        class="border-input size-18 shrink-0 rounded-lg border border-dashed ring-0 ring-offset-0"
      />
      <div class="min-w-0 flex-1">
        <FormInput
          label="Name"
          placeholder="Survival SMP"
          key="name"
          formData={form}
        />
      </div>
    </div>
    <Form.Fieldset {form} name="loader" class="flex flex-col gap-1.5 space-y-0">
      <Form.Legend class="text-sm font-medium">Loader</Form.Legend>
      <div class="grid grid-cols-5 gap-1.5" role="radiogroup">
        {#each Object.values(LoaderType) as item (item)}
          <label
            class="border-input bg-input/30 hover:bg-input/60 has-checked:border-primary has-checked:bg-primary/35 has-focus-visible:ring-ring/50 relative flex h-9 items-center justify-center overflow-hidden rounded-lg border px-1 text-[13px] font-medium whitespace-nowrap has-focus-visible:ring-3"
          >
            <input
              type="radio"
              name="loader"
              value={item}
              bind:group={$formData.loader}
              class="absolute inset-0 cursor-pointer opacity-0"
            />
            {item}
          </label>
        {/each}
      </div>
      <Form.FieldErrors />
    </Form.Fieldset>
    <div class="flex flex-wrap gap-3">
      <Form.Field
        {form}
        name="version"
        class="flex min-w-0 flex-[1_1_220px] flex-col gap-1.5"
      >
        <Form.Control>
          {#snippet children()}
            <Form.Label>Minecraft version</Form.Label>
            <VersionSelect
              label="Minecraft version"
              options={versions}
              bind:value={$formData.version}
            />
          {/snippet}
        </Form.Control>
        <Form.FieldErrors />
      </Form.Field>
      <Form.Field
        {form}
        name="loader_version"
        class="flex min-w-0 flex-[1_1_220px] flex-col gap-1.5"
      >
        <Form.Control>
          {#snippet children()}
            <Form.Label>Loader version</Form.Label>
            <VersionSelect
              label="Loader version"
              options={loader_versions}
              disabled={loader_versions.length === 0}
              bind:value={$formData.loader_version}
            />
          {/snippet}
        </Form.Control>
        <Form.FieldErrors />
      </Form.Field>
    </div>
    <label class="flex items-center gap-2 text-[13px]">
      <Switch
        size="sm"
        checked={show_snapshots}
        disabled={!settings}
        onCheckedChange={setShowSnapshots}
      />
      Show snapshots
    </label>
    <p class="text-muted-foreground text-xs">
      Minecraft, Java and the loader download when you first press play.
    </p>
  </div>
  <Dialog.Footer>
    <Button variant="outline" type="button" onclick={onclose}>Cancel</Button>
    <Button type="submit" disabled={$submitting}>
      {#if $submitting}
        <LoaderCircle class="animate-spin" data-icon="inline-start" />
      {/if}
      Create profile
    </Button>
  </Dialog.Footer>
</form>
