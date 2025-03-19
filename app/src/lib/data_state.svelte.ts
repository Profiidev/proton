import { tick } from "svelte";

export const create_data_state = <T>(
  update: () => Promise<T | undefined>,
) => {
  let value: T | undefined = $state();

  let subscribers = 0;

  return {
    get value() {
      if ($effect.tracking()) {
        $effect(() => {
          if (subscribers === 0) {
            //init
            update().then((v) => (value = v));
          }

          subscribers++;

          return () => {
            tick().then(() => {
              subscribers--;
              if (subscribers === 0) {
                //cleanup
              }
            });
          };
        });
      }

      return value;
    },
    update: async () => {
      update().then((v) => (value = v));
    },
  };
};