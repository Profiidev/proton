const create_state = <T>(base: T) => {
  let state: T = $state(base);

  return {
    get value() {
      return state;
    },

    set value(v: T) {
      state = v;
    },
  };
};

export const settings_open = create_state(false);
