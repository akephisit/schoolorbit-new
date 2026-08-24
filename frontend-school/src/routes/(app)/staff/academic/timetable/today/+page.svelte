<script lang="ts">
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';
	import { getAcademicContextStore } from '$lib/academic-context/store';
	import {
		getDailyTeachingOverview,
		type DailyTeachingEntry,
		type DailyTeachingOverview,
		type DailyTeachingPeriod,
		type DailyTeachingTeacher
	} from '$lib/api/timetable';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { DatePicker } from '$lib/components/ui/date-picker';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Switch } from '$lib/components/ui/switch';
	import * as Table from '$lib/components/ui/table';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import {
		DAILY_TEACHING_MIN_PERIOD_COLUMN_WIDTH,
		DAILY_TEACHING_TEACHER_COLUMN_WIDTH,
		dailyTeachingEntryCardPresentation,
		dailyTeachingTableMinWidth,
		groupDailyTeachingEntries
	} from '$lib/utils/daily-teaching-display';
	import {
		CalendarClock,
		ChevronLeft,
		ChevronRight,
		ExternalLink,
		MapPin,
		Printer,
		RefreshCw,
		Search
	} from 'lucide-svelte';

	type SelectedCell = {
		teacher: DailyTeachingTeacher;
		period: DailyTeachingPeriod;
		entries: DailyTeachingEntry[];
	};

	const academicContext = getAcademicContextStore();
	const academicTermId = $derived($academicContext.selected.academicTermId);
	let overview = $state<DailyTeachingOverview | null>(null);
	let selectedDate = $state(toDateInputValue(new Date()));
	let includeEmptyTeachers = $state(false);
	let teacherSearch = $state('');
	let loading = $state(false);
	let errorMessage = $state('');
	let selectedCell = $state<SelectedCell | null>(null);
	let cellDialogOpen = $state(false);
	let requestRevision = 0;

	const canReadDailyTeaching = $derived(
		$can.hasAny(
			PERMISSIONS.ACADEMIC_TIMETABLE_TODAY_READ_SCHOOL,
			PERMISSIONS.LEARNING_OFFERING_READ_SCHOOL,
			PERMISSIONS.LEARNING_OFFERING_READ_ORGANIZATION_TREE,
			PERMISSIONS.LEARNING_OFFERING_READ_ORGANIZATION_UNIT,
			PERMISSIONS.LEARNING_OFFERING_READ_ASSIGNED,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_SCHOOL,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_ORGANIZATION_TREE,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_ORGANIZATION_UNIT,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_ASSIGNED
		)
	);
	const canOpenPlanner = $derived(
		$can.hasAny(
			PERMISSIONS.LEARNING_OFFERING_MANAGE_SCHOOL,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_ORGANIZATION_TREE,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_ORGANIZATION_UNIT,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_ASSIGNED
		)
	);
	const filteredTeachers = $derived.by(() => {
		const query = teacherSearch.trim().toLocaleLowerCase('th');
		if (!query) return overview?.teachers ?? [];
		return (overview?.teachers ?? []).filter((teacher) => {
			if (teacher.displayName.toLocaleLowerCase('th').includes(query)) return true;
			return teacher.periods.some((cell) =>
				cell.entries.some((entry) =>
					[
						entry.offeringCode,
						entry.offeringName,
						entry.learningGroupName,
						entry.subjectVersionDisplayLabel,
						entry.activityVersionDisplayLabel,
						...entry.homeroomNames
					]
						.filter(Boolean)
						.some((value) => value?.toLocaleLowerCase('th').includes(query))
				)
			);
		});
	});
	const tableMinWidth = $derived(dailyTeachingTableMinWidth(overview?.periods.length ?? 4));

	async function loadOverview(termId = academicTermId): Promise<void> {
		if (!termId || !canReadDailyTeaching) return;
		const current = ++requestRevision;
		loading = true;
		errorMessage = '';
		try {
			const loaded = await getDailyTeachingOverview({
				academicTermId: termId,
				date: selectedDate,
				includeEmptyTeachers
			});
			if (current === requestRevision) overview = loaded;
		} catch (error) {
			if (current === requestRevision) {
				errorMessage = error instanceof Error ? error.message : 'โหลดตารางสอนรายวันไม่สำเร็จ';
				toast.error(errorMessage);
			}
		} finally {
			if (current === requestRevision) loading = false;
		}
	}

	function handleDateChange(value: string | undefined): void {
		if (!value) return;
		selectedDate = value;
		void loadOverview();
	}

	function moveDate(offset: number): void {
		const next = parseDateInput(selectedDate);
		next.setDate(next.getDate() + offset);
		selectedDate = toDateInputValue(next);
		void loadOverview();
	}

	function toggleEmptyTeachers(): void {
		includeEmptyTeachers = !includeEmptyTeachers;
		void loadOverview();
	}

	function cellForPeriod(teacher: DailyTeachingTeacher, periodId: string) {
		return (
			teacher.periods.find((cell) => cell.bellSchedulePeriodId === periodId) ?? {
				bellSchedulePeriodId: periodId,
				entries: []
			}
		);
	}

	function entryTitle(entry: DailyTeachingEntry): string {
		return (
			entry.offeringCode ??
			entry.offeringName ??
			entry.title ??
			entry.subjectVersionDisplayLabel ??
			entry.activityVersionDisplayLabel ??
			entry.entryType
		);
	}

	function entrySubtitle(entry: DailyTeachingEntry): string {
		return [
			entry.learningGroupName,
			entry.homeroomNames.join(', '),
			entry.roomCode ? `ห้อง ${entry.roomCode}` : null
		]
			.filter(Boolean)
			.join(' · ');
	}

	function openCell(
		teacher: DailyTeachingTeacher,
		period: DailyTeachingPeriod,
		entries: DailyTeachingEntry[]
	): void {
		selectedCell = { teacher, period, entries };
		cellDialogOpen = true;
	}

	function formatDate(value: string): string {
		return parseDateInput(value).toLocaleDateString('th-TH', {
			weekday: 'long',
			day: 'numeric',
			month: 'long',
			year: 'numeric'
		});
	}

	function parseDateInput(value: string): Date {
		const [year, month, day] = value.split('-').map(Number);
		return new Date(year, month - 1, day);
	}

	function toDateInputValue(date: Date): string {
		const year = date.getFullYear();
		const month = String(date.getMonth() + 1).padStart(2, '0');
		const day = String(date.getDate()).padStart(2, '0');
		return `${year}-${month}-${day}`;
	}

	onMount(() => {
		let loadedTermId: string | null = null;
		return academicContext.subscribe((state) => {
			const termId = state.selected.academicTermId;
			if (termId && termId !== loadedTermId) {
				loadedTermId = termId;
				void loadOverview(termId);
			}
		});
	});
