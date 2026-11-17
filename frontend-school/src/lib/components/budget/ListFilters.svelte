<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { goto } from '$app/navigation';
	import { Search, X } from '@lucide/svelte';
	import { getBudgetCatalogs, type BudgetCatalogs } from '#lib/api/budget.js';
	import { budgetFilters, destination, statusLabels } from '#lib/budget/presentation.js';
	import { BudgetRegion } from '#lib/budget/region.svelte.js';
	import type { RouteLoadResult } from '#lib/navigation/route-load.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { PageState } from '#lib/components/app-state/index.js';
	import Choice from './Choice.svelte';

	let {
		source,
		query,
		path,
		searchable = false
	}: {
		source: Promise<RouteLoadResult<BudgetCatalogs | null>>;
		query: ReturnType<typeof budgetFilters>;
		path: '/staff/budget/projects' | '/staff/budget/requests' | '/staff/budget/adjustments';
		searchable?: boolean;
	} = $props();
	const catalogs = new BudgetRegion<BudgetCatalogs | null>();
	let search = $state(untrack(() => query.search));
	const filtered = $derived(!!(query.fiscalYearId || query.status || (searchable && query.search)));
	$effect.pre(() => catalogs.consume(source));
	onDestroy(() => catalogs.dispose());
	function apply(values: Record<string, string>) {
		void goto(destination(path, { ...query, ...values, offset: 0 }));
	}
</script>

<section class="space-y-3 rounded-xl border bg-card p-3 sm:p-4" aria-label="ตัวกรองรายการงบประมาณ">
	<form
		class="flex flex-wrap items-end gap-3"
		onsubmit={(event) => {
			event.preventDefault();
			apply({ search });
		}}
	>
		<div class="w-full sm:w-48">
			<Choice
				id="budget-list-year"
				label="ปีงบประมาณ"
				value={query.fiscalYearId}
				options={[
					{ value: '', label: 'ทุกปีงบประมาณ' },
					...(catalogs.value?.fiscalYears ?? []).map((year) => ({
						value: year.id,
						label: year.name
					}))
				]}
				disabled={catalogs.loading && !catalogs.value}
				onchange={(fiscalYearId) => apply({ fiscalYearId })}
			/>
		</div>
		<div class="w-full sm:w-48">
			<Choice
				id="budget-list-status"
				label="สถานะ"
				value={query.status}
				options={[
					{ value: '', label: 'ทุกสถานะ' },
					...Object.entries(statusLabels).map(([value, label]) => ({ value, label }))
				]}
				onchange={(status) => apply({ status })}
			/>
		</div>
		{#if searchable}
			<div class="min-w-0 flex-1 basis-48 space-y-2">
				<Label for="budget-project-search">ค้นหาโครงการ</Label>
				<Input id="budget-project-search" bind:value={search} placeholder="ชื่อโครงการ" />
			</div>
			<Button type="submit" variant="outline"
				><Search class="size-4" aria-hidden="true" />ค้นหา</Button
			>
		{/if}
		{#if filtered}<Button
				type="button"
				variant="ghost"
				onclick={() => apply({ fiscalYearId: '', status: '', search: '' })}
				><X class="size-4" aria-hidden="true" />ล้างตัวกรอง</Button
			>{/if}
	</form>
	{#if catalogs.error}<PageState
			variant="error"
			title="โหลดปีงบประมาณไม่สำเร็จ"
			description={catalogs.error}
			actionLabel="ลองอีกครั้ง"
			onaction={() => catalogs.retry((signal) => getBudgetCatalogs({ signal }))}
		/>{/if}
</section>
