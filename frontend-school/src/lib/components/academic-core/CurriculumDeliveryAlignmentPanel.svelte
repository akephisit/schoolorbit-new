<script lang="ts">
	import type {
		CurriculumDeliveryAlignmentState,
		HomeroomDeliveryWorkspace
	} from '$lib/api/learning-delivery';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { ArrowLeft, BookCheck, ChevronDown, CircleAlert, Clock3 } from 'lucide-svelte';

	let {
		workspace,
		curriculumId,
		studyProgramId,
		academicYearId,
		academicTermId
	}: {
		workspace: HomeroomDeliveryWorkspace;
		curriculumId: string;
		studyProgramId?: string;
		academicYearId: string;
		academicTermId: string;
	} = $props();

	const alignmentLabels: Record<CurriculumDeliveryAlignmentState, string> = {
		matches_curriculum: 'ตรงกับหลักสูตร',
		curriculum_requirement_not_offered: 'หลักสูตรกำหนดไว้แต่ยังไม่เปิดสอน',
		extra_offering: 'เปิดสอนเพิ่มเติมนอกหลักสูตร',
		ended_early: 'หยุดสอนก่อนรุ่นตารางนี้มีผล',
		operational_periods_differ: 'คาบจริงต่างจากค่ามาตรฐานในหลักสูตร'
	};

	let rooms = $derived(
		workspace.homerooms.filter(
			(room) =>
				room.studyProgram.curriculumId === curriculumId &&
				(!studyProgramId || room.studyProgram.id === studyProgramId)
		)
	);
	let expectedItems = $derived(rooms.flatMap((room) => room.items));
	let extraItems = $derived(rooms.flatMap((room) => room.extraOfferings));
	let alignedCount = $derived(
		expectedItems.filter((item) => item.alignmentStates.includes('matches_curriculum')).length
	);
	let findingCount = $derived(
		expectedItems.filter((item) => !item.alignmentStates.includes('matches_curriculum')).length +
			extraItems.length
	);
	let backHref = $derived.by(() => {
		const query = new URLSearchParams({ academicYearId, academicTermId });
		if (workspace.timetableVersionId) {
			query.set('timetableVersionId', workspace.timetableVersionId);
		}
		return `/staff/academic/delivery?${query.toString()}`;
	});

	function badgeClass(state: CurriculumDeliveryAlignmentState): string {
		if (state === 'matches_curriculum') {
			return 'border-emerald-500/35 bg-emerald-500/[0.08] text-emerald-700 dark:text-emerald-300';
		}
		if (state === 'curriculum_requirement_not_offered' || state === 'ended_early') {
			return 'border-amber-500/40 bg-amber-500/[0.08] text-amber-800 dark:text-amber-200';
		}
		return 'border-sky-500/35 bg-sky-500/[0.07] text-sky-800 dark:text-sky-200';
	}

	function formatEffectiveDate(value: string | null): string {
		if (!value) return 'ยังไม่มีรุ่นตารางที่เลือก';
		return new Intl.DateTimeFormat('th-TH', { dateStyle: 'long' }).format(
			new Date(`${value}T00:00:00`)
		);
	}
</script>

