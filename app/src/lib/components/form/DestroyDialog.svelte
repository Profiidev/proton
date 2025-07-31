<script lang="ts">
  import { Button, Dialog } from 'positron-components/components/ui';

  interface Props {
    open: boolean;
    title: string;
    description?: string;
    onclick?: () => void;
    btnText: string;
  }

  let {
    open = $bindable(),
    title,
    description,
    onclick,
    btnText
  }: Props = $props();
</script>

<Dialog.Root bind:open>
  <Dialog.Content>
    <Dialog.Header>
      <Dialog.Title>{title}</Dialog.Title>
      <Dialog.Description>
        {description}
      </Dialog.Description>
    </Dialog.Header>
    <Dialog.Footer>
      <Button
        type="submit"
        variant="destructive"
        class="cursor-pointer"
        {onclick}
      >
        {btnText}
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
