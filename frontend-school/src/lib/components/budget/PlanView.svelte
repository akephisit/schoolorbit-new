<script lang="ts">
	import type { BudgetActivity, BudgetCatalogs } from '#lib/api/budget.js';
	import { money } from '#lib/budget/money.js';
	import * as Table from '#lib/components/ui/table/index.js';
	let {
		activities,
		catalogs = null
	}: { activities: BudgetActivity[]; catalogs?: BudgetCatalogs | null } = $props();
</script>

<div class="space-y-4">
	{#each activities as activity (activity.id)}<section class="space-y-3">
			<div class="flex flex-wrap items-center justify-between gap-2">
				<h3 class="font-semibold">{activity.title}</h3>
				<p class="text-sm tabular-nums">{money(activity.totalSatang)} บาท</p>
			</div>
			<div class="overflow-x-auto rounded-lg border bg-card">
				<Table.Root
					><Table.Header
						><Table.Row
							><Table.Head>รายการ</Table.Head><Table.Head>จำนวน</Table.Head><Table.Head
								>ราคา / หน่วย</Table.Head
							><Table.Head>แหล่งเงิน</Table.Head><Table.Head>หมวดรายจ่าย</Table.Head><Table.Head
								>รวม</Table.Head
							></Table.Row
						></Table.Header
					><Table.Body
						>{#each activity.lines as line (line.id)}<Table.Row
								><Table.Cell class="min-w-44">{line.description}</Table.Cell><Table.Cell
									class="whitespace-nowrap tabular-nums">{line.quantity} {line.unit}</Table.Cell
								><Table.Cell class="whitespace-nowrap tabular-nums"
									>{money(line.unitPriceSatang)}</Table.Cell
								><Table.Cell
									>{catalogs?.funds.find((item) => item.id === line.fundId)?.name ??
										'—'}</Table.Cell
								><Table.Cell
									>{catalogs?.expenseCategories.find((item) => item.id === line.expenseCategoryId)
										?.name ?? '—'}</Table.Cell
								><Table.Cell class="whitespace-nowrap tabular-nums"
									>{money(line.totalSatang)}</Table.Cell
								></Table.Row
							>{/each}</Table.Body
					></Table.Root
				>
			</div>
		</section>{/each}
</div>
