<script lang="ts">
	import type { BudgetDashboardRow } from '#lib/api/budget.js';
	import { budgetPivot, type BudgetPivotMetric } from '#lib/budget/pivot.js';
	import { money } from '#lib/budget/money.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	let { rows, metric }: { rows: BudgetDashboardRow[]; metric: BudgetPivotMetric } = $props();
	const pivot = $derived(budgetPivot(rows, metric));
</script>

<Card.Root class="gap-0 overflow-hidden py-0"
	><div class="min-w-0" data-testid="budget-pivot">
		<Table.Root class="min-w-[40rem]"
			><Table.Caption>จำนวนเงินเป็นบาท</Table.Caption><Table.Header
				><Table.Row
					><Table.Head rowspan={2}>โครงการ / กิจกรรม</Table.Head
					>{#each pivot.groups as group (group.id)}<Table.Head
							colspan={group.columns.length}
							class="border-l text-center">{group.name}</Table.Head
						>{/each}<Table.Head rowspan={2} class="border-l text-right">รวม</Table.Head></Table.Row
				><Table.Row
					>{#each pivot.groups as group (group.id)}{#each group.columns as column (column.key)}<Table.Head
								class="min-w-32 border-l text-right">{column.name}</Table.Head
							>{/each}{/each}</Table.Row
				></Table.Header
			><Table.Body
				>{#each pivot.activities as activity, index (activity.key)}<Table.Row
						><Table.Cell class="min-w-56"
							><p class="font-medium">{index + 1}. {activity.title}</p>
							<Button
								variant="link"
								size="sm"
								class="h-auto p-0 whitespace-normal text-left text-sm text-muted-foreground"
								href={`/staff/budget/projects/${activity.projectId}`}>{activity.projectName}</Button
							></Table.Cell
						>{#each pivot.columns as column (column.key)}<Table.Cell
								class="border-l text-right tabular-nums"
								>{money(activity.values[column.key] ?? '0')}</Table.Cell
							>{/each}<Table.Cell class="border-l text-right font-semibold tabular-nums"
							>{money(activity.totalSatang)}</Table.Cell
						></Table.Row
					>{/each}</Table.Body
			><Table.Footer
				><Table.Row
					><Table.Cell class="font-semibold">รวมทั้งหมด</Table.Cell
					>{#each pivot.columns as column (column.key)}<Table.Cell
							class="border-l text-right font-semibold tabular-nums"
							>{money(pivot.columnTotals[column.key])}</Table.Cell
						>{/each}<Table.Cell class="border-l text-right font-semibold tabular-nums"
						>{money(pivot.totalSatang)}</Table.Cell
					></Table.Row
				></Table.Footer
			></Table.Root
		>
	</div></Card.Root
>
