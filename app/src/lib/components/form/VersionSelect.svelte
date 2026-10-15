<script lang="ts">
  import Multiselect from '@profidev/pleiades/components/table/multiselect.svelte';

  interface Props {
    label: string;
    options: string[];
    value?: string;
    disabled?: boolean;
    onchange?: (value: string) => void;
  }

  let {
    label,
    options,
    value = $bindable(),
    disabled,
    onchange
  }: Props = $props();

  let data = $derived(options.map((v) => ({ label: v, value: v })));
</script>

<!-- searchable single select, a required value can not be cleared -->
<Multiselect
  {data}
  {label}
  {disabled}
  single
  buttonPrefix="Select"
  class="w-full"
  bind:selected={
    () => (value ? [value] : []),
    (selected) => {
      if (selected[0] !== undefined && selected[0] !== value) {
        value = selected[0];
        onchange?.(selected[0]);
      }
    }
  }
/>
