<script lang="ts">
  import { cn } from '@profidev/pleiades/utils';
  import { Label } from '@profidev/pleiades/components/ui/label';
  import { Switch } from '@profidev/pleiades/components/ui/switch';
  import * as Tooltip from '@profidev/pleiades/components/ui/tooltip';

  interface Props {
    id: string;
    label: string;
    checked: boolean;
    onCheckedChange: (value: boolean) => void;
    tooltip: string;
    class?: string;
    disabled?: boolean;
  }

  let {
    id,
    label,
    checked = $bindable(false),
    onCheckedChange,
    tooltip,
    class: className,
    disabled
  }: Props = $props();
</script>

<div class={cn('flex items-center', className)}>
  <Tooltip.Provider>
    <Tooltip.Root>
      <Tooltip.Trigger>
        <Label for={id} class={cn(disabled && 'text-muted-foreground')}
          >{label}</Label
        >
      </Tooltip.Trigger>
      <Tooltip.Content side="right">
        <p>
          {tooltip}
        </p>
      </Tooltip.Content>
    </Tooltip.Root>
  </Tooltip.Provider>
  <Switch {id} class="ml-auto" bind:checked {onCheckedChange} {disabled} />
</div>
