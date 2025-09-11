<script lang="ts">
  import { Progress } from 'positron-components/components/ui';

  interface Props {
    value: number;
    total: number;
    text: string;
    convert?: (value: number) => string;
  }

  let {
    value,
    total,
    text,
    convert = (value) => String(value)
  }: Props = $props();

  let percentage = $derived((value / total) * 100);
</script>

<div class="w-69">
  <div class="flex flex-wrap">
    <p class="flex-none whitespace-nowrap">
      {text}
    </p>
    <p class="ml-auto flex-none whitespace-nowrap">
      {percentage.toFixed(1)}% ({convert(value)} / {convert(total)})
    </p>
  </div>
  <Progress min={0} max={100} value={percentage} class="mt-1" />
</div>
