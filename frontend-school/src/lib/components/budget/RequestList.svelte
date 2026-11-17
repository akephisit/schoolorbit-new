<script lang="ts">
	import * as Card from '#lib/components/ui/card/index.js';
	import ReadAccessNotice from './ReadAccessNotice.svelte';
	import { onDestroy } from 'svelte';
	import { page } from '$app/state';
	import {
		listBudgetRequests,
		type BudgetRequestPage,
		type BudgetCatalogs
	} from '#lib/api/budget.js';
	import type { RouteLoadResult } from '#lib/navigation/route-load.js';
	import { BudgetRegion } from '#lib/budget/region.svelte.js';
	import { budgetFilters } from '#lib/budget/presentation.js';
	import { money } from '#lib/budget/money.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	import { PageState, TableSkeleton } from '#lib/components/app-state/index.js';
	import StatusBadge from './StatusBadge.svelte';
	import ListFilters from './ListFilters.svelte';
	import BudgetNavigation from './BudgetNavigation.svelte';
	import { budgetReadPermissions } from '#lib/budget/access.js';
	import { can } from '#lib/stores/permissions.js';
	import Paging from './Paging.svelte';
	let {
		source,
		catalogsSource,
		query,
		embedded = false
	}: {
		source: Promise<RouteLoadResult<BudgetRequestPage | null>>;
		catalogsSource?: Promise<RouteLoadResult<BudgetCatalogs | null>>;
		query: ReturnType<typeof budgetFilters>;
		embedded?: boolean;
	} = $props();
	const region = new BudgetRegion<BudgetRequestPage | null>();
	$effect.pre(() => region.consume(source));
	onDestroy(() => region.dispose());
</script>

{#if !embedded}<BudgetNavigation fiscalYearId={query.fiscalYearId} /><ReadAccessNotice />{/if}
<section
	class="space-y-4"
	aria-label="คำขอใช้เงิน"
	aria-busy={region.loading}
	data-testid="budget-requests"
>
	{#if !embedded && catalogsSource && $can.hasAny(...budgetReadPermissions)}<ListFilters
			source={catalogsSource}
			{query}
			path="/staff/budget/requests"
		/>{/if}
	<Card.Root
		><Card.Header
			><Card.Title>คำขอใช้เงิน</Card.Title><Card.Description
				>{region.value
					? `ทั้งหมด ${region.value.total} คำขอ`
					: 'รายการใช้เงินจากกิจกรรมที่อนุมัติแผนแล้ว'}</Card.Description
			></Card.Header
		><Card.Content class="space-y-4">
			{#if region.error}<PageState
					variant="error"
					title="โหลดคำขอใช้เงินไม่สำเร็จ"
					description={region.error}
					actionLabel="ลองอีกครั้ง"
					onaction={() => region.retry((signal) => listBudgetRequests({ signal, query }))}
				/>{/if}
			{#if region.loading && !region.value}<TableSkeleton
					columns={5}
				/>{:else if region.value?.items.length}<div
					class="min-w-0 overflow-hidden rounded-lg border"
				>
					<Table.Root class="block w-full sm:table sm:min-w-[40rem]"
						><Table.Header class="sr-only sm:not-sr-only"
							><Table.Row
								><Table.Head>คำขอ</Table.Head><Table.Head class="text-right"
									>จำนวนเงิน (บาท)</Table.Head
								><Table.Head class="text-right">จ่ายแล้ว (บาท)</Table.Head><Table.Head
									>สถานะ</Table.Head
								><Table.Head>รายละเอียด</Table.Head></Table.Row
							></Table.Header
						><Table.Body class="block sm:table-row-group"
							>{#each region.value.items as item (item.id)}<Table.Row
									class="grid grid-cols-2 gap-x-4 gap-y-2 p-4 sm:table-row sm:p-0"
									><Table.Cell class="col-span-2 min-w-0 font-medium">{item.title}</Table.Cell
									><Table.Cell
										class="col-span-2 flex justify-between gap-3 whitespace-nowrap tabular-nums sm:table-cell sm:text-right"
										><span class="text-sm text-muted-foreground sm:hidden">จำนวนเงิน (บาท)</span
										>{money(item.totalSatang)}</Table.Cell
									><Table.Cell
										class="col-span-2 flex justify-between gap-3 whitespace-nowrap tabular-nums sm:table-cell sm:text-right"
										><span class="text-sm text-muted-foreground sm:hidden">จ่ายแล้ว (บาท)</span
										>{money(item.paidSatang)}</Table.Cell
									><Table.Cell class="self-center"><StatusBadge status={item.status} /></Table.Cell
									><Table.Cell class="text-right"
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
				</div>{:else if region.value && !region.error}<PageState
					title={query.fiscalYearId || query.status ? 'ไม่พบคำขอตามตัวกรอง' : 'ยังไม่มีคำขอใช้เงิน'}
					description={query.fiscalYearId || query.status
						? 'ลองเปลี่ยนปีงบประมาณหรือสถานะเพื่อดูรายการอื่น'
						: 'สร้างคำขอจากกิจกรรมในโครงการที่อนุมัติแผนแล้ว'}
				/>{/if}
			{#if region.value}<Paging
					path={page.url.pathname}
					{query}
					total={region.value.total}
					limit={region.value.limit}
					offset={region.value.offset}
				/>{/if}
		</Card.Content></Card.Root
	>
</section>
