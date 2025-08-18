<script lang="ts">
  import { Label, Switch, Tooltip } from 'positron-components/components/ui';

  interface Props {
    label: string;
    checked: boolean;
    onCheckedChange: (value: boolean) => void;
    tooltip: string;
  }

  let {
    label,
    checked = $bindable(false),
    onCheckedChange,
    tooltip
  }: Props = $props();
</script>

<div class="flex items-center">
  <Tooltip.Provider>
    <Tooltip.Root>
      <Tooltip.Trigger>
        <Label for="instant-redirect">{label}</Label>
      </Tooltip.Trigger>
      <Tooltip.Content side="right">
        <p>
          {tooltip}
        </p>
      </Tooltip.Content>
    </Tooltip.Root>
  </Tooltip.Provider>
  <Switch
    id="instant-redirect"
    class="ml-auto"
    bind:checked
    {onCheckedChange}
  />
</div>
