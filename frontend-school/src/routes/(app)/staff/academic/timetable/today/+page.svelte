<script lang="ts">
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import { Switch } from '$lib/components/ui/switch';
	import * as Table from '$lib/components/ui/table';
	import { getAcademicStructure, type AcademicYear, type Semester } from '$lib/api/academic';
	import {
		getDailyTeachingOverview,
		type DailyTeachingEntry,
		type DailyTeachingOverview,
		type DailyTeachingPeriod,
		type DailyTeachingTeacher
	} from '$lib/api/timetable';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import {
		CalendarClock,
		ChevronLeft,
		ChevronRight,
		ExternalLink,
		Printer,
		RefreshCw,
		Search
	} from 'lucide-svelte';

	let { data } = $props();

	type FilterOption = {
		value: string;
		label: string;
	};

	type BadgeVariant = 'default' | 'secondary' | 'outline' | 'destructive';

	type SelectedCell = {
		teacher: DailyTeachingTeacher;
		period: DailyTeachingPeriod;
		entries: DailyTeachingEntry[];
	};

	let initialized = $state(false);
	let loading = $state(false);
	let loadingStructure = $state(false);
	let structureLoaded = $state(false);
	let overview = $state<DailyTeachingOverview | null>(null);
	let errorMessage = $state('');
	let academicYears = $state<AcademicYear[]>([]);
	let semesters = $state<Semester[]>([]);
	let selectedDate = $state(toDateInputValue(new Date()));
	let selectedYearId = $state('');
	let selectedSemesterId = $state('');
	let includeEmptyTeachers = $state(false);
	let teacherSearch = $state('');
	let selectedSubjectGroup = $state('all');
	let selectedSubject = $state('all');
	let selectedClassroom = $state('all');
	let cellDialogOpen = $state(false);
	let selectedCell = $state<SelectedCell | null>(null);
	let lastLoadedKey = '';
	let overviewRequestSeq = 0;

	const canReadDailyTeaching = $derived(
		$can.hasAny(
			PERMISSIONS.ACADEMIC_TIMETABLE_TODAY_READ_SCHOOL,
			PERMISSIONS.ACADEMIC_COURSE_PLAN_READ_ALL
		)
	);
	const canUseAcademicFilters = $derived($can.has(PERMISSIONS.ACADEMIC_COURSE_PLAN_READ_ALL));
	const canOpenPlanner = $derived($can.has(PERMISSIONS.ACADEMIC_COURSE_PLAN_MANAGE_ALL));

	const semesterOptions = $derived(
		semesters.filter((semester) => !selectedYearId || semester.academic_year_id === selectedYearId)
	);
	const subjectGroupOptions = $derived.by(() => {
		const values = new Set<string>();
		for (const teacher of overview?.teachers ?? []) {
			for (const subjectGroupName of teacher.subjectGroupNames) {
				if (subjectGroupName) values.add(subjectGroupName);
			}
		}
		return toSortedOptions(values);
	});
	const subjectOptions = $derived.by(() => {
		const options = new Map<string, string>();
		forEachOverviewEntry((entry) => {
			const value = entrySubjectValue(entry);
			if (!value) return;
			options.set(value, entrySubjectLabel(entry));
		});
		return toSortedOptions(options);
	});
	const classroomOptions = $derived.by(() => {
		const values = new Set<string>();
		forEachOverviewEntry((entry) => {
			if (entry.classroomName) values.add(entry.classroomName);
		});
		return toSortedOptions(values);
	});
	const filteredTeachers = $derived.by(() => {
		const query = teacherSearch.trim().toLowerCase();
		return (overview?.teachers ?? []).filter((teacher) => {
			const matchesTeacher =
				!query ||
				teacher.displayName.toLowerCase().includes(query) ||
				teacher.subjectGroupNames.some((name) => name.toLowerCase().includes(query));
			const matchesSubjectGroup =
				selectedSubjectGroup === 'all' || teacher.subjectGroupNames.includes(selectedSubjectGroup);
			const matchesSubject =
				selectedSubject === 'all' ||
				teacher.periods.some((cell) =>
					cell.entries.some((entry) => entrySubjectValue(entry) === selectedSubject)
				);
			const matchesClassroom =
				selectedClassroom === 'all' ||
				teacher.periods.some((cell) =>
					cell.entries.some((entry) => entry.classroomName === selectedClassroom)
				);

			return matchesTeacher && matchesSubjectGroup && matchesSubject && matchesClassroom;
		});
	});
	const teacherColumnWidth = $derived.by(() => {
		const longestNameLength = Math.max(
			3,
			...filteredTeachers.map((teacher) => teacher.displayName.length)
		);
		return clamp(104 + longestNameLength * 7, 136, 188);
	});
	const periodColumnWidth = 168;
	const tableMinWidth = $derived(
		Math.max(680, teacherColumnWidth + (overview?.periods.length ?? 4) * periodColumnWidth)
	);

	function currentOverviewKey(): string {
		return [
			selectedDate,
			selectedSemesterId,
			canUseAcademicFilters && includeEmptyTeachers ? 'include-empty' : 'teaching-only',
			canReadDailyTeaching ? 'allowed' : 'blocked'
		].join('|');
	}

	async function loadStructure() {
		if (!canUseAcademicFilters || structureLoaded || loadingStructure) return;

		try {
			loadingStructure = true;
			const response = await getAcademicStructure();
			academicYears = response.data.years;
			semesters = response.data.semesters;

			const activeSemester = semesters.find((semester) => semester.is_active) ?? semesters[0];
			if (activeSemester) {
				selectedSemesterId = activeSemester.id;
				selectedYearId = activeSemester.academic_year_id;
			} else if (academicYears.length > 0) {
				const activeYear = academicYears.find((year) => year.is_active) ?? academicYears[0];
				selectedYearId = activeYear.id;
			}

			structureLoaded = true;
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'โหลดปีการศึกษาไม่สำเร็จ');
		} finally {
			loadingStructure = false;
		}
	}

	async function loadOverview(key = currentOverviewKey()) {
		if (!canReadDailyTeaching) {
			overview = null;
			loading = false;
			return;
		}
		if (!selectedDate) return;
		if (key === lastLoadedKey && overview && !errorMessage) return;

		const requestSeq = ++overviewRequestSeq;
		try {
			loading = true;
			errorMessage = '';
			const response = await getDailyTeachingOverview({
				date: selectedDate,
				academicSemesterId: selectedSemesterId || undefined,
				includeEmptyTeachers: canUseAcademicFilters && includeEmptyTeachers
			});

			if (requestSeq !== overviewRequestSeq) return;
			overview = response.data;
			lastLoadedKey = key;
			resetInvalidFilters();
		} catch (error) {
			if (requestSeq !== overviewRequestSeq) return;
			errorMessage = error instanceof Error ? error.message : 'โหลดตารางสอนวันนี้ไม่สำเร็จ';
			toast.error(errorMessage);
		} finally {
			if (requestSeq === overviewRequestSeq) loading = false;
		}
	}

	function resetInvalidFilters() {
		if (selectedSubjectGroup !== 'all' && !subjectGroupOptions.some(matchesSelectedSubjectGroup)) {
			selectedSubjectGroup = 'all';
		}
		if (selectedSubject !== 'all' && !subjectOptions.some(matchesSelectedSubject)) {
			selectedSubject = 'all';
		}
		if (selectedClassroom !== 'all' && !classroomOptions.some(matchesSelectedClassroom)) {
			selectedClassroom = 'all';
		}
	}

	function matchesSelectedSubjectGroup(option: FilterOption) {
		return option.value === selectedSubjectGroup;
	}

	function matchesSelectedSubject(option: FilterOption) {
		return option.value === selectedSubject;
	}

	function matchesSelectedClassroom(option: FilterOption) {
		return option.value === selectedClassroom;
	}

	function handleRetry() {
		lastLoadedKey = '';
		void loadOverview();
	}

	function handleRefresh() {
		lastLoadedKey = '';
		void loadOverview();
	}

	function handlePrint() {
		window.print();
	}

	function moveDate(offsetDays: number) {
		const nextDate = parseDateInput(selectedDate);
		nextDate.setDate(nextDate.getDate() + offsetDays);
		selectedDate = toDateInputValue(nextDate);
	}

	function openCell(
		teacher: DailyTeachingTeacher,
		period: DailyTeachingPeriod,
		entries: DailyTeachingEntry[]
	) {
		selectedCell = { teacher, period, entries };
		cellDialogOpen = true;
	}

	function cellForPeriod(teacher: DailyTeachingTeacher, periodId: string) {
		return (
			teacher.periods.find((cell) => cell.periodId === periodId) ?? {
				periodId,
				entries: []
			}
		);
	}

	function forEachOverviewEntry(callback: (entry: DailyTeachingEntry) => void) {
		for (const teacher of overview?.teachers ?? []) {
			for (const cell of teacher.periods) {
				for (const entry of cell.entries) {
					callback(entry);
				}
			}
		}
	}

	function toSortedOptions(values: Set<string> | Map<string, string>): FilterOption[] {
		const options =
			values instanceof Map
				? Array.from(values, ([value, label]) => ({ value, label }))
				: Array.from(values, (value) => ({ value, label: value }));

		return options.sort((a, b) => a.label.localeCompare(b.label, 'th'));
	}

	function entrySubjectValue(entry: DailyTeachingEntry): string {
		return entry.subjectCode ?? entry.subjectName ?? entry.title ?? '';
	}

	function entrySubjectLabel(entry: DailyTeachingEntry): string {
		if (entry.subjectCode && entry.subjectName) return `${entry.subjectCode} ${entry.subjectName}`;
		return entry.subjectName ?? entry.subjectCode ?? entry.title ?? entryTypeLabel(entry.entryType);
	}

	function entryTitle(entry: DailyTeachingEntry): string {
		if (entry.entryType === 'COURSE') return entrySubjectLabel(entry);
		return entry.title ?? entry.subjectName ?? entryTypeLabel(entry.entryType);
	}

	function entrySubjectCodeLine(entry: DailyTeachingEntry): string {
		if (entry.entryType !== 'COURSE') return '';
		return entry.subjectCode ?? '';
	}

	function entrySubjectNameLine(entry: DailyTeachingEntry): string {
		if (entry.entryType !== 'COURSE') {
			return entry.title ?? entry.subjectName ?? entryTypeLabel(entry.entryType);
		}
		if (entry.subjectName) return entry.subjectName;
		if (entry.title && entry.title !== entry.subjectCode) return entry.title;
		return entry.subjectCode ? '' : entryTypeLabel(entry.entryType);
	}

	function entryMeta(entry: DailyTeachingEntry): string {
		return [entry.classroomName, entry.roomCode].filter(Boolean).join(' / ');
	}

	function periodLabel(period: DailyTeachingPeriod): string {
		return period.name || `คาบ ${period.orderIndex}`;
	}

	function periodTime(period: DailyTeachingPeriod): string {
		return `${formatTime(period.startTime)}-${formatTime(period.endTime)}`;
	}

	function formatTime(time: string): string {
		return time.substring(0, 5);
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

	function clamp(value: number, min: number, max: number): number {
		return Math.min(max, Math.max(min, value));
	}

	function entryTypeLabel(entryType: DailyTeachingEntry['entryType']): string {
		switch (entryType) {
			case 'ACTIVITY':
				return 'กิจกรรม';
			case 'ACADEMIC':
				return 'งานวิชาการ';
			case 'BREAK':
				return 'พัก';
			case 'HOMEROOM':
				return 'โฮมรูม';
			default:
				return 'รายวิชา';
		}
	}

	function entryBadgeVariant(entryType: DailyTeachingEntry['entryType']): BadgeVariant {
		switch (entryType) {
			case 'COURSE':
				return 'default';
			case 'BREAK':
				return 'secondary';
			case 'HOMEROOM':
				return 'outline';
			default:
				return 'secondary';
		}
	}

	$effect(() => {
		if (!canUseAcademicFilters && includeEmptyTeachers) {
			includeEmptyTeachers = false;
		}
	});

	$effect(() => {
		if (initialized && canUseAcademicFilters && !structureLoaded) {
			void loadStructure();
		}
	});

	$effect(() => {
		if (!selectedYearId || semesterOptions.length === 0) return;
		if (
			selectedSemesterId &&
			semesterOptions.some((semester) => semester.id === selectedSemesterId)
		) {
			return;
		}

		const activeSemester =
			semesterOptions.find((semester) => semester.is_active) ?? semesterOptions[0];
		selectedSemesterId = activeSemester.id;
	});

	$effect(() => {
		const key = currentOverviewKey();
		if (!initialized || !canReadDailyTeaching) return;
		void loadOverview(key);
	});

	onMount(async () => {
		await loadStructure();
		initialized = true;
	});
</script>

<svelte:head>
	<title>{data.title} - SchoolOrbit</title>
</svelte:head>

<PageShell
	title="ตารางสอนวันนี้"
	description="ภาพรวมคาบสอนของครูทั้งโรงเรียน แยกตามวัน คาบ ชั้นเรียน และรายวิชา"
>
	<div class="print:hidden space-y-3 rounded-md border bg-background p-3">
		<div class="flex flex-col gap-3 xl:flex-row xl:items-end xl:justify-between">
			<div class="grid flex-1 gap-3 md:grid-cols-2 xl:grid-cols-4">
				<div class="space-y-1.5">
					<Label for="teaching-date">วันที่</Label>
					<div class="flex gap-2">
						<Button
							variant="outline"
							size="icon"
							onclick={() => moveDate(-1)}
							aria-label="วันก่อนหน้า"
						>
							<ChevronLeft class="h-4 w-4" />
						</Button>
						<Input id="teaching-date" type="date" bind:value={selectedDate} class="min-w-0" />
						<Button variant="outline" size="icon" onclick={() => moveDate(1)} aria-label="วันถัดไป">
							<ChevronRight class="h-4 w-4" />
						</Button>
					</div>
				</div>

				{#if canUseAcademicFilters}
					<div class="space-y-1.5">
						<Label for="academic-year">ปีการศึกษา</Label>
						<Select.Root type="single" bind:value={selectedYearId}>
							<Select.Trigger id="academic-year" class="w-full">
								{academicYears.find((year) => year.id === selectedYearId)?.name ??
									'เลือกปีการศึกษา'}
							</Select.Trigger>
							<Select.Content>
								{#each academicYears as year (year.id)}
									<Select.Item value={year.id}>{year.name}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>

					<div class="space-y-1.5">
						<Label for="academic-semester">ภาคเรียน</Label>
						<Select.Root type="single" bind:value={selectedSemesterId}>
							<Select.Trigger id="academic-semester" class="w-full">
								{semesterOptions.find((semester) => semester.id === selectedSemesterId)?.name ??
									'เลือกภาคเรียน'}
							</Select.Trigger>
							<Select.Content>
								{#each semesterOptions as semester (semester.id)}
									<Select.Item value={semester.id}>{semester.name}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>
				{/if}

				<div class="space-y-1.5">
					<Label for="teacher-search">ค้นหาครู</Label>
					<div class="relative">
						<Search class="text-muted-foreground absolute left-3 top-2.5 h-4 w-4" />
						<Input
							id="teacher-search"
							bind:value={teacherSearch}
							placeholder="ชื่อครูหรือกลุ่มงาน"
							class="pl-9"
						/>
					</div>
				</div>
			</div>

			<div class="flex flex-wrap items-center gap-2">
				<Button variant="outline" onclick={handleRefresh} disabled={loading}>
					<RefreshCw class={loading ? 'h-4 w-4 animate-spin' : 'h-4 w-4'} />
					รีเฟรช
				</Button>
				{#if canOpenPlanner}
					<Button variant="outline" href="/staff/academic/timetable">
						<ExternalLink class="h-4 w-4" />
						จัดตาราง
					</Button>
				{/if}
				<Button variant="outline" onclick={handlePrint} disabled={!overview}>
					<Printer class="h-4 w-4" />
					พิมพ์
				</Button>
			</div>
		</div>

		{#if canUseAcademicFilters}
			<div class="grid gap-3 border-t pt-3 md:grid-cols-2 xl:grid-cols-4">
				<div class="space-y-1.5">
					<Label for="subject-group-filter">กลุ่มสาระ</Label>
					<Select.Root type="single" bind:value={selectedSubjectGroup}>
						<Select.Trigger id="subject-group-filter" class="w-full">
							{selectedSubjectGroup === 'all' ? 'ทุกกลุ่มสาระ' : selectedSubjectGroup}
						</Select.Trigger>
						<Select.Content>
							<Select.Item value="all">ทุกกลุ่มสาระ</Select.Item>
							{#each subjectGroupOptions as option (option.value)}
								<Select.Item value={option.value}>{option.label}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>

				<div class="space-y-1.5">
					<Label for="subject-filter">วิชา/กิจกรรม</Label>
					<Select.Root type="single" bind:value={selectedSubject}>
						<Select.Trigger id="subject-filter" class="w-full">
							{selectedSubject === 'all'
								? 'ทุกวิชา'
								: subjectOptions.find((option) => option.value === selectedSubject)?.label}
						</Select.Trigger>
						<Select.Content>
							<Select.Item value="all">ทุกวิชา</Select.Item>
							{#each subjectOptions as option (option.value)}
								<Select.Item value={option.value}>{option.label}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>

				<div class="space-y-1.5">
					<Label for="classroom-filter">ชั้น/ห้อง</Label>
					<Select.Root type="single" bind:value={selectedClassroom}>
						<Select.Trigger id="classroom-filter" class="w-full">
							{selectedClassroom === 'all' ? 'ทุกชั้นเรียน' : selectedClassroom}
						</Select.Trigger>
						<Select.Content>
							<Select.Item value="all">ทุกชั้นเรียน</Select.Item>
							{#each classroomOptions as option (option.value)}
								<Select.Item value={option.value}>{option.label}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>

				<div class="flex items-end">
					<div class="flex min-h-9 items-center gap-3 rounded-md border px-3 py-2">
						<Switch id="include-empty-teachers" bind:checked={includeEmptyTeachers} />
						<Label for="include-empty-teachers" class="text-sm">รวมครูที่ไม่มีคาบ</Label>
					</div>
				</div>
			</div>
		{/if}
	</div>

	{#if !canReadDailyTeaching}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ดูตารางสอนวันนี้"
			description="บัญชีนี้ยังไม่ได้รับสิทธิ์ดูภาพรวมตารางสอนรายวันของโรงเรียน"
		/>
	{:else if loading && !overview}
		<PageSkeleton variant="table" rows={8} columns={6} />
	{:else if errorMessage}
		<PageState
			variant="error"
			title="โหลดตารางสอนไม่สำเร็จ"
			description={errorMessage}
			actionLabel="ลองใหม่"
			onaction={handleRetry}
		/>
	{:else if !overview}
		<PageState
			title="ยังไม่มีข้อมูลตารางสอน"
			description="เลือกวันที่หรือภาคเรียนแล้วรีเฟรชอีกครั้ง"
			actionLabel="รีเฟรช"
			onaction={handleRefresh}
		/>
	{:else}
		<div class="grid gap-3 md:grid-cols-2 xl:grid-cols-5">
			<div class="rounded-md border bg-background p-3">
				<p class="text-muted-foreground text-xs">ครูทั้งหมด</p>
				<p class="text-2xl font-semibold">{overview.summary.totalTeacherCount}</p>
			</div>
			<div class="rounded-md border bg-background p-3">
				<p class="text-muted-foreground text-xs">ครูที่แสดง</p>
				<p class="text-2xl font-semibold">{filteredTeachers.length}</p>
			</div>
			<div class="rounded-md border bg-background p-3">
				<p class="text-muted-foreground text-xs">ครูที่มีคาบ</p>
				<p class="text-2xl font-semibold">{overview.summary.teachersTeachingCount}</p>
			</div>
			<div class="rounded-md border bg-background p-3">
				<p class="text-muted-foreground text-xs">จำนวนคาบสอน</p>
				<p class="text-2xl font-semibold">{overview.summary.lessonCount}</p>
			</div>
			<div class="rounded-md border bg-background p-3">
				<p class="text-muted-foreground text-xs">ไม่มีคาบวันนี้</p>
				<p class="text-2xl font-semibold">{overview.summary.emptyTeacherCount}</p>
			</div>
		</div>

		<section class="schedule-shell rounded-md border bg-background">
			<div class="flex flex-col gap-2 border-b p-4 md:flex-row md:items-center md:justify-between">
				<div>
					<div class="flex items-center gap-2">
						<CalendarClock class="text-muted-foreground h-5 w-5" />
						<h2 class="text-lg font-semibold">{formatDate(overview.date)}</h2>
					</div>
					<p class="text-muted-foreground text-sm">
						{overview.periods.length} คาบ / {overview.summary.displayedTeacherCount} ครู
					</p>
				</div>
				{#if loading || loadingStructure}
					<Badge variant="secondary">กำลังอัปเดต</Badge>
				{/if}
			</div>

			{#if filteredTeachers.length === 0}
				<PageState
					title="ไม่พบครูตามตัวกรอง"
					description="ลองล้างคำค้นหาหรือตัวกรองด้านบน"
					class="m-4"
				/>
			{:else}
				<div class="daily-teaching-scroll max-h-[70vh] overflow-x-auto overflow-y-auto">
					<Table.Root
						class="daily-teaching-table border-0"
						style={`--teacher-column-width: ${teacherColumnWidth}px; --period-column-width: ${periodColumnWidth}px; min-width: ${tableMinWidth}px;`}
					>
						<Table.Header class="sticky top-0 z-40">
							<Table.Row class="bg-muted/60 hover:bg-muted/60">
								<Table.Head class="daily-teaching-teacher-column sticky left-0 top-0 z-50 bg-muted">
									ครู
								</Table.Head>
								{#each overview.periods as period (period.id)}
									<Table.Head
										class="daily-teaching-period-column sticky top-0 z-40 bg-muted text-center"
									>
										<div class="font-medium">{periodLabel(period)}</div>
										<div class="text-muted-foreground text-xs font-normal">
											{periodTime(period)}
										</div>
									</Table.Head>
								{/each}
							</Table.Row>
						</Table.Header>
						<Table.Body>
							{#each filteredTeachers as teacher (teacher.id)}
								<Table.Row>
									<Table.Cell
										class="daily-teaching-teacher-column sticky left-0 z-20 overflow-hidden bg-background"
									>
										<div class="min-w-0">
											<p class="truncate font-medium">{teacher.displayName}</p>
											{#if teacher.subjectGroupNames.length > 0}
												<p class="text-muted-foreground truncate text-xs">
													{teacher.subjectGroupNames.join(', ')}
												</p>
											{/if}
										</div>
									</Table.Cell>

									{#each overview.periods as period (period.id)}
										{@const cell = cellForPeriod(teacher, period.id)}
										<Table.Cell class="daily-teaching-period-column align-top">
											<button
												type="button"
												class="hover:border-primary/50 hover:bg-accent/40 focus-visible:border-ring focus-visible:ring-ring/50 min-h-24 w-full rounded-md border border-dashed border-transparent p-2 text-left transition-colors focus-visible:ring-[3px] focus-visible:outline-none"
												onclick={() => openCell(teacher, period, cell.entries)}
											>
												{#if cell.entries.length === 0}
													<span class="text-muted-foreground text-xs">ว่าง</span>
												{:else}
													<div class="space-y-1.5">
														{#each cell.entries as entry (entry.entryId)}
															{@const subjectCodeLine = entrySubjectCodeLine(entry)}
															{@const subjectNameLine = entrySubjectNameLine(entry)}
															<div class="rounded-md border bg-muted/30 p-2">
																<div class="flex items-center gap-1.5">
																	<Badge
																		variant={entryBadgeVariant(entry.entryType)}
																		class="max-w-full truncate"
																	>
																		{entryTypeLabel(entry.entryType)}
																	</Badge>
																	{#if entry.isTeamTeaching}
																		<Badge variant="outline">ทีมสอน</Badge>
																	{/if}
																</div>
																<div class="mt-1 min-w-0">
																	{#if subjectCodeLine}
																		<p class="text-muted-foreground truncate text-xs font-medium">
																			{subjectCodeLine}
																		</p>
																	{/if}
																	{#if subjectNameLine}
																		<p class="line-clamp-2 text-sm font-medium">
																			{subjectNameLine}
																		</p>
																	{/if}
																</div>
																{#if entryMeta(entry)}
																	<p class="text-muted-foreground mt-1 truncate text-xs">
																		{entryMeta(entry)}
																	</p>
																{/if}
															</div>
														{/each}
													</div>
												{/if}
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
		<Dialog.Header>
			<Dialog.Title>
				{selectedCell?.teacher.displayName ?? 'รายละเอียดคาบ'}
			</Dialog.Title>
			<Dialog.Description>
				{#if selectedCell}
					{periodLabel(selectedCell.period)} {periodTime(selectedCell.period)}
				{/if}
			</Dialog.Description>
		</Dialog.Header>

		{#if selectedCell}
			{#if selectedCell.entries.length === 0}
				<div class="rounded-md border border-dashed p-4 text-sm text-muted-foreground">
					ไม่มีคาบสอนในช่วงเวลานี้
				</div>
			{:else}
				<div class="space-y-3">
					{#each selectedCell.entries as entry (entry.entryId)}
						<div class="rounded-md border p-4">
							<div class="mb-2 flex flex-wrap items-center gap-2">
								<Badge variant={entryBadgeVariant(entry.entryType)}>
									{entryTypeLabel(entry.entryType)}
								</Badge>
								{#if entry.isTeamTeaching}
									<Badge variant="outline">ทีมสอน</Badge>
								{/if}
							</div>
							<h3 class="font-semibold">{entryTitle(entry)}</h3>
							<div class="mt-3 grid gap-2 text-sm md:grid-cols-2">
								<div>
									<p class="text-muted-foreground text-xs">ชั้น/ห้อง</p>
									<p>{entry.classroomName ?? '-'}</p>
								</div>
								<div>
									<p class="text-muted-foreground text-xs">ห้องเรียน</p>
									<p>{entry.roomCode ?? '-'}</p>
								</div>
								<div>
									<p class="text-muted-foreground text-xs">กลุ่มสาระ</p>
									<p>{entry.subjectGroupName ?? '-'}</p>
								</div>
								<div>
									<p class="text-muted-foreground text-xs">รหัสวิชา</p>
									<p>{entry.subjectCode ?? '-'}</p>
								</div>
							</div>
							{#if entry.note}
								<p class="text-muted-foreground mt-3 text-sm">{entry.note}</p>
							{/if}
						</div>
					{/each}
				</div>
			{/if}
		{/if}
	</Dialog.Content>
</Dialog.Root>

<style>
	:global(.daily-teaching-table th),
	:global(.daily-teaching-table td) {
		border-bottom: 1px solid hsl(var(--border));
	}

	:global(.daily-teaching-teacher-column) {
		width: var(--teacher-column-width);
		min-width: var(--teacher-column-width);
		max-width: var(--teacher-column-width);
	}

	:global(.daily-teaching-period-column) {
		width: var(--period-column-width);
		min-width: var(--period-column-width);
		max-width: var(--period-column-width);
	}

	:global(.daily-teaching-scroll [data-slot='table-container']) {
		overflow: visible;
	}

	@media print {
		.schedule-shell {
			border: 0;
		}
	}
</style>
