<script lang="ts">
	import type {
		WholeSchoolTimetableIssue,
		WholeSchoolTimetableLesson,
		WholeSchoolTimetableOverview
	} from '$lib/api/timetable';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Select from '$lib/components/ui/select';
	import { AlertTriangle, DoorOpen, LoaderCircle, Users } from 'lucide-svelte';

	import TimetableIssueSummary from './TimetableIssueSummary.svelte';

	let {
		overview,
		selectedDay,
		loading = false,
		errorMessage = '',
		onDayChange,
		onRetry,
		onOpenHomeroom,
		onOpenTeacher
	}: {
		overview: WholeSchoolTimetableOverview | null;
		selectedDay: string;
		loading?: boolean;
		errorMessage?: string;
		onDayChange: (day: string) => void;
		onRetry: () => void;
		onOpenHomeroom: (homeroomId: string, periodId: string | null) => void;
		onOpenTeacher: (teacherId: string, periodId: string | null) => void;
	} = $props();

	const days = [
		{ id: 'MON', label: 'จันทร์', shortLabel: 'จ.' },
		{ id: 'TUE', label: 'อังคาร', shortLabel: 'อ.' },
		{ id: 'WED', label: 'พุธ', shortLabel: 'พ.' },
		{ id: 'THU', label: 'พฤหัสบดี', shortLabel: 'พฤ.' },
		{ id: 'FRI', label: 'ศุกร์', shortLabel: 'ศ.' }
	];
	let selectedLesson = $state.raw<WholeSchoolTimetableLesson | null>(null);
	let detailOpen = $state(false);
	let selectedMobilePeriodId = $derived(overview?.periods[0]?.id ?? '');
	let selectedMobileHomeroomId = $derived(overview?.rows[0]?.homeroomId ?? '');
	const selectedMobileRow = $derived(
		overview?.rows.find((row) => row.homeroomId === selectedMobileHomeroomId) ?? null
	);
	const selectedMobilePeriod = $derived(
		overview?.periods.find((period) => period.id === selectedMobilePeriodId) ?? null
	);
	const selectedMobileCell = $derived(
		selectedMobileRow?.cells.find((cell) => cell.bellSchedulePeriodId === selectedMobilePeriodId) ??
			null
	);

	function cellFor(
		row: WholeSchoolTimetableOverview['rows'][number],
		periodId: string
	): WholeSchoolTimetableOverview['rows'][number]['cells'][number] | null {
		return row.cells.find((cell) => cell.bellSchedulePeriodId === periodId) ?? null;
	}

	function periodLabel(period: WholeSchoolTimetableOverview['periods'][number]): string {
		return period.name ?? `คาบที่ ${period.orderIndex}`;
	}

	function teacherInitials(lesson: WholeSchoolTimetableLesson): string {
		return lesson.instructors
			.map((teacher) => teacher.displayName.replace(/^ครู/, '').trim().slice(0, 2))
			.filter(Boolean)
			.join('+');
	}

	function openLesson(lesson: WholeSchoolTimetableLesson): void {
		selectedLesson = lesson;
		detailOpen = true;
	}

	function homeroomForIssue(issue: WholeSchoolTimetableIssue): string | null {
		if (!overview) return null;
		return (
			overview.rows.find((row) =>
				row.cells.some((cell) =>
					cell.lessons.some((lesson) => issue.entryIds.includes(lesson.entryId))
				)
			)?.homeroomId ?? null
		);
	}

	function focusIssue(issue: WholeSchoolTimetableIssue): void {
		const homeroomId = issue.homeroomIds[0] ?? homeroomForIssue(issue);
		if (homeroomId) selectedMobileHomeroomId = homeroomId;
		if (issue.bellSchedulePeriodId) selectedMobilePeriodId = issue.bellSchedulePeriodId;
		const targetId = `${homeroomId ?? ''}:${issue.bellSchedulePeriodId ?? ''}`;
		document.querySelector<HTMLElement>(`[data-overview-cell="${targetId}"]`)?.scrollIntoView({
			block: 'center',
			inline: 'center',
			behavior: 'smooth'
		});
	}
</script>

