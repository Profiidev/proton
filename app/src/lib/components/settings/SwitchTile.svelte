<script lang="ts">
  import { Switch } from '@profidev/pleiades/components/ui/switch';

  interface Props {
    title: string;
    description: string;
    checked: boolean;
    disabled?: boolean;
    onCheckedChange: (checked: boolean) => void;
  }

  let { title, description, checked, disabled, onCheckedChange }: Props =
    $props();
</script>

<label
  class="border-input bg-input/30 hover:bg-input/60 flex items-center gap-3 rounded-lg border px-3.5 py-3"
>
  <Switch {checked} {disabled} {onCheckedChange} />
  <span class="flex flex-1 flex-col">
    <span class="font-medium">{title}</span>
    <span class="text-muted-foreground text-xs">{description}</span>
  </span>
</label>
