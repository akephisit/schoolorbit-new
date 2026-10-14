<script lang="ts">
	import { onDestroy } from 'svelte';
	import { saveCategory, type SupplyCategory } from '#lib/api/supply-catalog.js';
	import { message, validateSupplyText, trimSupplyText } from '#lib/supply-catalog/drafts.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { LoadingButton } from '#lib/components/app-state/index.js';
	import * as Alert from '#lib/components/ui/alert/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import { AlertCircle } from '@lucide/svelte';
	import FieldError from './FieldError.svelte';
	let {
		categories,
		onpatch
	}: { categories: SupplyCategory[]; onpatch: (category: SupplyCategory) => void } = $props();
	let selected = $state.raw<SupplyCategory | null>(null),
		name = $state(''),
		pending = $state(false),
		error = $state<string | null>(null);
	let disposed = false;
	let submitted = $state(false);
	const nameError = $derived(submitted ? validateSupplyText(name, 'ชื่อหมวดหมู่', 120) : null);
	function reset() {
		selected = null;
		name = '';
		submitted = false;
		error = null;
	}
	export async function discard() {
		return !pending;
	}
	onDestroy(() => (disposed = true));
	async function save() {
		if (pending || disposed) return;
		submitted = true;
		if (nameError) return;
		pending = true;
		error = null;
		try {
			const saved = await saveCategory(selected?.id ?? null, {
				name: trimSupplyText(name),
				revision: selected?.revision ?? null
			});
			if (!disposed) {
				onpatch(saved);
				reset();
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
		class="space-y-4"
		novalidate
		onsubmit={(e) => {
			e.preventDefault();
			void save();
		}}
	>
		<div class="min-w-0 flex-1 space-y-1">
			<Label for="category-name">ชื่อหมวดหมู่ *</Label><Input
				id="category-name"
				bind:value={name}
				required
				disabled={pending}
				aria-invalid={!!nameError}
				aria-describedby={nameError ? 'category-name-error' : undefined}
			/><FieldError id="category-name-error" message={nameError} />
		</div>
		{#if error}<Alert.Root variant="destructive"
				><AlertCircle aria-hidden="true" /><Alert.Title>บันทึกหมวดหมู่ไม่สำเร็จ</Alert.Title
				><Alert.Description>{error}</Alert.Description></Alert.Root
			>{/if}
		<div class="flex flex-wrap justify-end gap-2">
			<Button type="button" variant="outline" disabled={pending} onclick={reset}>ยกเลิก</Button
			><LoadingButton type="submit" loading={pending}
				>{selected ? 'บันทึกการแก้ไข' : 'เพิ่มหมวดหมู่'}</LoadingButton
			>
		</div>
	</form>
	<Card.Root class="gap-0 py-0"
		><Card.Content class="space-y-3 p-4">
			<h3 class="font-semibold">หมวดหมู่ในคลัง</h3>
			<ul class="divide-y rounded-xl border">
				{#each categories as category (category.id)}<li class="flex items-center gap-2 p-3">
						<span class="min-w-0 flex-1 break-words text-sm">{category.name}</span><Button
							type="button"
							size="sm"
							variant="ghost"
							disabled={pending}
							onclick={() => {
								selected = category;
								name = category.name;
								submitted = false;
								error = null;
							}}>แก้ไข</Button
						>
					</li>{/each}
			</ul>
		</Card.Content></Card.Root
	>
</div>
