<script lang="ts">
	import { onDestroy } from 'svelte';
	import { page } from '$app/state';
	import { listBudgetRequests, type BudgetRequestPage } from '#lib/api/budget.js';
	import type { RouteLoadResult } from '#lib/navigation/route-load.js';
	import { BudgetRegion } from '#lib/budget/region.svelte.js';
	import { budgetFilters, destination, statusLabels } from '#lib/budget/presentation.js';
	import { money } from '#lib/budget/money.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	import { PageState, TableSkeleton } from '#lib/components/app-state/index.js';
	import { goto } from '$app/navigation';
	import StatusBadge from './StatusBadge.svelte';
	import Choice from './Choice.svelte';
	import Paging from './Paging.svelte';
	let {
		source,
		query,
		embedded = false
	}: {
		source: Promise<RouteLoadResult<BudgetRequestPage | null>>;
		query: ReturnType<typeof budgetFilters>;
		embedded?: boolean;
	} = $props();
	const region = new BudgetRegion<BudgetRequestPage | null>();
	$effect.pre(() => region.consume(source));
	onDestroy(() => region.dispose());
</script>

<section
	class="space-y-3"
	aria-label="คำขอใช้เงิน"
	aria-busy={region.loading}
	data-testid="budget-requests"
>
	{#if embedded}<h2 class="font-semibold">คำขอใช้เงิน</h2>{:else}<div class="w-full sm:w-60">
			<Choice
				id="budget-request-status"
				label="สถานะ"
				value={query.status}
				options={[
					{ value: '', label: 'ทั้งหมด' },
					...Object.entries(statusLabels).map(([value, label]) => ({ value, label }))
				]}
				onchange={(status) =>
					goto(destination('/staff/budget/requests', { ...query, status, offset: 0 }))}
			/>
		</div>{/if}
	{#if region.error}<PageState
			variant="error"
			title="โหลดคำขอใช้เงินไม่สำเร็จ"
			description={region.error}
			actionLabel="ลองอีกครั้ง"
			onaction={() => region.retry((signal) => listBudgetRequests({ signal, query }))}
		/>{/if}
	{#if region.loading && !region.value}<TableSkeleton
			columns={5}
		/>{:else if region.value?.items.length}<div class="overflow-x-auto rounded-lg border bg-card">
			<Table.Root
				><Table.Header
					><Table.Row
						><Table.Head>คำขอ</Table.Head><Table.Head>จำนวนเงิน</Table.Head><Table.Head
							>จ่ายแล้ว</Table.Head
						><Table.Head>สถานะ</Table.Head><Table.Head>รายละเอียด</Table.Head></Table.Row
					></Table.Header
				><Table.Body
					>{#each region.value.items as item (item.id)}<Table.Row
							><Table.Cell class="min-w-48">{item.title}</Table.Cell><Table.Cell
								class="whitespace-nowrap tabular-nums">{money(item.totalSatang)}</Table.Cell
							><Table.Cell class="whitespace-nowrap tabular-nums"
								>{money(item.paidSatang)}</Table.Cell
							><Table.Cell><StatusBadge status={item.status} /></Table.Cell><Table.Cell
								><Button
									variant="outline"
									size="sm"
									href={`/staff/budget/requests/${item.id}?returnTo=${encodeURIComponent(page.url.pathname + page.url.search)}`}
									>เปิดคำขอ</Button
								></Table.Cell
							></Table.Row
						>{/each}</Table.Body
				></Table.Root
			>
		</div>{:else if !region.error}<PageState
			title="ยังไม่มีคำขอใช้เงิน"
			description="สร้างคำขอจากกิจกรรมในโครงการที่อนุมัติแผนแล้ว"
		/>{/if}
	{#if region.value}<Paging
			path={page.url.pathname}
			{query}
			total={region.value.total}
			limit={region.value.limit}
			offset={region.value.offset}
		/>{/if}
</section>
