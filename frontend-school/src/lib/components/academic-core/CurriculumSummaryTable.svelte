<script lang="ts">
	import type { CurriculumSummaryRow, MetricTotal } from '#lib/academic/curriculum-structure.js';
	import CurriculumCreditCriterionIndicator from './CurriculumCreditCriterionIndicator.svelte';
	import * as Table from '#lib/components/ui/table/index.js';
	let {
		programs,
		rows,
		label,
		creditBreakdown = false
	}: {
		programs: readonly { id: string; nameTh: string }[];
		rows: readonly CurriculumSummaryRow[];
		label: string;
		creditBreakdown?: boolean;
	} = $props();
</script>

{#snippet metric(total: MetricTotal | null)}
	{total?.value ?? '—'}{#if total?.partial}<abbr
			title="รวมเฉพาะค่าที่มีข้อมูล ข้อมูลบางรายการไม่ครบ"
			aria-label="ข้อมูลบางรายการไม่ครบ"
			class="ml-0.5 no-underline">*</abbr
		>{/if}
{/snippet}
<div class="summary-table">
	<Table.Root aria-label={label}>
		<Table.Header>
			<Table.Row>
				<Table.Head rowspan={2} class="min-w-44 whitespace-normal">{label}</Table.Head>
				{#each programs as program (program.id)}<Table.Head
						colspan={creditBreakdown ? 5 : 3}
						scope="colgroup"
						class="border-l text-center whitespace-normal">{program.nameTh}</Table.Head
					>{/each}
			</Table.Row>
			<Table.Row
				>{#each programs as program (program.id)}
					{#if creditBreakdown}
						<Table.Head class="min-w-24 border-l text-center whitespace-normal"
							>หน่วยกิตพื้นฐาน</Table.Head
						>
						<Table.Head class="min-w-24 text-center whitespace-normal">หน่วยกิตเพิ่มเติม</Table.Head
						>
					{/if}
					<Table.Head class="min-w-24 border-l text-center whitespace-normal"
						>หน่วยกิตรวม</Table.Head
					>
					<Table.Head class="min-w-28 text-center whitespace-normal">ชั่วโมง/สัปดาห์</Table.Head>
					<Table.Head class="min-w-24 text-center whitespace-normal">ชั่วโมงรวม</Table.Head>
				{/each}</Table.Row
			>
		</Table.Header>
		<Table.Body
			>{#each rows as row (row.key)}<Table.Row>
					<Table.Cell class="whitespace-normal">{row.label}</Table.Cell>
					{#each programs as program (program.id)}{@const totals = row.totals[program.id]}
						{#if creditBreakdown}
							<Table.Cell class="border-l text-center tabular-nums"
								>{@render metric(
									totals?.basicCredits ?? null
								)}{#if totals?.basicAssessment}<CurriculumCreditCriterionIndicator
										assessment={totals.basicAssessment}
									/>{/if}</Table.Cell
							>
							<Table.Cell class="text-center tabular-nums"
								>{@render metric(totals?.additionalCredits ?? null)}</Table.Cell
							>
						{/if}
						<Table.Cell class="border-l text-center tabular-nums"
							>{@render metric(totals?.credits ?? null)}</Table.Cell
						>
						<Table.Cell class="text-center tabular-nums"
							>{@render metric(totals?.weekly ?? null)}</Table.Cell
						>
						<Table.Cell class="text-center tabular-nums"
							>{@render metric(totals?.hours ?? null)}</Table.Cell
						>
					{/each}
				</Table.Row>{/each}</Table.Body
		>
	</Table.Root>
</div>

<style>
	.summary-table :global(th),
	.summary-table :global(td) {
		padding: 0.6rem 0.75rem;
	}
</style>