</script>

<PageShell
	title="ตารางสอนวันนี้"
	description="ภาพรวมคาบของครูทั้งโรงเรียนในภาคเรียนที่เลือก โดยใช้กลุ่มเรียนและห้องประจำชั้นจากระบบใหม่"
>
	<div
		class="print:hidden flex flex-col gap-3 rounded-xl border bg-card p-4 xl:flex-row xl:items-end xl:justify-between"
	>
		<div class="flex flex-wrap items-end gap-3">
			<div class="space-y-2">
				<Label for="teaching-date">วันที่</Label>
				<div class="flex gap-2">
					<Button
						variant="outline"
						size="icon"
						aria-label="วันก่อนหน้า"
						onclick={() => moveDate(-1)}><ChevronLeft /></Button
					>
					<DatePicker
						id="teaching-date"
						value={selectedDate}
						onValueChange={handleDateChange}
						class="min-w-48"
					/>
					<Button variant="outline" size="icon" aria-label="วันถัดไป" onclick={() => moveDate(1)}
						><ChevronRight /></Button
					>
				</div>
			</div>
			<div class="min-w-64 space-y-2">
				<Label for="teacher-search">ค้นหาครู วิชา กลุ่มเรียน หรือห้อง</Label>
				<div class="relative">
					<Search class="text-muted-foreground absolute top-2.5 left-3 size-4" /><Input
						id="teacher-search"
						bind:value={teacherSearch}
						class="pl-9"
					/>
				</div>
			</div>
			<div class="flex h-9 items-center gap-3 rounded-md border px-3">
				<Switch id="include-empty" checked={includeEmptyTeachers} onclick={toggleEmptyTeachers} />
				<Label for="include-empty">รวมครูที่ไม่มีคาบ</Label>
			</div>
		</div>
		<div class="flex flex-wrap gap-2">
			<Button variant="outline" disabled={loading || !academicTermId} onclick={() => loadOverview()}
				><RefreshCw class={loading ? 'animate-spin' : ''} /> รีเฟรช</Button
			>
			{#if canOpenPlanner}<Button variant="outline" href="/staff/academic/timetable"
					><ExternalLink /> จัดตาราง</Button
				>{/if}
			<Button variant="outline" disabled={!overview} onclick={() => window.print()}
				><Printer /> พิมพ์</Button
			>
		</div>
	</div>

	{#if !canReadDailyTeaching}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ดูตารางสอนวันนี้"
			description="บัญชีนี้ยังไม่ได้รับสิทธิ์ดูภาพรวมตารางสอนรายวันของโรงเรียน"
		/>
	{:else if !academicTermId}
		<PageState
			variant="empty"
			title="เลือกภาคเรียนก่อน"
			description="ใช้ตัวเลือกปีการศึกษาและภาคเรียนบนแถบด้านบน"
		/>
	{:else if loading && !overview}
		<PageSkeleton variant="table" rows={8} columns={6} />
	{:else if errorMessage}
		<PageState
			variant="error"
			title="โหลดตารางสอนไม่สำเร็จ"
			description={errorMessage}
			actionLabel="ลองใหม่"
			onaction={() => loadOverview()}
		/>
	{:else if !overview}
		<PageState
			title="ยังไม่มีข้อมูลตารางสอน"
			description="ภาคเรียนหรือวันที่นี้ยังไม่มีคาบที่แสดงได้"
		/>
	{:else}
		<div class="grid gap-3 md:grid-cols-2 xl:grid-cols-5">
			{#each [['ครูทั้งหมด', overview.summary.totalTeacherCount], ['ครูที่แสดง', filteredTeachers.length], ['ครูที่มีคาบ', overview.summary.teachersTeachingCount], ['จำนวนคาบสอน', overview.summary.lessonCount], ['ไม่มีคาบวันนี้', overview.summary.emptyTeacherCount]] as item (item[0])}
				<div class="rounded-md border bg-background p-3">
					<p class="text-muted-foreground text-xs">{item[0]}</p>
					<p class="text-2xl font-semibold">{item[1]}</p>
				</div>
			{/each}
		</div>

		<section class="overflow-hidden rounded-md border bg-background">
			<div class="flex items-center justify-between gap-3 border-b p-4">
				<div>
					<h2 class="flex items-center gap-2 text-lg font-semibold">
						<CalendarClock class="size-5" />
						{formatDate(overview.date)}
					</h2>
					<p class="text-muted-foreground text-sm">
						{overview.periods.length} คาบ · {overview.summary.displayedTeacherCount} ครู
					</p>
				</div>
				{#if loading}<Badge variant="secondary">กำลังอัปเดต</Badge>{/if}
			</div>
			{#if filteredTeachers.length === 0}
				<PageState
					title="ไม่พบครูตามคำค้น"
					description="ลองเปลี่ยนคำค้นหาหรือเปิดรวมครูที่ไม่มีคาบ"
					class="m-4"
				/>
			{:else}
				<div class="max-h-[70vh] overflow-auto">
					<Table.Root
						style={`--teacher-column-width: ${DAILY_TEACHING_TEACHER_COLUMN_WIDTH}px; --minimum-period-column-width: ${DAILY_TEACHING_MIN_PERIOD_COLUMN_WIDTH}px; min-width: ${tableMinWidth}px;`}
					>
						<Table.Header class="sticky top-0 z-40"
							><Table.Row class="bg-muted/80 hover:bg-muted/80"
								><Table.Head class="sticky left-0 z-50 bg-muted">ครู</Table.Head
								>{#each overview.periods as period (period.id)}<Table.Head
										class="bg-muted text-center"
										><p>{period.name ?? `คาบ ${period.orderIndex}`}</p>
										<p class="text-muted-foreground text-xs font-normal">
											{period.startTime.slice(0, 5)}–{period.endTime.slice(0, 5)}
										</p></Table.Head
									>{/each}</Table.Row
							></Table.Header
						>
						<Table.Body>
							{#each filteredTeachers as teacher (teacher.id)}
								<Table.Row>
									<Table.Cell class="sticky left-0 z-20 bg-background font-medium"
										>{teacher.displayName}</Table.Cell
									>
									{#each overview.periods as period (period.id)}
										{@const cell = cellForPeriod(teacher, period.id)}
										<Table.Cell class="p-1 align-top">
											<button
												type="button"
												class="hover:bg-muted/30 min-h-20 w-full rounded-md p-1 text-left"
												onclick={() => openCell(teacher, period, cell.entries)}
											>
												{#each groupDailyTeachingEntries(cell.entries) as group (group.key)}
													{@const entry = group.entries[0]}
													{@const presentation = dailyTeachingEntryCardPresentation(entry)}
													<div
														class="mb-1 overflow-hidden rounded-md border p-2 {presentation.tone ===
														'course'
															? 'border-blue-200 bg-blue-50 dark:border-blue-800 dark:bg-blue-950/40'
															: presentation.tone === 'activity'
																? 'border-emerald-200 bg-emerald-50 dark:border-emerald-800 dark:bg-emerald-950/40'
																: 'border-amber-200 bg-amber-50 dark:border-amber-800 dark:bg-amber-950/40'}"
													>
														<p class="truncate text-xs font-semibold">{entryTitle(entry)}</p>
														<p class="text-muted-foreground mt-1 truncate text-[10px]">
															{entrySubtitle(entry)}
														</p>
													</div>
												{/each}
											</button>
										</Table.Cell>
									{/each}
								</Table.Row>
							{/each}
						</Table.Body>
					</Table.Root>
				</div>
			{/if}
		</section>
	{/if}
</PageShell>

<Dialog.Root bind:open={cellDialogOpen}>
	<Dialog.Content class="max-h-[85vh] overflow-y-auto sm:max-w-2xl">
		<Dialog.Header
			><Dialog.Title>{selectedCell?.teacher.displayName ?? 'รายละเอียดคาบ'}</Dialog.Title
			><Dialog.Description
				>{selectedCell
					? `${selectedCell.period.name ?? `คาบ ${selectedCell.period.orderIndex}`} · ${selectedCell.period.startTime.slice(0, 5)}–${selectedCell.period.endTime.slice(0, 5)}`
					: ''}</Dialog.Description
			></Dialog.Header
		>
		<div class="space-y-3 py-2">
			{#if selectedCell?.entries.length === 0}<p class="text-muted-foreground text-sm">
					ไม่มีคาบสอนในช่วงเวลานี้
				</p>{/if}
			{#each selectedCell?.entries ?? [] as entry (entry.entryId)}
				<div class="rounded-lg border p-4">
					<div class="flex items-center justify-between gap-3">
						<p class="font-medium">{entryTitle(entry)}</p>
						<Badge variant="outline">{entry.entryType}</Badge>
					</div>
					<p class="text-muted-foreground mt-2 text-sm">
						{entrySubtitle(entry) || 'ไม่มีข้อมูลห้องหรือกลุ่มเรียน'}
					</p>
					{#if entry.note}<p class="mt-2 text-sm">{entry.note}</p>{/if}{#if entry.roomCode}<p
							class="mt-2 flex items-center gap-1 text-sm"
						>
							<MapPin class="size-4" />
							{entry.roomCode}
						</p>{/if}
				</div>
			{/each}
		</div>
	</Dialog.Content>
</Dialog.Root>
