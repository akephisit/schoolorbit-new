<script lang="ts">
	import { onDestroy } from 'svelte';
	import { listItems, type SupplyItem, type SupplyItemPage } from '#lib/api/supply-catalog.js';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { message } from '#lib/budget/drafts.js';
	import { toSatang, money } from '#lib/budget/money.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { PageState, TableSkeleton } from '#lib/components/app-state/index.js';
	let { onselect }: { onselect: (item: SupplyItem) => void } = $props();
	let search = $state(''),
		result = $state<SupplyItemPage | null>(null),
		error = $state(''),
		loading = $state(false);
	const request = new LatestRequest();
	async function load(page = 1) {
		const ticket = request.begin();
		loading = true;
		error = '';
		try {
			const value = await listItems({ search, page, pageSize: 8 }, { signal: ticket.signal });
			if (request.isCurrent(ticket.revision)) result = value;
		} catch (cause) {
			if (request.isCurrent(ticket.revision)) error = message(cause);
		} finally {
			if (request.isCurrent(ticket.revision)) loading = false;
		}
	}
	onDestroy(() => request.abort());
</script>

<section aria-label="เลือกพัสดุจากคลัง" class="space-y-4">
	<form
		class="flex flex-wrap items-end gap-3"
		onsubmit={(event) => {
			event.preventDefault();
			void load();
		}}
	>
		<div class="min-w-0 flex-1 space-y-2">
			<Label for="budget-catalog-search">ค้นหาพัสดุ</Label><Input
				id="budget-catalog-search"
				bind:value={search}
			/>
		</div>
		<Button type="submit" variant="outline" disabled={loading}>ค้นหา</Button>
	</form>
	{#if loading && !result}<TableSkeleton columns={3} rows={4} />{/if}
	{#if error}<PageState
			variant="error"
			title="ค้นหาพัสดุไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={() => load()}
		/>{/if}
	{#if result}<ul class="divide-y rounded-lg border" aria-busy={loading}>
			{#each result.items as item (item.id)}<li>
					<Button
						type="button"
						variant="ghost"
						class="h-auto w-full justify-between gap-3 py-3 text-left"
						onclick={() => onselect(item)}
						><span class="min-w-0 whitespace-normal break-words"
							>{item.name}<span class="block text-xs text-muted-foreground"
								>{item.unit} · {item.latestVersion.shop}</span
							></span
						><span class="shrink-0 text-right tabular-nums"
							>{money(toSatang(item.latestVersion.price))} บาท</span
						></Button
					>
				</li>{/each}
		</ul>
		{#if !result.items.length}<p class="text-sm text-muted-foreground">ไม่พบพัสดุตามคำค้น</p>{/if}
		<div class="flex justify-between">
			<Button
				variant="outline"
				size="sm"
				disabled={loading || result.page <= 1}
				onclick={() => load(result!.page - 1)}>ก่อนหน้า</Button
			><span class="text-sm text-muted-foreground">หน้า {result.page}</span><Button
				variant="outline"
				size="sm"
				disabled={loading || result.page * result.pageSize >= result.total}
				onclick={() => load(result!.page + 1)}>ถัดไป</Button
			>
		</div>{/if}
</section>
