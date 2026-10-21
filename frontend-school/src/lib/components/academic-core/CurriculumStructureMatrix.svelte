<script lang="ts">
	import type { CurriculumStructureWorkspace } from '#lib/api/academic-core.js';
	import {
		buildCurriculumMatrix,
		curriculumMetric,
		curriculumTotals,
		curriculumSummaryMetrics,
		matrixCellKey,
		type MetricTotal,
		type CurriculumSummaryRow
	} from '#lib/academic/curriculum-structure.js';
	import CurriculumOutdatedBadge from '#lib/components/academic-core/CurriculumOutdatedBadge.svelte';
	import CurriculumSummaryTable from '#lib/components/academic-core/CurriculumSummaryTable.svelte';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import * as Collapsible from '#lib/components/ui/collapsible/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	let {
		workspace,
		gradeLevelId,
		termSlotIds,
		studyProgramId
	}: {
		workspace: CurriculumStructureWorkspace;
		gradeLevelId: string;
		termSlotIds?: readonly string[];
		studyProgramId?: string;
	} = $props();
	let matrix = $derived(
		buildCurriculumMatrix(workspace, gradeLevelId, termSlotIds, studyProgramId)
	);
	const kindLabels = { required: 'บังคับ', elective: 'เลือก', optional: 'เพิ่มเติม' } as const;
	function scopeRows(programId: string, gradeId: string, termId?: string, section?: string) {
		return workspace.requirements.filter(
			(row) =>
				row.studyProgramId === programId &&
				row.gradeLevel.id === gradeId &&
				(!termId || row.termSlotId === termId) &&
				(!section || row.section === section)
		);
	}
	function annualRows(gradeId: string): CurriculumSummaryRow[] {
		return [
			{
				key: gradeId,
				label: 'รวมทั้งปี · ทุกภาคเรียน',
				totals: Object.fromEntries(
					matrix.programs.map((program) => [
						program.id,
						curriculumSummaryMetrics(curriculumTotals(scopeRows(program.id, gradeId)))
					])
				)
			}
		];
	}
	let summaryRows: CurriculumSummaryRow[] = $derived([
		{
			key: 'selected',
			label: `รวมช่วงที่เลือก · ${gradeLevelId === 'all' ? 'ทุกชั้น' : (matrix.grades[0]?.grade.name ?? '')} · ${matrix.terms.map((term) => term.name).join(', ') || 'ไม่มีภาคเรียน'}`,
			totals: Object.fromEntries(
				matrix.summaries.map((summary) => [
					summary.program.id,
					curriculumSummaryMetrics(
						summary.selected,
						matrix.grades.length === 1 && matrix.terms.length === 1
					)
				])
			)
		},
		{
			key: 'level',
			label: 'รวมทั้งระดับ · ทุกชั้น ทุกภาคเรียน',
			totals: Object.fromEntries(
				matrix.summaries.map((summary) => [
					summary.program.id,
					curriculumSummaryMetrics(summary.level)
				])
			)
		}
	]);
	function measureHeader(node: HTMLDivElement) {
		const header = node.querySelector('thead');
		if (!header) return;
		const observer = new ResizeObserver(() =>
			node.style.setProperty('--matrix-header-height', `${header.getBoundingClientRect().height}px`)
		);
		observer.observe(header);
		return { destroy: () => observer.disconnect() };
	}
</script>

