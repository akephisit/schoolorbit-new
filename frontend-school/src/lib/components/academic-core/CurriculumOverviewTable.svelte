<script lang="ts">
	import type { CurriculumDisplayState, CurriculumOverviewItem } from '$lib/api/academic-core';
	import { gradeLevelSummary } from '$lib/academic-core/catalog-presentation';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Table from '$lib/components/ui/table';
	import { ArrowUpRight, BookOpenCheck } from 'lucide-svelte';

	let { items }: { items: CurriculumOverviewItem[] } = $props();

	const stateLabels: Record<CurriculumDisplayState, string> = {
		current: 'ใช้อยู่',
		upcoming: 'เตรียมใช้',
		expired: 'สิ้นสุดแล้ว',
		unpublished: 'ยังไม่เผยแพร่'
	};

	const stateClasses: Record<CurriculumDisplayState, string> = {
		current: 'border-emerald-500/35 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300',
		upcoming: 'border-sky-500/35 bg-sky-500/10 text-sky-700 dark:text-sky-300',
		expired: 'border-muted-foreground/25 bg-muted text-muted-foreground',
		unpublished: 'border-amber-500/35 bg-amber-500/10 text-amber-700 dark:text-amber-300'
	};

	function effectiveYears(item: CurriculumOverviewItem) {
		if (!item.startAcademicYearName) return 'ยังไม่มีรุ่นที่เผยแพร่';
		if (!item.endAcademicYearName) return `ตั้งแต่ ${item.startAcademicYearName}`;
		return `${item.startAcademicYearName}–${item.endAcademicYearName}`;
	}
</script>

<div class="hidden overflow-x-auto md:block">
	<Table.Root>
		<Table.Header>
			<Table.Row>
				<Table.Head class="w-[145px] ps-5">รหัส</Table.Head>
				<Table.Head>ชื่อหลักสูตร</Table.Head>
				<Table.Head>ระดับชั้น</Table.Head>
				<Table.Head>รุ่นที่แสดง</Table.Head>
				<Table.Head>ปีที่มีผล</Table.Head>
				<Table.Head class="text-center">แผนการเรียน</Table.Head>
				<Table.Head>สถานะ</Table.Head>
				<Table.Head class="w-12"><span class="sr-only">เปิดหลักสูตร</span></Table.Head>
			</Table.Row>
		</Table.Header>
		<Table.Body>
			{#each items as item (item.curriculum.id)}
				<Table.Row>
					<Table.Cell class="border-s-4 border-s-primary ps-5 font-mono font-semibold">
						{item.curriculum.code}
					</Table.Cell>
					<Table.Cell class="max-w-[280px] whitespace-normal">
						<p class="font-medium">{item.curriculum.nameTh}</p>
						{#if item.curriculum.nameEn}
							<p class="text-xs text-muted-foreground">{item.curriculum.nameEn}</p>
						{/if}
					</Table.Cell>
					<Table.Cell class="max-w-[220px] whitespace-normal">
						{gradeLevelSummary(item.gradeLevels)}
					</Table.Cell>
					<Table.Cell>{item.displayVersion?.versionName ?? '—'}</Table.Cell>
					<Table.Cell>{effectiveYears(item)}</Table.Cell>
					<Table.Cell class="text-center font-mono tabular-nums">
						{item.studyProgramCount}
					</Table.Cell>
					<Table.Cell>
						<div class="flex flex-wrap gap-1.5">
							<Badge variant="outline" class={stateClasses[item.displayState]}>
								{stateLabels[item.displayState]}
							</Badge>
							{#if item.draftCount > 0}
								<Badge variant="secondary">ร่าง {item.draftCount}</Badge>
							{/if}
						</div>
					</Table.Cell>
					<Table.Cell>
						<Button
							href={`/staff/academic/curricula/${item.curriculum.id}`}
							variant="ghost"
							size="icon"
							aria-label={`เปิดหลักสูตร ${item.curriculum.nameTh}`}
						>
							<ArrowUpRight class="size-4" />
						</Button>
					</Table.Cell>
				</Table.Row>
			{/each}
		</Table.Body>
	</Table.Root>
</div>

<div class="grid gap-3 p-4 md:hidden">
	{#each items as item (item.curriculum.id)}
		<Button
			href={`/staff/academic/curricula/${item.curriculum.id}`}
			variant="outline"
			class="h-auto w-full justify-start rounded-xl border-s-4 border-s-primary bg-background p-4 text-start font-normal shadow-xs transition hover:bg-muted/40"
		>
			<div class="flex items-start justify-between gap-3">
				<div class="min-w-0">
					<p class="font-mono text-sm font-semibold text-primary">{item.curriculum.code}</p>
					<h2 class="mt-1 font-medium">{item.curriculum.nameTh}</h2>
				</div>
				<ArrowUpRight class="size-4 shrink-0 text-muted-foreground" />
			</div>
			<div class="mt-4 grid grid-cols-2 gap-3 text-sm">
				<div class="col-span-2">
					<p class="text-xs text-muted-foreground">ระดับชั้น</p>
					<p>{gradeLevelSummary(item.gradeLevels)}</p>
				</div>
				<div>
					<p class="text-xs text-muted-foreground">รุ่นที่แสดง</p>
					<p>{item.displayVersion?.versionName ?? '—'}</p>
				</div>
				<div>
					<p class="text-xs text-muted-foreground">ปีที่มีผล</p>
					<p>{effectiveYears(item)}</p>
				</div>
				<div class="col-span-2 flex items-center gap-2 rounded-lg bg-muted/45 px-3 py-2">
					<BookOpenCheck class="size-4 text-primary" />
					<span>แผนการเรียน {item.studyProgramCount} แผน</span>
				</div>
			</div>
			<div class="mt-3 flex flex-wrap gap-1.5">
				<Badge variant="outline" class={stateClasses[item.displayState]}>
					{stateLabels[item.displayState]}
				</Badge>
				{#if item.draftCount > 0}<Badge variant="secondary">ร่าง {item.draftCount}</Badge>{/if}
			</div>
		</Button>
	{/each}
</div>