<section class="space-y-4" aria-label="ภาพรวมตารางสอนทั้งโรงเรียน">
	<header class="overflow-hidden rounded-xl border bg-background shadow-sm">
		<div class="h-1 bg-primary"></div>
		<div class="flex flex-col gap-4 p-4 lg:flex-row lg:items-center lg:justify-between">
			<div>
				<div class="flex flex-wrap items-center gap-2">
					<h2 class="text-lg font-semibold tracking-tight">ภาพรวมทั้งโรงเรียน · ดูอย่างเดียว</h2>
					<Badge variant="outline">ข้อมูลรายวัน</Badge>
				</div>
				<p class="mt-1 text-sm text-muted-foreground">
					สมุดตรวจตารางรวม แสดงห้อง ครู และห้องเรียนเฉพาะจากรุ่นที่เลือก
				</p>
			</div>
			<div class="inline-flex w-fit rounded-lg border bg-muted/45 p-1" aria-label="เลือกวัน">
				{#each days as day (day.id)}
					<Button
						type="button"
						size="sm"
						variant={selectedDay === day.id ? 'default' : 'ghost'}
						class="h-8 min-w-9 px-2 sm:px-3"
						aria-pressed={selectedDay === day.id}
						disabled={loading}
						onclick={() => onDayChange(day.id)}
					>
						<span class="sm:hidden">{day.shortLabel}</span>
						<span class="hidden sm:inline">{day.label}</span>
					</Button>
				{/each}
			</div>
		</div>
	</header>

	{#if loading && !overview}
		<div class="flex min-h-64 items-center justify-center rounded-xl border bg-background">
			<LoaderCircle class="size-6 animate-spin text-primary" />
			<span class="ms-2 text-sm text-muted-foreground">กำลังโหลดภาพรวมของวันที่เลือก</span>
		</div>
	{:else if errorMessage && !overview}
		<div class="rounded-xl border border-destructive/30 bg-destructive/5 p-5">
			<div class="flex items-start gap-3">
				<AlertTriangle class="mt-0.5 size-5 text-destructive" />
				<div class="flex-1">
					<h2 class="font-semibold">โหลดภาพรวมไม่สำเร็จ</h2>
					<p class="mt-1 text-sm text-muted-foreground">{errorMessage}</p>
					<Button class="mt-3" variant="outline" onclick={onRetry}>ลองอีกครั้ง</Button>
				</div>
			</div>
		</div>
	{:else if overview}
		<div class="grid gap-4 2xl:grid-cols-[minmax(0,1fr)_22rem]">
			<div class="min-w-0 space-y-3">
				<div class="flex flex-wrap gap-2">
					<Badge variant="secondary">{overview.summary.homeroomCount} ห้อง</Badge>
					<Badge variant="secondary">{overview.summary.uniqueLessonCount} คาบ</Badge>
					<Badge variant={overview.summary.blockingIssueCount > 0 ? 'destructive' : 'outline'}>
						{overview.summary.blockingIssueCount} จุดบล็อก
					</Badge>
					<Badge variant="outline">{overview.summary.warningIssueCount} คำเตือน</Badge>
				</div>

				<div class="sm:hidden">
					<div class="grid gap-2 rounded-xl border bg-background p-3">
						<Select.Root type="single" bind:value={selectedMobileHomeroomId}>
							<Select.Trigger class="w-full" aria-label="เลือกห้องประจำชั้น">
								{selectedMobileRow?.homeroomName ?? 'เลือกห้องประจำชั้น'}
							</Select.Trigger>
							<Select.Content>
								{#each overview.rows as row (row.homeroomId)}
									<Select.Item value={row.homeroomId}>{row.homeroomName}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
						<Select.Root type="single" bind:value={selectedMobilePeriodId}>
							<Select.Trigger class="w-full" aria-label="เลือกคาบ">
								{selectedMobilePeriod ? periodLabel(selectedMobilePeriod) : 'เลือกคาบ'}
							</Select.Trigger>
							<Select.Content>
								{#each overview.periods as period (period.id)}
									<Select.Item value={period.id}>{periodLabel(period)}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
						<div class="min-h-28 rounded-lg border bg-muted/10 p-2">
							{#if selectedMobileCell?.lessons.length}
								{#each selectedMobileCell.lessons as lesson (lesson.entryId)}
									<button
										type="button"
										class="mb-2 w-full rounded-md border border-primary/20 bg-primary/5 p-2 text-left"
										onclick={() => openLesson(lesson)}
									>
										<p class="font-mono text-xs font-semibold text-primary">
											{lesson.offeringCode ?? lesson.entryType}
										</p>
										<p class="text-xs">{lesson.offeringName ?? lesson.title}</p>
									</button>
								{/each}
							{:else}
								<p class="py-8 text-center text-xs text-muted-foreground">ว่าง</p>
							{/if}
						</div>
					</div>
				</div>

				<div class="hidden max-h-[68vh] overflow-auto rounded-xl border bg-background sm:block">
					<table class="w-full border-collapse text-left text-xs">
						<thead>
							<tr>
								<th
									class="sticky top-0 left-0 z-30 min-w-32 border-r border-b bg-muted px-3 py-2.5"
								>
									ห้อง
								</th>
								{#each overview.periods as period (period.id)}
									<th
										class="sticky top-0 z-20 min-w-40 border-r border-b bg-muted px-3 py-2.5 text-center"
									>
										<p>{periodLabel(period)}</p>
										<p class="font-mono text-[0.65rem] font-normal text-muted-foreground">
											{period.startTime.slice(0, 5)}–{period.endTime.slice(0, 5)}
										</p>
									</th>
								{/each}
							</tr>
						</thead>
						<tbody>
							{#each overview.rows as row (row.homeroomId)}
								<tr>
									<th
										class="sticky left-0 z-10 border-r border-b bg-background px-3 py-3 align-top"
									>
										<p class="font-semibold">{row.homeroomName}</p>
										<p class="font-mono text-[0.65rem] text-muted-foreground">{row.homeroomCode}</p>
									</th>
									{#each overview.periods as period (period.id)}
										{@const cell = cellFor(row, period.id)}
										<td
											class="border-r border-b p-1.5 align-top"
											data-overview-cell={`${row.homeroomId}:${period.id}`}
										>
											{#if cell?.lessons.length}
												<div class="space-y-1.5">
													{#each cell.lessons as lesson (lesson.entryId)}
														<button
															type="button"
															class="w-full rounded-md border border-primary/20 bg-primary/5 p-2 text-left transition hover:border-primary/45 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary"
															onclick={() => openLesson(lesson)}
														>
															<div class="flex items-start justify-between gap-1">
																<span class="font-mono font-semibold text-primary">
																	{lesson.offeringCode ?? lesson.entryType}
																</span>
																{#if lesson.isSharedGroup}<Badge variant="outline">รวม</Badge>{/if}
															</div>
															<p class="mt-1 truncate text-[0.68rem]">
																{teacherInitials(lesson) || 'ยังไม่ระบุครู'} · {lesson.roomCode ??
																	'ห้องประจำชั้น'}
															</p>
														</button>
													{/each}
												</div>
											{:else}
												<p class="py-4 text-center text-muted-foreground/50">—</p>
											{/if}
										</td>
									{/each}
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			</div>

			<TimetableIssueSummary
				issues={overview.issues}
				{homeroomForIssue}
				onFocusIssue={focusIssue}
				{onOpenHomeroom}
				{onOpenTeacher}
			/>
		</div>
	{/if}
</section>

<Dialog.Root bind:open={detailOpen}>
	<Dialog.Content class="sm:max-w-lg">
		<Dialog.Header>
			<Dialog.Title
				>{selectedLesson?.offeringName ?? selectedLesson?.title ?? 'รายละเอียดคาบ'}</Dialog.Title
			>
			<Dialog.Description>
				{selectedLesson?.offeringCode ?? selectedLesson?.entryType ?? ''} · ดูอย่างเดียว
			</Dialog.Description>
		</Dialog.Header>
		{#if selectedLesson}
			<div class="space-y-3 text-sm">
				<p class="flex items-start gap-2">
					<Users class="mt-0.5 size-4 text-muted-foreground" />
					<span
						>{selectedLesson.instructors.map((teacher) => teacher.displayName).join(', ') ||
							'ยังไม่ระบุครู'}</span
					>
				</p>
				<p class="flex items-start gap-2">
					<DoorOpen class="mt-0.5 size-4 text-muted-foreground" />
					<span>{selectedLesson.roomCode ?? 'ใช้ห้องประจำชั้น'}</span>
				</p>
				{#if selectedLesson.learningGroupName}
					<p class="rounded-lg bg-muted/40 p-3 text-xs text-muted-foreground">
						{selectedLesson.learningGroupCode} · {selectedLesson.learningGroupName}
					</p>
				{/if}
			</div>
		{/if}
		<Dialog.Footer
			><Button variant="outline" onclick={() => (detailOpen = false)}>ปิด</Button></Dialog.Footer
		>
	</Dialog.Content>
</Dialog.Root>