<section class="overflow-hidden rounded-2xl border border-primary/20 bg-card shadow-sm">
	<header class="border-b bg-primary/[0.045] p-4 sm:p-5">
		<div class="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
			<div class="flex min-w-0 items-start gap-3">
				<div class="rounded-xl bg-primary/10 p-2.5 text-primary"><BookCheck class="size-5" /></div>
				<div>
					<p class="text-xs font-semibold uppercase tracking-[0.16em] text-primary">
						บริบทการเปิดสอน
					</p>
					<h2 class="mt-1 text-lg font-semibold">เทียบการเปิดสอนกับหลักสูตร</h2>
					<p class="mt-1 text-sm text-muted-foreground">
						ตรวจตามห้องและแผนการเรียน ณ รุ่นตารางที่มีผล
						{formatEffectiveDate(workspace.timetableVersionEffectiveFrom)}
					</p>
				</div>
			</div>
			<Button href={backHref} variant="outline">
				<ArrowLeft class="size-4" /> กลับไปจัดการการเปิดสอน
			</Button>
		</div>
	</header>

	<div class="grid gap-px border-b bg-border sm:grid-cols-3">
		<div class="bg-background p-4">
			<p class="text-xs font-medium text-muted-foreground">ห้องในบริบทนี้</p>
			<p class="mt-1 text-2xl font-semibold tabular-nums">{rooms.length}</p>
		</div>
		<div class="bg-background p-4">
			<p class="text-xs font-medium text-muted-foreground">ตรงกับหลักสูตร</p>
			<p class="mt-1 text-2xl font-semibold tabular-nums text-emerald-700 dark:text-emerald-300">
				{alignedCount}/{expectedItems.length}
			</p>
		</div>
		<div class="bg-background p-4">
			<p class="text-xs font-medium text-muted-foreground">รายการที่ควรพิจารณา</p>
			<p class="mt-1 text-2xl font-semibold tabular-nums text-amber-700 dark:text-amber-300">
				{findingCount}
			</p>
		</div>
	</div>

	{#if rooms.length === 0}
		<div class="p-8 text-center">
			<CircleAlert class="mx-auto size-6 text-muted-foreground" />
			<p class="mt-2 font-medium">ไม่พบห้องของหลักสูตรหรือแผนการเรียนนี้ในบริบทที่เลือก</p>
			<p class="mt-1 text-sm text-muted-foreground">
				ตรวจปีการศึกษา ภาคเรียน และแผนการเรียนจากหน้าการเปิดสอนอีกครั้ง
			</p>
		</div>
	{:else}
		<div class="divide-y">
			{#each rooms as room, index (room.homeroom.id)}
				<details open={index === 0} class="group">
					<summary
						class="flex cursor-pointer list-none items-center justify-between gap-3 p-4 hover:bg-muted/25 [&::-webkit-details-marker]:hidden"
					>
						<div>
							<div class="flex flex-wrap items-center gap-2">
								<h3 class="font-semibold">{room.homeroom.name}</h3>
								<Badge variant="outline">{room.gradeLevel.short_name ?? room.gradeLevel.name}</Badge>
							</div>
							<p class="mt-1 text-sm text-muted-foreground">{room.studyProgram.name}</p>
						</div>
						<div class="flex items-center gap-2">
							<span class="font-mono text-sm tabular-nums">
								{room.items.length} ตามหลักสูตร · {room.extraOfferings.length} เพิ่มเติม
							</span>
							<ChevronDown class="size-4 transition-transform group-open:rotate-180" />
						</div>
					</summary>

					<div class="space-y-3 border-t bg-muted/[0.12] p-4">
						{#each room.items as item (item.requirementId)}
							<div class="rounded-xl border bg-background p-3">
								<div class="flex flex-col gap-2 md:flex-row md:items-start md:justify-between">
									<div>
										<p class="font-mono text-xs font-semibold text-primary">{item.code}</p>
										<p class="font-medium">{item.name}</p>
										{#if item.resourceKind === 'course'}
											<p class="mt-1 flex items-center gap-1 text-xs text-muted-foreground">
												<Clock3 class="size-3.5" /> ตามหลักสูตร
												{item.standardPeriodsPerWeek ?? '—'} · จัดจริง
												{item.weeklyPeriodTarget ?? '—'} คาบ/สัปดาห์
											</p>
										{/if}
									</div>
									<div class="flex max-w-xl flex-wrap gap-1.5 md:justify-end">
										{#each item.alignmentStates as state (state)}
											<Badge variant="outline" class={badgeClass(state)}>
												{alignmentLabels[state]}
											</Badge>
										{/each}
									</div>
								</div>
							</div>
						{/each}

						{#each room.extraOfferings as item (item.offeringId)}
							<div class="rounded-xl border border-sky-500/25 bg-sky-500/[0.045] p-3">
								<div class="flex flex-col gap-2 md:flex-row md:items-start md:justify-between">
									<div>
										<p class="font-mono text-xs font-semibold text-sky-700 dark:text-sky-300">
											{item.code}
										</p>
										<p class="font-medium">{item.name}</p>
									</div>
									<div class="flex flex-wrap gap-1.5 md:justify-end">
										{#each item.alignmentStates as state (state)}
											<Badge variant="outline" class={badgeClass(state)}>
												{alignmentLabels[state]}
											</Badge>
										{/each}
									</div>
								</div>
							</div>
						{/each}
					</div>
				</details>
			{/each}
		</div>
	{/if}
</section>
