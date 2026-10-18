<script lang="ts">
	import { Slider as SliderPrimitive } from 'bits-ui';
	import { cn, type WithoutChildrenOrChild } from '#lib/utils.js';

	let {
		ref = $bindable(null),
		value = $bindable(0),
		class: className,
		thumbProps,
		orientation = 'horizontal',
		...restProps
	}: Omit<
		WithoutChildrenOrChild<Extract<SliderPrimitive.RootProps, { type: 'single' }>>,
		'orientation'
	> & {
		orientation?: 'horizontal';
		thumbProps?: Omit<WithoutChildrenOrChild<SliderPrimitive.ThumbProps>, 'index'>;
	} = $props();
</script>

<SliderPrimitive.Root
	bind:ref
	bind:value
	{orientation}
	data-slot="slider"
	class={cn(
		'relative flex h-8 w-full touch-none items-center select-none data-disabled:opacity-50',
		className
	)}
	{...restProps}
>
	<span
		data-slot="slider-track"
		class="bg-muted relative h-2 w-full grow overflow-hidden rounded-full"
	>
		<SliderPrimitive.Range data-slot="slider-range" class="bg-primary absolute h-full" />
	</span>
	<SliderPrimitive.Thumb
		index={0}
		data-slot="slider-thumb"
		class="border-primary bg-background ring-ring/50 block size-4 shrink-0 rounded-full border-2 shadow-sm outline-none focus-visible:ring-4 data-disabled:pointer-events-none"
		{...thumbProps}
	/>
</SliderPrimitive.Root>
