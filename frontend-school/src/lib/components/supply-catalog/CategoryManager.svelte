<script lang="ts">
	import { onDestroy } from 'svelte';
	import { saveCategory, type SupplyCategory } from '#lib/api/supply-catalog.js';
	import { message } from '#lib/supply-catalog/drafts.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { LoadingButton } from '#lib/components/app-state/index.js';
	let {
		categories,
		onpatch
	}: { categories: SupplyCategory[]; onpatch: (category: SupplyCategory) => void } = $props();
	let selected = $state.raw<SupplyCategory | null>(null),
		name = $state(''),
		pending = $state(false),
		error = $state<string | null>(null);
	let disposed = false;
	export async function discard() {
		return !pending;
	}
	onDestroy(() => (disposed = true));
	async function save() {
		if (pending || !name.trim()) return;
		pending = true;
		error = null;
		try {
			const saved = await saveCategory(selected?.id ?? null, {
				name: name.trim(),
				revision: selected?.revision ?? null
			});
			if (!disposed) {
				onpatch(saved);
				selected = null;
				name = '';
			}
		} catch (e) {
			if (!disposed) error = message(e, 'บันทึกหมวดหมู่ไม่สำเร็จ');
		} finally {
			if (!disposed) pending = false;
		}
	}
</script>

<div class="space-y-4">
	<form
		class="flex items-end gap-2"
		onsubmit={(e) => {
			e.preventDefault();
			void save();
		}}
	>
		<div class="min-w-0 flex-1 space-y-1">
			<Label for="category-name">{selected ? 'แก้ไขหมวดหมู่' : 'หมวดหมู่ใหม่'}</Label><Input
				id="category-name"
				bind:value={name}
				maxlength={120}
				required
				disabled={pending}
			/>
		</div>
		{#if selected}<Button
				variant="outline"
				disabled={pending}
				onclick={() => {
					selected = null;
					name = '';
				}}>ยกเลิก</Button
			>{/if}<LoadingButton type="submit" loading={pending}>บันทึก</LoadingButton>
	</form>
	{#if error}<p role="alert" class="text-sm text-destructive">{error}</p>{/if}
	<ul class="divide-y rounded-xl border">
		{#each categories as category (category.id)}<li class="flex items-center gap-2 p-3">
				<span class="min-w-0 flex-1 break-words text-sm">{category.name}</span><Button
					size="sm"
					variant="ghost"
					disabled={pending}
					onclick={() => {
						selected = category;
						name = category.name;
					}}>แก้ไข</Button
				>
			</li>{/each}
	</ul>
</div>
