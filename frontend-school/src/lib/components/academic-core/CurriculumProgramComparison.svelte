<script lang="ts">
	import type { CurriculumStructureWorkspace } from '$lib/api/academic-core';
	import { buildProgramComparison } from '$lib/academic/curriculum-structure';
	import { Badge } from '$lib/components/ui/badge';
	import * as Table from '$lib/components/ui/table';

	let { workspace, gradeLevelId }: { workspace: CurriculumStructureWorkspace; gradeLevelId: string } =
		$props();

	let comparison = $derived(buildProgramComparison(workspace, gradeLevelId));
	const kindLabels = { required: 'บังคับ', elective: 'เลือก', optional: 'เพิ่มเติม' } as const;
</script>

<div class="overflow-hidden rounded-xl border bg-card">
	<div class="border-b bg-muted/35 px-4 py-3">
		<h2 class="font-semibold tracking-tight">ภาพรวมทุกแผนการเรียน</h2>
		<p class="mt-0.5 text-sm text-muted-foreground">
			อ่านตามแถวเพื่อเห็นทันทีว่าวิชาเดียวกันอยู่ภาคเรียนใดและต่างกันระหว่างแผนตรงไหน
		</p>
	</div>
	<div class="overflow-x-auto">
		<Table.Root class="min-w-[880px]">
			<Table.Header class="sticky top-0 z-10 bg-card">
				<Table.Row>
					<Table.Head class="min-w-72">รายวิชา / กิจกรรม</Table.Head>
					{#each comparison.programs as program (program.id)}
						<Table.Head class="min-w-52 border-l align-top">
							<div class="font-semibold text-foreground">{program.name}</div>
							<div class="mt-0.5 font-mono text-xs font-normal text-muted-foreground">
								{program.code}{program.isDefault ? ' · แผนเริ่มต้น' : ''}
							</div>
						</Table.Head>
					{/each}
				</Table.Row>
			</Table.Header>
			<Table.Body>
				{#each comparison.sections as section (section.id)}
					<Table.Row class="border-y bg-primary/[0.045] hover:bg-primary/[0.045]">
						<Table.Cell colspan={comparison.programs.length + 1} class="py-2 font-semibold text-primary">
							{section.label}
						</Table.Cell>
					</Table.Row>
					{@const rows = comparison.rows.filter((row) => row.section === section.id)}
					{#each rows as row (row.key)}
						<Table.Row class={row.isDifferent ? 'bg-amber-50/60 dark:bg-amber-950/10' : ''}>
							<Table.Cell class="align-top">
								<div class="font-mono text-xs font-semibold text-primary">{row.code}</div>
								<div class="mt-1 font-medium">{row.name}</div>
								{#if row.isDifferent}
									<div class="mt-1 text-xs text-amber-700 dark:text-amber-400">ข้อมูลต่างกันระหว่างแผน</div>
								{/if}
							</Table.Cell>
							{#each comparison.programs as program (program.id)}
								{@const cell = row.cells[program.id]}
								<Table.Cell class="border-l align-top">
									{#if cell}
										<div class="flex flex-wrap gap-1">
											{#each cell.termNames as termName, index (`${termName}-${index}`)}
												<Badge variant="outline" class="bg-background">{termName}</Badge>
											{/each}
										</div>
										<div class="mt-2 text-xs text-muted-foreground">
											{cell.requirementKinds.map((kind) => kindLabels[kind]).join(', ')}
											{#if cell.credit} · {cell.credit} หน่วยกิต{/if}
											{#if cell.totalHours} · {cell.totalHours} ชม.{/if}
										</div>
									{:else}
										<span class="text-sm text-muted-foreground/70">—</span>
									{/if}
								</Table.Cell>
							{/each}
						</Table.Row>
					{/each}
					{#if rows.length === 0}
						<Table.Row>
							<Table.Cell colspan={comparison.programs.length + 1} class="py-4 text-center text-sm text-muted-foreground">
								ยังไม่มีรายการในหมวดนี้
							</Table.Cell>
						</Table.Row>
					{/if}
				{/each}
			</Table.Body>
		</Table.Root>
	</div>
</div>
