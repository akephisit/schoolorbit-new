<script lang="ts">
	import type { CurriculumStructureWorkspace } from '$lib/api/academic-core';
	import { buildCurriculumDocument } from '$lib/academic/curriculum-structure';
	import { Badge } from '$lib/components/ui/badge';
	import * as Table from '$lib/components/ui/table';

	let {
		workspace,
		studyProgramId,
		gradeLevelId
	}: {
		workspace: CurriculumStructureWorkspace;
		studyProgramId: string;
		gradeLevelId: string;
	} = $props();

	let document = $derived(buildCurriculumDocument(workspace, studyProgramId, gradeLevelId));
</script>

<article class="overflow-hidden rounded-xl border bg-card">
	<header class="border-b px-5 py-4 text-center sm:px-8">
		<div class="text-xs font-medium tracking-[0.16em] text-primary">โครงสร้างหลักสูตรสถานศึกษา</div>
		<h2 class="mt-1 text-lg font-semibold tracking-tight sm:text-xl">
			{document.program?.nameTh ?? 'เลือกแผนการเรียน'} · {document.gradeName}
		</h2>
		<p class="mt-1 text-sm text-muted-foreground">{workspace.curriculumVersion.versionName}</p>
	</header>

	<div class="overflow-x-auto p-3 sm:p-5">
		<div class="grid min-w-[680px] gap-4" style:grid-template-columns={`repeat(${Math.max(document.termPanels.length, 1)}, minmax(330px, 1fr))`}>
			{#each document.termPanels as panel (panel.id)}
				<section class="overflow-hidden rounded-lg border bg-background">
					<div class="flex items-center justify-between border-b bg-primary px-4 py-2.5 text-primary-foreground">
						<h3 class="font-semibold">{panel.name}</h3>
						<span class="text-xs tabular-nums">{panel.totalCredits} หน่วยกิต</span>
					</div>
					<Table.Root>
						<Table.Header>
							<Table.Row class="bg-muted/40">
								<Table.Head class="w-24">รหัส</Table.Head>
								<Table.Head>รายวิชา / กิจกรรม</Table.Head>
								<Table.Head class="w-16 text-center">ต่อสัปดาห์</Table.Head>
								<Table.Head class="w-16 text-center">หน่วยกิต</Table.Head>
								<Table.Head class="w-16 text-center">รวม ชม.</Table.Head>
							</Table.Row>
						</Table.Header>
						<Table.Body>
							{#each panel.sections as section (section.id)}
								<Table.Row class="bg-primary/[0.04] hover:bg-primary/[0.04]">
									<Table.Cell colspan={5} class="py-1.5 text-xs font-semibold text-primary">
										{section.label}
									</Table.Cell>
								</Table.Row>
								{#each section.rows as row (row.id)}
									<Table.Row>
										<Table.Cell class="font-mono text-xs font-semibold">{row.code}</Table.Cell>
										<Table.Cell>
											<div class="font-medium">{row.name}</div>
											<Badge variant="secondary" class="mt-1 text-[10px]">
												{row.requirementKind === 'required' ? 'บังคับ' : row.requirementKind === 'elective' ? 'เลือก' : 'เพิ่มเติม'}
											</Badge>
										</Table.Cell>
										<Table.Cell class="text-center tabular-nums">{row.metrics.weeklyValue ?? '—'}</Table.Cell>
										<Table.Cell class="text-center tabular-nums">{row.metrics.credit ?? '—'}</Table.Cell>
										<Table.Cell class="text-center tabular-nums">{row.metrics.totalHours ?? '—'}</Table.Cell>
									</Table.Row>
								{/each}
								{#if section.rows.length === 0}
									<Table.Row>
										<Table.Cell colspan={5} class="py-2 text-center text-xs text-muted-foreground">—</Table.Cell>
									</Table.Row>
								{/if}
							{/each}
						</Table.Body>
						<Table.Footer>
							<Table.Row>
								<Table.Cell colspan={3} class="font-semibold">รวม {panel.name}</Table.Cell>
								<Table.Cell class="text-center font-semibold tabular-nums">{panel.totalCredits}</Table.Cell>
								<Table.Cell class="text-center font-semibold tabular-nums">{panel.totalHours}</Table.Cell>
							</Table.Row>
						</Table.Footer>
					</Table.Root>
				</section>
			{/each}
		</div>
	</div>

	<footer class="flex justify-end gap-5 border-t bg-muted/25 px-5 py-3 text-sm tabular-nums sm:px-8">
		<span>รวม <strong>{document.totalCredits}</strong> หน่วยกิต</span>
		<span>รวม <strong>{document.totalHours}</strong> ชั่วโมง</span>
	</footer>
</article>
