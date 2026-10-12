<script lang="ts">
	import { ToggleGroup as ToggleGroupPrimitive } from 'bits-ui';
	import { cn, type WithoutChild } from '#lib/utils.js';

	let {
		ref = $bindable(null),
		value = $bindable(''),
		class: className,
		children,
		...restProps
	}: WithoutChild<Extract<ToggleGroupPrimitive.RootProps, { type: 'single' }>> = $props();
</script>

<ToggleGroupPrimitive.Root bind:ref bind:value {...restProps}>
	{#snippet child({ props })}
		<div
			{...props}
			role="radiogroup"
			data-slot="toggle-group"
			class={cn('flex items-center gap-1', className)}
		>
			{@render children?.()}
		</div>
	{/snippet}
</ToggleGroupPrimitive.Root>
