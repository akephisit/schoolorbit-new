<script lang="ts">
	import type { Component } from 'svelte';
	import { Card } from '#lib/components/ui/card/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { ArrowRight } from '@lucide/svelte';
	let {
		items
	}: { items: { title: string; description: string; href: string; icon: Component }[] } = $props();
</script>

<section aria-label="เมนูนักเรียน" class="grid gap-4 sm:grid-cols-2">
	{#each items as item (item.href)}
		<Card class="flex min-w-0 flex-col gap-4 p-5">
			<div class="flex items-center gap-3">
				<div class="rounded-xl bg-primary/10 p-2.5 text-primary"><item.icon class="size-5" /></div>
				<h2 class="font-semibold">{item.title}</h2>
			</div>
			<p class="text-sm text-muted-foreground">{item.description}</p>
			<Button
				href={item.href}
				variant="outline"
				class="mt-auto w-full justify-between"
				data-sveltekit-preload-data="tap">ดู{item.title}<ArrowRight class="size-4" /></Button
			>
		</Card>
	{/each}
</section>
