<script lang="ts">
	import { onDestroy, tick } from 'svelte';
	import { MoreHorizontal } from '@lucide/svelte';
	import * as ContextMenu from '#lib/components/ui/context-menu/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import type {
		CurriculumQuickAction,
		CurriculumQuickActionContext
	} from '#lib/academic-core/curriculum-quick-actions.js';
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
	import { assessBasicCredits } from '#lib/academic-core/curriculum-credit-criteria.js';
	import CurriculumOutdatedBadge from '#lib/components/academic-core/CurriculumOutdatedBadge.svelte';
	import CurriculumSummaryTable from '#lib/components/academic-core/CurriculumSummaryTable.svelte';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import * as Collapsible from '#lib/components/ui/collapsible/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	let {
		workspace,
		gradeLevelId,
		termSlotIds,
		studyProgramId,
		canManage = false,
		onQuickAction,
		busy = false
	}: {
		workspace: CurriculumStructureWorkspace;
		gradeLevelId: string;
		termSlotIds?: readonly string[];
		studyProgramId?: string;
		canManage?: boolean;
		onQuickAction?: (action: CurriculumQuickAction) => void;
		busy?: boolean;
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
	function annualMetrics(programId: string, gradeId: string) {
		const totals = curriculumTotals(scopeRows(programId, gradeId));
		return {
			...curriculumSummaryMetrics(totals),
			basicAssessment: assessBasicCredits(
				totals.basicCredits,
				workspace.level.creditCriteria.find((criterion) => criterion.scope === 'annual'),
				workspace.level.gradeLevelIds,
				'annual',
				gradeId
			)
		};
	}
	function annualRows(gradeId: string): CurriculumSummaryRow[] {
		return [
			{
				key: gradeId,
				label: 'รวมทั้งปี · ทุกภาคเรียน',
				totals: Object.fromEntries(
					matrix.programs.map((program) => [program.id, annualMetrics(program.id, gradeId)])
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
					{
						...curriculumSummaryMetrics(summary.level),
						basicAssessment: assessBasicCredits(
							summary.level.basicCredits,
							workspace.level.creditCriteria.find((criterion) => criterion.scope === 'level'),
							workspace.level.gradeLevelIds,
							'level'
						)
					}
				])
			)
		}
	]);
	let menuOpen = $state(false);
	let menuTrigger = $state<HTMLDivElement | null>(null);
	let menuTarget = $state<{
		context: CurriculumQuickActionContext;
		requirementId?: string;
		key: string;
	} | null>(null);
	let returnFocus: HTMLElement | null = null;
	let chosen = false;
	let pendingAction: {
		action: CurriculumQuickAction;
		workspace: CurriculumStructureWorkspace;
	} | null = null;
	let editable = $derived(canManage && !!workspace.level.draftId && !!onQuickAction && !busy);
	$effect(() => {
		const currentWorkspace = workspace;
		if (!editable || (pendingAction && pendingAction.workspace !== currentWorkspace)) {
			pendingAction = null;
			menuOpen = false;
		}
	});
	onDestroy(() => {
		pendingAction = null;
	});
	function targetProps(
		context: CurriculumQuickActionContext,
		key: string,
		requirementId?: string,
		focusable = true
	) {
		return {
			'data-program-id': context.studyProgramId,
			'data-grade-id': context.gradeLevelId,
			'data-term-id': context.termSlotId,
			'data-section-id': context.section,
			'data-quick-target': key,
			'data-selected': menuOpen && menuTarget?.key === key ? 'true' : undefined,
			tabindex: editable && focusable ? 0 : undefined,
			oncontextmenu: (event: MouseEvent) => {
				if (!editable) return;
				event.preventDefault();
				event.stopPropagation();
				openMenu(
					event.currentTarget as HTMLElement,
					context,
					key,
					requirementId,
					event.clientX,
					event.clientY
				);
			},
			onkeydown: (event: KeyboardEvent) => {
				if (!editable || !(event.key === 'ContextMenu' || (event.shiftKey && event.key === 'F10')))
					return;
				event.preventDefault();
				event.stopPropagation();
				openMenu(event.currentTarget as HTMLElement, context, key, requirementId);
			}
		};
	}
	function openMenu(
		node: HTMLElement,
		context: CurriculumQuickActionContext,
		key: string,
		requirementId?: string,
		x?: number,
		y?: number
	) {
		if (!editable || !menuTrigger) return;
		const identity = Array.from(
			node.closest('tr')?.querySelectorAll<HTMLElement>('[data-quick-target][tabindex="0"]') ?? []
		).find((cell) => cell.dataset.quickTarget === key);
		node = identity ?? node;
		chosen = false;
		pendingAction = null;
		returnFocus = node;
		node.focus();
		menuTarget = { context, key, requirementId };
		const rect = node.getBoundingClientRect();
		menuTrigger.dispatchEvent(
			new MouseEvent('contextmenu', {
				bubbles: true,
				cancelable: true,
				clientX: x ?? rect.left + 16,
				clientY: y ?? rect.top + 16
			})
		);
		void tick().then(() => {
			document
				.querySelector<HTMLElement>('[data-slot="context-menu-content"] [role="menuitem"]')
				?.focus();
		});
	}
	function choose(kind: CurriculumQuickAction['kind']) {
		if (!menuTarget || !editable) return;
		const { context, requirementId } = menuTarget;
		if (kind === 'edit' || kind === 'remove') {
			if (!requirementId) return;
			pendingAction = { action: { kind, context, requirementId }, workspace };
		} else pendingAction = { action: { kind, context }, workspace };
		chosen = true;
		menuOpen = false;
	}
	function completeMenuTransition(open: boolean) {
		if (open || !pendingAction) return;
		const pending = pendingAction;
		pendingAction = null;
		if (!editable || pending.workspace !== workspace) return;
		returnFocus?.focus();
		onQuickAction?.(pending.action);
	}
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
{#snippet touchAction(context: CurriculumQuickActionContext, key: string, requirementId?: string)}
	{#if editable}<Button
			variant="ghost"
			size="icon-sm"
			class="quick-touch-action ml-1 shrink-0"
			aria-label="เปิดเมนูรายการ"
			onclick={(event) => {
				event.stopPropagation();
				const node = event.currentTarget.closest<HTMLElement>('[data-quick-target]');
				if (node) openMenu(node, context, key, requirementId);
			}}><MoreHorizontal class="size-4" /></Button
		>{/if}
{/snippet}
<ContextMenu.Root bind:open={menuOpen} onOpenChangeComplete={completeMenuTransition}>
	<ContextMenu.Trigger
		bind:ref={menuTrigger}
		class="sr-only"
		aria-hidden="true"
		disabled={!editable}
	/>
	<!-- The root must finish closing its content before an action makes the matrix busy. -->
	<ContextMenu.Content
		class="max-w-[calc(100vw-1rem)] w-72"
		collisionPadding={8}
		onCloseAutoFocus={(event) => {
			event.preventDefault();
			if (!chosen) returnFocus?.focus();
		}}
	>
		<ContextMenu.Group
			><ContextMenu.GroupHeading class="px-2 py-1.5 text-xs text-muted-foreground whitespace-normal"
				>{workspace.programs.find((p) => p.id === menuTarget?.context.studyProgramId)?.nameTh} · {workspace.gradeLevels.find(
					(g) => g.id === menuTarget?.context.gradeLevelId
				)?.name} · {workspace.termSlots.find((t) => t.id === menuTarget?.context.termSlotId)
					?.name}</ContextMenu.GroupHeading
			>
			<ContextMenu.Item onSelect={() => choose('add-course')}>เพิ่มรายวิชา…</ContextMenu.Item>
			<ContextMenu.Item onSelect={() => choose('add-activity')}>เพิ่มกิจกรรม…</ContextMenu.Item>
			{#if menuTarget?.requirementId}<ContextMenu.Item onSelect={() => choose('edit')}
					>แก้ไขเงื่อนไข…</ContextMenu.Item
				><ContextMenu.Item variant="destructive" onSelect={() => choose('remove')}
					>นำรายการนี้ออก…</ContextMenu.Item
				>{/if}</ContextMenu.Group
		>
	</ContextMenu.Content>
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
												{...targetProps(
													{
														studyProgramId: program.id,
														gradeLevelId: grade.id,
														termSlotId: term.id
													},
													`${program.id}:${grade.id}:${term.id}:term`
												)}
												data-term-header="true"
												colspan={4}
												scope="colgroup"
												class="border-l text-center whitespace-normal"
												>{term.name}{@render touchAction(
													{
														studyProgramId: program.id,
														gradeLevelId: grade.id,
														termSlotId: term.id
													},
													`${program.id}:${grade.id}:${term.id}:term`
												)}</Table.Head
											>{/each}{/each}</Table.Row
								>
								<Table.Row
									>{#each matrix.programs as program (program.id)}{#each matrix.terms as term (term.id)}
											<Table.Head class="identity-head border-l whitespace-normal"
												>รายวิชา</Table.Head
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
												{@const context = {
													studyProgramId: program.id,
													gradeLevelId: grade.id,
													termSlotId: term.id,
													section: section.id
												}}
												{@const key = `${program.id}:${grade.id}:${term.id}:${section.id}:section`}
												{@const totals = curriculumTotals(
													scopeRows(program.id, grade.id, term.id, section.id)
												)}
												<Table.Cell
													{...targetProps(context, key, undefined)}
													class="identity-cell border-l whitespace-normal font-semibold text-primary"
													>{section.label}{@render touchAction(context, key)}</Table.Cell
												>
												<Table.Cell
													{...targetProps(context, key, undefined, false)}
													class="metric-cell"
													>{#if section.id === 'student_development'}—{:else}{@render metric(
															totals.credits
														)}{/if}</Table.Cell
												>
												<Table.Cell
													{...targetProps(context, key, undefined, false)}
													class="metric-cell">{@render metric(totals.weeklySchoolHours)}</Table.Cell
												>
												<Table.Cell
													{...targetProps(context, key, undefined, false)}
													class="metric-cell">{@render metric(totals.hours)}</Table.Cell
												>
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
														{@const context = {
															studyProgramId: program.id,
															gradeLevelId: grade.id,
															termSlotId: term.id,
															section: section.id
														}}
														{@const key = `${program.id}:${grade.id}:${term.id}:${section.id}:${row.key}:${occurrence}`}
														{#if requirement}
															<Table.Cell
																{...targetProps(context, key, requirement.id)}
																class="identity-cell border-l whitespace-normal align-top"
																data-requirement-id={requirement.id}
															>
																{#if requirement.resourceKind === 'course'}<span
																		class="block text-xs font-medium text-primary tabular-nums"
																		>{requirement.code}</span
																	>{/if}
																<span
																	>{requirement.name}
																	<Badge
																		variant="secondary"
																		class="inline-flex px-1 py-0 text-[10px]"
																		>{kindLabels[requirement.requirementKind]}</Badge
																	></span
																>
																{@render touchAction(context, key, requirement.id)}
																<CurriculumOutdatedBadge
																	resourceKind={requirement.resourceKind}
																	hasNewerCurrentVersion={requirement.hasNewerCurrentVersion}
																/>
															</Table.Cell>
															<Table.Cell
																{...targetProps(context, key, requirement.id, false)}
																class="metric-cell"
																>{requirement.resourceKind === 'activity'
																	? '—'
																	: curriculumMetric(requirement.metrics.credit)}</Table.Cell
															>
															<Table.Cell
																{...targetProps(context, key, requirement.id, false)}
																class="metric-cell"
																>{curriculumMetric(requirement.metrics.weeklyValue)}</Table.Cell
															>
															<Table.Cell
																{...targetProps(context, key, requirement.id, false)}
																class="metric-cell"
																>{curriculumMetric(requirement.metrics.totalHours)}</Table.Cell
															>
														{:else}<Table.Cell
																{...targetProps(context, key)}
																colspan={4}
																class="border-l text-center text-muted-foreground"
																aria-label="ไม่มีรายการ"
																>—{@render touchAction(context, key)}</Table.Cell
															>{/if}
													{/each}{/each}
											</Table.Row>
										{/each}
									{/each}
									{#if !section.rows.length}<Table.Row
											>{#each matrix.programs as program (program.id)}{#each matrix.terms as term (term.id)}
													{@const context = {
														studyProgramId: program.id,
														gradeLevelId: grade.id,
														termSlotId: term.id,
														section: section.id
													}}
													{@const key = `${program.id}:${grade.id}:${term.id}:${section.id}:empty`}
													<Table.Cell
														{...targetProps(context, key)}
														colspan={4}
														class="border-l py-3 text-center text-muted-foreground"
														>ยังไม่มีรายการในหมวดนี้{@render touchAction(context, key)}</Table.Cell
													>
												{/each}{/each}</Table.Row
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
						creditBreakdown
						label={`สรุปทั้งปี ${grade.name} · ทุกภาคเรียน`}
					/>
				</div>
			</section>
		{/each}
		<div class="space-y-3 rounded-xl border bg-card p-4">
			<p class="text-xs text-muted-foreground">
				* ข้อมูลบางรายการไม่ครบ ยอดรวมใช้เฉพาะค่าที่มีข้อมูล · หากประเภทวิชาไม่ครบ
				พื้นฐานและเพิ่มเติมอาจไม่เท่ากับยอดรวม · กิจกรรมไม่มีหน่วยกิต · ชั่วโมงตามหลักสูตรนับ 1
				คาบเรียนเป็น 1 ชั่วโมงเรียนตามโครงสร้าง โดยใช้จำนวนเดิมในทะเบียน ·
				ยอดรายปีและทั้งระดับไม่รวมชั่วโมงต่อสัปดาห์ข้ามภาคเรียน
			</p>
			<CurriculumSummaryTable
				programs={matrix.programs}
				rows={summaryRows}
				creditBreakdown
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
</ContextMenu.Root>

<style>
	:global([data-quick-target]:focus-visible) {
		outline: 2px solid var(--ring);
		outline-offset: -2px;
	}
	.matrix-viewport :global([data-quick-target][data-selected='true']) {
		background: color-mix(in srgb, var(--primary) 12%, var(--card));
	}
	:global(.quick-touch-action) {
		display: none;
	}
	@media (any-pointer: coarse) {
		:global(.quick-touch-action) {
			display: inline-flex;
		}
	}
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
