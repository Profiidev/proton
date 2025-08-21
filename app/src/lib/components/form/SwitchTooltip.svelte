<script lang="ts">
  import { cn } from 'positron-components/utils';
  import { Label, Switch, Tooltip } from 'positron-components/components/ui';

  interface Props {
    id: string;
    label: string;
    checked: boolean;
    onCheckedChange: (value: boolean) => void;
    tooltip: string;
    class?: string;
  }

  let {
    id,
    label,
    checked = $bindable(false),
    onCheckedChange,
    tooltip,
    class: className
  }: Props = $props();
</script>

<div class={cn('flex items-center', className)}>
  <Tooltip.Provider>
    <Tooltip.Root>
      <Tooltip.Trigger>
        <Label for={id}>{label}</Label>
      </Tooltip.Trigger>
      <Tooltip.Content side="right">
        <p>
          {tooltip}
        </p>
      </Tooltip.Content>
    </Tooltip.Root>
  </Tooltip.Provider>
  <Switch {id} class="ml-auto" bind:checked {onCheckedChange} />
</div>
