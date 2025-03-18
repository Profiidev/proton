<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { Button } from "positron-components";

  const auth = async () => {
    await invoke("auth");
  };
</script>

<main class="bg-black h-full">
  <Button onclick={auth}>Testing</Button>
</main>
