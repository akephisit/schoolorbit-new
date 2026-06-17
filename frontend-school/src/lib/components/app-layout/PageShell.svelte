<script lang="ts">
	import type { Component, Snippet } from 'svelte';
	import { cn } from '$lib/utils';
	import PageHeader from './PageHeader.svelte';

	let {
		title,
		description,
		backHref,
		backLabel = 'ย้อนกลับ',
		icon,
		meta,
		actions,
		children,
		class: className,
		contentClass
	}: {
		title: string;
		description?: string;
		backHref?: string;
		backLabel?: string;
		icon?: Component;
		meta?: Snippet;
		actions?: Snippet;
		children?: Snippet;
		class?: string;
		contentClass?: string;
	} = $props();
</script>

<div class={cn('space-y-6', className)}>
	<PageHeader {title} {description} {backHref} {backLabel} {icon} {meta} {actions} />
	<div class={cn('space-y-6', contentClass)}>
		{@render children?.()}
	</div>
</div>
