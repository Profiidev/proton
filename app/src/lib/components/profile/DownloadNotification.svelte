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
  <p>{text}</p>
  <p class="whitespace-nowrap">
    {percentage.toFixed(1)}% ({convert(value)} / {convert(total)})
  </p>
  <Progress min={0} max={100} value={percentage} class="mr-2" />
</div>
