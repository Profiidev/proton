<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { Button } from "positron-components/components/ui";

  const download = async () => {
    let res = await invoke("versions_download", {
      id: "1.21.4",
    });
    console.log(res);
  };
</script>

<p>Home</p>
<Button onclick={download}>Download</Button>
