<script lang="ts">
	import ReadAccessNotice from './ReadAccessNotice.svelte';
	import { onDestroy } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { listBudgetAdjustments, type BudgetAdjustmentPage } from '#lib/api/budget.js';
	import type { RouteLoadResult } from '#lib/navigation/route-load.js';
	import { BudgetRegion } from '#lib/budget/region.svelte.js';
	import { adjustmentLabels, budgetFilters } from '#lib/budget/presentation.js';
	import { money } from '#lib/budget/money.js';
	import { budgetReadPermissions } from '#lib/budget/access.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	import { PageState, TableSkeleton } from '#lib/components/app-state/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import { ArrowLeftRight, Plus } from '@lucide/svelte';
	import Paging from './Paging.svelte';
	import StatusBadge from './StatusBadge.svelte';
	import AdjustmentForm from './AdjustmentForm.svelte';
	let {
		source,
		query
	}: {
		source: Promise<RouteLoadResult<BudgetAdjustmentPage | null>>;
		query: ReturnType<typeof budgetFilters>;
	} = $props();
	const region = new BudgetRegion<BudgetAdjustmentPage | null>();
	let creating = $state(false);
	$effect.pre(() => region.consume(source));
	onDestroy(() => region.dispose());
</script>

<PageShell
	title="ปรับงบประมาณและเงินยืม"
	description="ติดตามการเพิ่ม ลด โอนงบประมาณ และการยืมคืนเงิน"
	icon={ArrowLeftRight}
	contentClass="space-y-6"
	>{#snippet actions()}{#if $can.has(PERMISSIONS.BUDGET_ADJUSTMENT_CREATE_SCHOOL) && $can.hasAny(...budgetReadPermissions)}<Button
				onclick={() => (creating = true)}
				><Plus class="size-4" aria-hidden="true" />สร้างรายการปรับงบ</Button
			>{/if}{/snippet}<ReadAccessNotice />
	<section
		aria-label="รายการปรับงบประมาณ"
		aria-busy={region.loading}
		data-testid="budget-adjustments"
	>
		<Card.Root
			><Card.Header
				><Card.Title>รายการปรับงบประมาณ</Card.Title><Card.Description
					>{region.value
						? `ทั้งหมด ${region.value.total} รายการ`
						: 'รายการและสถานะการพิจารณา'}</Card.Description
				></Card.Header
			><Card.Content>
				{#if region.error}<PageState
						variant="error"
						title="โหลดรายการปรับงบไม่สำเร็จ"
						description={region.error}
						actionLabel="ลองอีกครั้ง"
						onaction={() => region.retry((signal) => listBudgetAdjustments({ signal, query }))}
					/>{/if}{#if region.loading && !region.value}<TableSkeleton
						columns={5}
					/>{:else if region.value?.items.length}<div
						class="min-w-0 overflow-hidden rounded-lg border"
					>
						<Table.Root class="min-w-[44rem]"
							><Table.Header
								><Table.Row
									><Table.Head>รายการ / เหตุผล</Table.Head><Table.Head class="text-right"
										>จำนวนเงิน (บาท)</Table.Head
									><Table.Head class="text-right">ยืมค้างคืน (บาท)</Table.Head><Table.Head
										>สถานะ</Table.Head
									><Table.Head>รายละเอียด</Table.Head></Table.Row
								></Table.Header
							><Table.Body
								>{#each region.value.items as item (item.id)}<Table.Row
										><Table.Cell class="min-w-48"
											><p class="font-medium">{adjustmentLabels[item.kind]}</p>
											<p class="text-sm text-muted-foreground">{item.reason}</p></Table.Cell
										><Table.Cell class="whitespace-nowrap text-right tabular-nums"
											>{money(item.amountSatang)}</Table.Cell
										><Table.Cell class="whitespace-nowrap text-right tabular-nums"
											>{item.kind === 'loan' ? money(item.loanOutstandingSatang) : '—'}</Table.Cell
										><Table.Cell><StatusBadge status={item.status} /></Table.Cell><Table.Cell
											><Button
												size="sm"
												variant="outline"
												href={`/staff/budget/adjustments/${item.id}?returnTo=${encodeURIComponent(page.url.pathname + page.url.search)}`}
												>เปิดรายการ</Button
											></Table.Cell
										></Table.Row
									>{/each}</Table.Body
							></Table.Root
						>
					</div>{:else if region.value && !region.error}<PageState
						title="ยังไม่มีรายการปรับงบประมาณ"
						description="รายการเพิ่ม ลด โอน ยืม และคืนเงินยืมจะแสดงที่นี่"
					/>{/if}
			</Card.Content></Card.Root
		>
	</section>
	{#if region.value}<Paging
			path="/staff/budget/adjustments"
			{query}
			total={region.value.total}
			limit={region.value.limit}
			offset={region.value.offset}
		/>{/if}</PageShell
>
<Dialog.Root bind:open={creating}
	><Dialog.Content class="max-h-[90dvh] overflow-y-auto sm:max-w-3xl"
		><Dialog.Header
			><Dialog.Title>สร้างรายการปรับงบประมาณ</Dialog.Title><Dialog.Description
				>ระบุงบประมาณและเหตุผลก่อนส่งพิจารณา</Dialog.Description
			></Dialog.Header
		>{#if creating}<AdjustmentForm
				oncancel={() => (creating = false)}
				onsaved={(value) => {
					creating = false;
					void goto(`/staff/budget/adjustments/${value.id}`);
				}}
			/>{/if}</Dialog.Content
	></Dialog.Root
>
