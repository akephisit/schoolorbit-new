<script lang="ts">
	import { Dialog as SheetPrimitive } from 'bits-ui';
	import SheetPortal from './sheet-portal.svelte';
	import XIcon from '@lucide/svelte/icons/x';
	import type { ComponentProps, Snippet } from 'svelte';
	import * as Sheet from './index.js';
	import { cn, type WithoutChildrenOrChild } from '$lib/utils.js';

	type SheetSide = 'top' | 'right' | 'bottom' | 'left';

	const sideClasses = {
		top: 'inset-x-0 top-0 h-auto border-b data-[state=closed]:slide-out-to-top data-[state=open]:slide-in-from-top',
		right:
			'inset-y-0 end-0 h-full w-3/4 border-s data-[state=closed]:slide-out-to-right data-[state=open]:slide-in-from-right sm:max-w-sm',
		bottom:
			'inset-x-0 bottom-0 h-auto border-t data-[state=closed]:slide-out-to-bottom data-[state=open]:slide-in-from-bottom',
		left: 'inset-y-0 start-0 h-full w-3/4 border-e data-[state=closed]:slide-out-to-left data-[state=open]:slide-in-from-left sm:max-w-sm'
	} satisfies Record<SheetSide, string>;

	let {
		ref = $bindable(null),
		class: className,
		side = 'right',
		portalProps,
		children,
		showCloseButton = true,
		...restProps
	}: WithoutChildrenOrChild<SheetPrimitive.ContentProps> & {
		side?: SheetSide;
		portalProps?: WithoutChildrenOrChild<ComponentProps<typeof SheetPortal>>;
		children: Snippet;
		showCloseButton?: boolean;
	} = $props();
</script>

<SheetPortal {...portalProps}>
	<Sheet.Overlay />
	<SheetPrimitive.Content
		bind:ref
		data-slot="sheet-content"
		data-side={side}
		class={cn(
			'bg-background data-[state=open]:animate-in data-[state=closed]:animate-out fixed z-50 flex flex-col gap-4 p-6 shadow-lg transition ease-in-out data-[state=closed]:duration-300 data-[state=open]:duration-500',
			sideClasses[side],
			className
		)}
		{...restProps}
	>
		{@render children?.()}
		{#if showCloseButton}
			<SheetPrimitive.Close
				data-slot="sheet-close"
				class="ring-offset-background focus:ring-ring absolute end-4 top-4 rounded-xs opacity-70 transition-opacity hover:opacity-100 focus:ring-2 focus:ring-offset-2 focus:outline-hidden disabled:pointer-events-none [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4"
			>
				<XIcon />
				<span class="sr-only">Close</span>
			</SheetPrimitive.Close>
		{/if}
	</SheetPrimitive.Content>
</SheetPortal>
