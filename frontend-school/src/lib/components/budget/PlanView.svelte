<script lang="ts">
	import type { BudgetActivity, BudgetCatalogs } from '#lib/api/budget.js';
	import { money } from '#lib/budget/money.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	let {
		activities,
		catalogs = null
	}: { activities: BudgetActivity[]; catalogs?: BudgetCatalogs | null } = $props();
</script>

<div class="space-y-4">
	{#each activities as activity (activity.id)}<Card.Root class="gap-0 overflow-hidden py-0"
			><Card.Header class="gap-3 py-5">
				<div class="flex flex-wrap items-center justify-between gap-2">
					<Card.Title><h3>{activity.title}</h3></Card.Title>
					<p class="text-sm tabular-nums">{money(activity.totalSatang)} บาท</p>
				</div>
			</Card.Header>
			<div class="border-t">
				<Table.Root class="min-w-[48rem]"
					><Table.Header
						><Table.Row
							><Table.Head>รายการ</Table.Head><Table.Head class="whitespace-nowrap text-right"
								>จำนวน</Table.Head
							><Table.Head class="whitespace-nowrap text-right">ราคา / หน่วย</Table.Head><Table.Head
								>แหล่งเงิน</Table.Head
							><Table.Head>หมวดรายจ่าย</Table.Head><Table.Head class="whitespace-nowrap text-right"
								>รวม</Table.Head
							></Table.Row
						></Table.Header
					><Table.Body
						>{#each activity.lines as line (line.id)}<Table.Row
								><Table.Cell class="min-w-44">{line.description}</Table.Cell><Table.Cell
									class="whitespace-nowrap text-right tabular-nums"
									>{line.quantity} {line.unit}</Table.Cell
								><Table.Cell class="whitespace-nowrap text-right tabular-nums"
									>{money(line.unitPriceSatang)}</Table.Cell
								><Table.Cell
									>{catalogs?.funds.find((item) => item.id === line.fundId)?.name ??
										'—'}</Table.Cell
								><Table.Cell
									>{catalogs?.expenseCategories.find((item) => item.id === line.expenseCategoryId)
										?.name ?? '—'}</Table.Cell
								><Table.Cell class="whitespace-nowrap text-right tabular-nums"
									>{money(line.totalSatang)}</Table.Cell
								></Table.Row
							>{/each}</Table.Body
					></Table.Root
				>
			</div>
		</Card.Root>{/each}
</div>