{#snippet metric(total: MetricTotal)}
	{total.value}{#if total.partial}<abbr
			class="ml-0.5 no-underline"
			title="รวมเฉพาะค่าที่มีข้อมูล ข้อมูลบางรายการไม่ครบ"
			aria-label="ข้อมูลบางรายการไม่ครบ">*</abbr
		>{/if}
{/snippet}
<div class="space-y-4" data-testid="curriculum-matrix">
	{#each matrix.grades as { grade, sections } (grade.id)}
		<section
			class="overflow-hidden rounded-xl border bg-card"
			aria-label={`โครงสร้าง ${grade.name}`}
		>
			<div class="border-b px-4 py-3">
				<h3 class="font-semibold">{grade.name}</h3>
				<p class="text-xs text-muted-foreground">
					{matrix.terms.map((term) => term.name).join(' · ') || 'ยังไม่ได้เลือกภาคเรียน'}
				</p>
			</div>
			{#if matrix.terms.length}
				<div class="matrix-viewport" use:measureHeader>
					<Table.Root class="matrix-table" aria-label={`รายวิชาและกิจกรรม ${grade.name}`}>
						<Table.Header class="matrix-head">
							<Table.Row
								>{#each matrix.programs as program (program.id)}<Table.Head
										colspan={matrix.terms.length * 4}
										scope="colgroup"
										class="border-l text-center whitespace-normal font-semibold"
										>{program.nameTh}</Table.Head
									>{/each}</Table.Row
							>
							<Table.Row
								>{#each matrix.programs as program (program.id)}{#each matrix.terms as term (term.id)}<Table.Head
											colspan={4}
											scope="colgroup"
											class="border-l text-center whitespace-normal">{term.name}</Table.Head
										>{/each}{/each}</Table.Row
							>
							<Table.Row
								>{#each matrix.programs as program (program.id)}{#each matrix.terms as term (term.id)}
										<Table.Head class="identity-head border-l whitespace-normal">รายวิชา</Table.Head
										>
										<Table.Head class="metric-head"><span>หน่วยกิต</span></Table.Head>
										<Table.Head class="metric-head"><span>ชั่วโมง/สัปดาห์</span></Table.Head>
										<Table.Head class="metric-head"><span>ชั่วโมง/ภาคเรียน</span></Table.Head>
									{/each}{/each}</Table.Row
							>
						</Table.Header>
						{#each sections as section (section.id)}
							<Table.Body>
								<Table.Row class="section-heading">
									{#each matrix.programs as program (program.id)}{#each matrix.terms as term (term.id)}
											{@const totals = curriculumTotals(
												scopeRows(program.id, grade.id, term.id, section.id)
											)}
											<Table.Cell
												class="identity-cell border-l whitespace-normal font-semibold text-primary"
												>{section.label}</Table.Cell
											>
											<Table.Cell class="metric-cell"
												>{#if section.id === 'student_development'}—{:else}{@render metric(
														totals.credits
													)}{/if}</Table.Cell
											>
											<Table.Cell class="metric-cell"
												>{@render metric(totals.weeklySchoolHours)}</Table.Cell
											>
											<Table.Cell class="metric-cell">{@render metric(totals.hours)}</Table.Cell>
										{/each}{/each}
								</Table.Row>
								{#each section.rows as row, index (row.key)}
									{@const count = Math.max(
										1,
										...matrix.programs.flatMap((program) =>
											matrix.terms.map(
												(term) => row.cells[matrixCellKey(program.id, term.id)]?.length ?? 0
											)
										)
									)}
									{#each Array.from({ length: count }, (_, i) => i) as occurrence (occurrence)}
										<Table.Row
											class={index > 0 &&
											occurrence === 0 &&
											section.rows[index - 1].groupId !== row.groupId
												? 'group-start'
												: 'within-group'}
										>
											{#each matrix.programs as program (program.id)}{#each matrix.terms as term (term.id)}
													{@const requirement =
														row.cells[matrixCellKey(program.id, term.id)]?.[occurrence]}
													{#if requirement}
														<Table.Cell
															class="identity-cell border-l whitespace-normal align-top"
															data-requirement-id={requirement.id}
														>
															{#if requirement.resourceKind === 'course'}<span
																	class="block text-xs font-medium text-primary tabular-nums"
																	>{requirement.code}</span
																>{/if}
															<span
																>{requirement.name}
																<Badge variant="secondary" class="inline-flex px-1 py-0 text-[10px]"
																	>{kindLabels[requirement.requirementKind]}</Badge
																></span
															>
															<CurriculumOutdatedBadge
																hasNewerCurrentVersion={requirement.hasNewerCurrentVersion}
															/>
														</Table.Cell>
														<Table.Cell class="metric-cell"
															>{requirement.resourceKind === 'activity'
																? '—'
																: curriculumMetric(requirement.metrics.credit)}</Table.Cell
														>
														<Table.Cell class="metric-cell"
															>{curriculumMetric(requirement.metrics.weeklyValue)}</Table.Cell
														>
														<Table.Cell class="metric-cell"
															>{curriculumMetric(requirement.metrics.totalHours)}</Table.Cell
														>
													{:else}<Table.Cell
															colspan={4}
															class="border-l text-center text-muted-foreground"
															aria-label="ไม่มีรายการ">—</Table.Cell
														>{/if}
												{/each}{/each}
										</Table.Row>
									{/each}
								{/each}
								{#if !section.rows.length}<Table.Row
										><Table.Cell
											colspan={matrix.programs.length * matrix.terms.length * 4}
											class="py-3 text-center text-muted-foreground"
											>ยังไม่มีรายการในหมวดนี้</Table.Cell
										></Table.Row
									>{/if}
							</Table.Body>
						{/each}
						<Table.Footer
							><Table.Row>
								{#each matrix.programs as program (program.id)}{#each matrix.terms as term (term.id)}
										{@const totals = curriculumTotals(scopeRows(program.id, grade.id, term.id))}
										<Table.Cell class="identity-cell border-l whitespace-normal font-semibold"
											>รวมรายภาคเรียน</Table.Cell
										>
										<Table.Cell class="metric-cell">{@render metric(totals.credits)}</Table.Cell>
										<Table.Cell class="metric-cell"
											>{@render metric(totals.weeklySchoolHours)}</Table.Cell
										>
										<Table.Cell class="metric-cell">{@render metric(totals.hours)}</Table.Cell>
									{/each}{/each}
							</Table.Row></Table.Footer
						>
					</Table.Root>
				</div>
			{:else}<p class="p-4 text-sm text-muted-foreground">เลือกภาคเรียนเพื่อแสดงตาราง</p>{/if}
			<div class="border-t">
				<CurriculumSummaryTable
					programs={matrix.programs}
					rows={annualRows(grade.id)}
					label={`สรุปทั้งปี ${grade.name} · ทุกภาคเรียน`}
				/>
			</div>
		</section>
	{/each}
	<div class="space-y-3 rounded-xl border bg-card p-4">
		<p class="text-xs text-muted-foreground">
			* ข้อมูลบางรายการไม่ครบ ยอดรวมใช้เฉพาะค่าที่มีข้อมูล · กิจกรรมไม่มีหน่วยกิต ·
			ชั่วโมงตามหลักสูตรนับ 1 คาบเรียนเป็น 1 ชั่วโมงเรียนตามโครงสร้าง โดยใช้จำนวนเดิมในทะเบียน ·
			ยอดรายปีและทั้งระดับไม่รวมชั่วโมงต่อสัปดาห์ข้ามภาคเรียน
		</p>
		<CurriculumSummaryTable
			programs={matrix.programs}
			rows={summaryRows}
			label="สรุปช่วงที่เลือกและทั้งระดับ"
		/>
		{#each matrix.summaries as summary (summary.program.id)}
			<Collapsible.Root class="text-xs text-muted-foreground">
				<Collapsible.Trigger class="py-1 text-left"
					>{summary.program.nameTh} · ชั่วโมงต่อสัปดาห์ แยกทุกชั้นและภาคเรียน</Collapsible.Trigger
				>
				<Collapsible.Content
					><CurriculumSummaryTable
						programs={[summary.program]}
						rows={summary.weekly.map((item) => ({
							key: `${item.grade.id}:${item.term.id}`,
							label: `${item.grade.short_name ?? item.grade.name} · ${item.term.name}`,
							totals: { [summary.program.id]: curriculumSummaryMetrics(item.totals, true) }
						}))}
						label="ชั่วโมงตามชั้นและภาคเรียน"
					/></Collapsible.Content
				>
			</Collapsible.Root>
		{/each}
	</div>
</div>

<style>
	.matrix-viewport {
		--matrix-header-height: 10rem;
	}
	.matrix-viewport :global(> [data-slot='table-container']) {
		max-height: 70dvh;
		overflow: auto;
		overscroll-behavior-x: contain;
	}
	.matrix-viewport :global(.matrix-table) {
		border-collapse: separate;
		border-spacing: 0;
	}
	.matrix-viewport :global(.matrix-head) {
		position: sticky;
		top: 0;
		z-index: 30;
		background: var(--card);
	}
	.matrix-viewport :global(.matrix-head th) {
		background: var(--card);
	}
	.matrix-viewport :global(.identity-head),
	.matrix-viewport :global(.identity-cell) {
		min-width: 10rem;
		max-width: 15rem;
		padding: 0.6rem;
		overflow-wrap: anywhere;
	}
	.matrix-viewport :global(.metric-head) {
		min-width: 3.25rem;
		height: 6.5rem;
		text-align: center;
	}
	.matrix-viewport :global(.metric-head span) {
		writing-mode: vertical-rl;
		transform: rotate(180deg);
		display: inline-block;
		white-space: nowrap;
		font-size: 0.7rem;
	}
	.matrix-viewport :global(.metric-cell) {
		text-align: center;
		font-variant-numeric: tabular-nums;
		padding: 0.6rem 0.35rem;
	}
	.matrix-viewport :global(.section-heading td) {
		position: sticky;
		top: var(--matrix-header-height);
		z-index: 20;
		background: var(--muted);
		border-block: 1px solid var(--border);
		padding: 0.6rem 0.75rem;
	}
	.matrix-viewport :global(.within-group td) {
		border-bottom: 0.5px solid color-mix(in srgb, var(--border) 55%, transparent);
	}
	.matrix-viewport :global(.group-start td) {
		border-top: 1px solid var(--border);
		border-bottom: 0.5px solid color-mix(in srgb, var(--border) 55%, transparent);
	}
	.matrix-viewport :global(tfoot td) {
		background: var(--muted);
		padding: 0.7rem;
		border-top: 1px solid var(--border);
	}
</style>
