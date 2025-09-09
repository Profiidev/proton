<script lang="ts">
  import { b_to_mb } from '$lib/util.svelte';
  import { Progress } from 'positron-components/components/ui';

  interface Props {
    value: number;
    total: number;
    text: string;
    unit?: string;
  }

  let { value, total, text, unit }: Props = $props();

  let percentage = $derived((value / total) * 100);
</script>

<div class="w-69">
  <p>{text}</p>
  <p class="whitespace-nowrap">
    {percentage.toFixed(1)}% ({b_to_mb(value).toFixed(1)}{unit} / {b_to_mb(
      total
    ).toFixed(1)}{unit})
  </p>
  <Progress min={0} max={100} value={percentage} class="mr-2" />
</div>
