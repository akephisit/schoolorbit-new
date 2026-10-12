<script lang="ts">
	import { ToggleGroup as ToggleGroupPrimitive } from 'bits-ui';
	import { buttonVariants } from '#lib/components/ui/button/index.js';
	import { cn, type WithoutChild } from '#lib/utils.js';

	let {
		ref = $bindable(null),
		class: className,
		...restProps
	}: WithoutChild<ToggleGroupPrimitive.ItemProps> = $props();
</script>

<ToggleGroupPrimitive.Item
	bind:ref
	data-slot="toggle-group-item"
	class={cn(
		buttonVariants({ variant: 'outline', size: 'sm' }),
		'data-[state=on]:bg-accent data-[state=on]:text-accent-foreground',
		className
	)}
	{...restProps}
/>
