<script lang="ts" generics="T, S extends FormRecord = FormRecord">
  import {
    type FormRecord,
    type SuperForm
  } from 'positron-components/components/form/types';
  import FormSelect from 'positron-components/components/form/form-select.svelte';
  import type { ComponentProps } from 'svelte';
  import { get } from 'svelte/store';

  interface Props {
    formData: SuperForm<S>;
    //typescript is dump and thinks the type could be infinite if we use FormPath<S>
    key: any;
    val: any;
  }
  type FormSelectProps = Omit<
    ComponentProps<FormSelect<T, S>>,
    'formData' | 'key'
  >;

  let {
    val = $bindable(),
    formData,
    ...props
  }: Props & FormSelectProps = $props();

  formData.form.subscribe((form) => {
    val = form[props.key];
  });

  $effect(() => {
    let from = get(formData.form);
    let current = from[props.key];

    if (
      Array.isArray(current) &&
      Array.isArray(val) &&
      current.length === val.length &&
      current.every((v, i) => v === val[i])
    ) {
      return;
    }

    formData.form.set({
      ...from,
      [props.key]: val
    });
  });
</script>

<FormSelect {formData} {...props} />
