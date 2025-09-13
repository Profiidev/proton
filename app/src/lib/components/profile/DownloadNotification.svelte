<script lang="ts">
  import { Progress } from 'positron-components/components/ui';
  import { untrack } from 'svelte';

  interface Props {
    value: number;
    total: number;
    text: string;
    unit?: string;
    change?: boolean;
    convert?: (value: number) => number;
    round?: (value: number) => string;
  }

  let {
    value,
    total,
    text,
    unit,
    change,
    convert = (value) => value,
    round = (value) => String(value)
  }: Props = $props();

  let percentage = $derived((value / total) * 100);

  let last_diffs = $state<number[]>([]);
  let last_updates = $state<number[]>([]);
  let change_value = $derived(
    last_diffs.reduce((a, b) => a + b, 0) /
      (last_updates.length > 1
        ? (last_updates[last_updates.length - 1] - last_updates[0]) / 1000
        : 1)
  );
  let last_value = 0;

  $effect(() => {
    value;
    if (change) {
      untrack(() => {
        let diff = value - last_value;
        last_value = value;
        last_diffs.push(convert(diff));
        last_updates.push(Date.now());
        if (last_diffs.length > 5) {
          last_diffs.shift();
          last_updates.shift();
        }
      });
    }
  });
</script>

<div class="mr-1 w-68">
  <div class="flex flex-wrap">
    <p class="flex-none whitespace-nowrap">
      {text}
    </p>
  </div>
  <div class="flex flex-wrap">
    <p class="flex-none whitespace-nowrap">
      {round(convert(value))}{unit} / {round(convert(total))}{unit}
    </p>
    <p class="ml-auto flex-none whitespace-nowrap">
      {#if change}
        {change_value.toFixed(1)}{unit}/s
      {/if}
    </p>
  </div>
  <div class="flex items-center">
    <Progress min={0} max={100} value={percentage} class="mt-1" />
    <p class="ml-2 flex-none whitespace-nowrap">
      {percentage.toFixed(1)}%
    </p>
  </div>
</div>
