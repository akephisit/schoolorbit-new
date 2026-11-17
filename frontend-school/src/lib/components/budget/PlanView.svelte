<script lang="ts">
	import type { BudgetActivity, BudgetCatalogs } from '#lib/api/budget.js';
	import ActivitySchedule from './ActivitySchedule.svelte';
	import { dateLabel } from '#lib/budget/presentation.js';
	import { money } from '#lib/budget/money.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	let {
		activities,
		catalogs = null
	}: { activities: BudgetActivity[]; catalogs?: BudgetCatalogs | null } = $props();
</script>

<div class="space-y-4">
	<ActivitySchedule {activities} />
	{#each activities as activity (activity.id)}<Card.Root class="gap-0 overflow-hidden py-0"
			><Card.Header class="gap-3 py-5">
				<div class="flex flex-wrap items-center justify-between gap-2">
					<Card.Title class="min-w-0 break-words"><h3>{activity.title}</h3></Card.Title>
					<p class="text-sm tabular-nums">{money(activity.totalSatang)} บาท</p>
				</div>
				<p class="text-sm text-muted-foreground">
					{activity.startDate && activity.endDate
						? `${dateLabel(activity.startDate)} – ${dateLabel(activity.endDate)}`
						: 'ยังไม่บันทึกวันที่กิจกรรม'}
				</p>
			</Card.Header>
			<ul
				class="divide-y border-t px-4 md:hidden"
				aria-label={`รายการค่าใช้จ่าย ${activity.title}`}
			>
				{#each activity.lines as line (line.id)}<li class="space-y-3 py-4">
						<h4 class="break-words font-medium">{line.description}</h4>
						<dl class="grid grid-cols-2 gap-3 text-sm">
							<div>
								<dt class="text-muted-foreground">จำนวน</dt>
								<dd class="break-words tabular-nums">{line.quantity} {line.unit}</dd>
							</div>
							<div>
								<dt class="text-muted-foreground">ราคา / หน่วย</dt>
								<dd class="tabular-nums">{money(line.unitPriceSatang)} บาท</dd>
							</div>
							<div>
								<dt class="text-muted-foreground">แหล่งเงิน</dt>
								<dd class="break-words">
									{catalogs?.funds.find((item) => item.id === line.fundId)?.name ?? '—'}
								</dd>
							</div>
							<div>
								<dt class="text-muted-foreground">หมวดรายจ่าย</dt>
								<dd class="break-words">
									{catalogs?.expenseCategories.find((item) => item.id === line.expenseCategoryId)
										?.name ?? '—'}
								</dd>
							</div>
						</dl>
						<p class="text-right font-semibold tabular-nums">รวม {money(line.totalSatang)} บาท</p>
					</li>{/each}
			</ul>
			<div class="hidden border-t md:block">
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
