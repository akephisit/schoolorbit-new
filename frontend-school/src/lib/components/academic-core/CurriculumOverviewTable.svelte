<script lang="ts">
	import type { CurriculumOverviewItem } from '#lib/api/academic-core.js';
	import { gradeLevelSummary } from '#lib/academic-core/catalog-presentation.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	import { ArrowUpRight } from '@lucide/svelte';
	let { items }: { items: CurriculumOverviewItem[] } = $props();
	function curriculumLink(item: CurriculumOverviewItem) {
		const base = `/staff/academic/curricula/${item.curriculum.id}`;
		return item.displayVersion ? `${base}?versionId=${item.displayVersion.id}` : base;
	}
</script>

<div class="hidden overflow-x-auto md:block">
	<Table.Root>
		<Table.Header
			><Table.Row
				><Table.Head class="ps-5">ระดับการศึกษา / หลักสูตร</Table.Head><Table.Head
					>ระดับชั้น</Table.Head
				><Table.Head class="text-center">แผนการเรียน</Table.Head><Table.Head>สถานะ</Table.Head
				><Table.Head><span class="sr-only">เปิดหลักสูตร</span></Table.Head></Table.Row
			></Table.Header
		>
		<Table.Body>
			{#each items as item (item.curriculum.id)}
				<Table.Row>
					<Table.Cell class="ps-5 font-medium">{item.curriculum.nameTh}</Table.Cell>
					<Table.Cell>{gradeLevelSummary(item.gradeLevels)}</Table.Cell>
					<Table.Cell class="text-center tabular-nums">{item.studyProgramCount}</Table.Cell>
					<Table.Cell
						><div class="flex flex-wrap gap-1.5">
							<Badge variant="secondary"
								>{item.displayState === 'published' ? 'เผยแพร่แล้ว' : 'ยังไม่เผยแพร่'}</Badge
							>{#if item.draftCount > 0}<Badge variant="outline">ร่าง {item.draftCount}</Badge>{/if}
						</div></Table.Cell
					>
					<Table.Cell
						><Button
							href={curriculumLink(item)}
							variant="ghost"
							size="icon"
							aria-label={`เปิดหลักสูตร ${item.curriculum.nameTh}`}
							><ArrowUpRight class="size-4" /></Button
						></Table.Cell
					>
				</Table.Row>
			{/each}
		</Table.Body>
	</Table.Root>
</div>
<div class="grid gap-3 p-4 md:hidden">
	{#each items as item (item.curriculum.id)}
		<Button
			href={curriculumLink(item)}
			variant="outline"
			class="h-auto w-full justify-start p-4 text-start font-normal"
		>
			<div class="w-full space-y-3">
				<div class="flex items-start justify-between gap-3">
					<h3 class="whitespace-normal font-medium">{item.curriculum.nameTh}</h3>
					<ArrowUpRight class="size-4 shrink-0" />
				</div>
				<p class="whitespace-normal text-sm">
					{gradeLevelSummary(item.gradeLevels)} · {item.studyProgramCount} แผนการเรียน
				</p>
				<div class="flex flex-wrap gap-1.5">
					<Badge variant="secondary"
						>{item.displayState === 'published' ? 'เผยแพร่แล้ว' : 'ยังไม่เผยแพร่'}</Badge
					>{#if item.draftCount > 0}<Badge variant="outline">ร่าง {item.draftCount}</Badge>{/if}
				</div>
			</div>
		</Button>
	{/each}
</div>
