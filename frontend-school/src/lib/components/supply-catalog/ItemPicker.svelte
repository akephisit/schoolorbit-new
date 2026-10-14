<script lang="ts">
	import { onDestroy } from 'svelte';
	import { listItems, type SupplyItem, type SupplyItemPage } from '#lib/api/supply-catalog.js';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { PageState, PageSkeleton } from '#lib/components/app-state/index.js';
	import { message, money } from '#lib/supply-catalog/drafts.js';
	let { onselect, disabled = false }: { onselect: (item: SupplyItem) => void; disabled?: boolean } =
		$props();
	const request = new LatestRequest();
	let search = $state(''),
		loading = $state(false),
		error = $state<string | null>(null);
	let result = $state.raw<SupplyItemPage | null>(null);
	async function load(page = 1) {
		const ticket = request.begin();
		loading = true;
		error = null;
		try {
			const data = await listItems({ search, page, pageSize: 6 }, { signal: ticket.signal });
			if (request.isCurrent(ticket.revision)) result = data;
		} catch (e) {
			if (request.isCurrent(ticket.revision)) error = message(e, 'ค้นหาพัสดุไม่สำเร็จ');
		} finally {
			if (request.isCurrent(ticket.revision)) loading = false;
		}
	}
	onDestroy(() => request.abort());
</script>

<section class="space-y-3 rounded-xl border bg-muted/20 p-3" aria-label="ค้นหาพัสดุเข้าชุด">
	<div class="flex items-end gap-2">
		<div class="min-w-0 flex-1 space-y-1">
			<Label for="item-picker-search">ค้นหาพัสดุในคลัง</Label><Input
				id="item-picker-search"
				bind:value={search}
				placeholder="ชื่อหรือรายละเอียด"
				onkeydown={(event) => {
					if (event.key !== 'Enter') return;
					event.preventDefault();
					event.stopPropagation();
					if (!event.isComposing && !disabled && !loading) void load();
				}}
				{disabled}
			/>
		</div>
		<Button type="button" variant="outline" disabled={disabled || loading} onclick={() => load()}
			>ค้นหา</Button
		>
	</div>
	{#if loading && !result}<PageSkeleton variant="cards" rows={2} />{/if}
	{#if error}<PageState
			variant="error"
			title="ค้นหาไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={() => load()}
		/>{/if}
	{#if result}<div class="space-y-1" aria-busy={loading}>
			{#each result.items as item (item.id)}<Button
					variant="ghost"
					class="h-auto w-full justify-between gap-3 py-2 text-left"
					disabled={disabled || loading}
					onclick={() => onselect(item)}
					><span class="min-w-0 whitespace-normal"
						>{item.name}<span class="block text-xs text-muted-foreground"
							>{item.unit} · {item.latestVersion.shop}</span
						></span
					><span class="shrink-0">{money(item.latestVersion.price)} ฿</span></Button
				>{/each}
			{#if result.items.length === 0}<p class="text-sm text-muted-foreground">
					ไม่พบพัสดุที่ตรงกับคำค้น
				</p>{/if}
			{#if result.total > result.pageSize}<div class="flex items-center justify-between">
					<Button
						size="sm"
						variant="ghost"
						disabled={loading || result.page === 1}
						onclick={() => load(result!.page - 1)}>ก่อนหน้า</Button
					><span class="text-xs text-muted-foreground">หน้า {result.page}</span><Button
						size="sm"
						variant="ghost"
						disabled={loading || result.page * result.pageSize >= result.total}
						onclick={() => load(result!.page + 1)}>ถัดไป</Button
					>
				</div>{/if}
		</div>{/if}
</section>
