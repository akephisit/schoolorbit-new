<script lang="ts">
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';
	import {
		type TimetableEntry,
		type AcademicPeriod,
		listTimetableEntries,
		createTimetableEntry,
		deleteTimetableEntry,
		listPeriods
	} from '$lib/api/timetable';
	import {
		lookupAcademicYears,
		listClassrooms,
		listClassroomCourses,
		type Classroom
	} from '$lib/api/academic';

	import * as Card from '$lib/components/ui/card';
	import * as Table from '$lib/components/ui/table';
	import { Button } from '$lib/components/ui/button';
	import * as Select from '$lib/components/ui/select';
	import { Badge } from '$lib/components/ui/badge';

	import {
		CalendarDays,
		Trash2,
		Loader2,
		Clock,
		School,
		GripVertical,
		BookOpen
	} from 'lucide-svelte';

	const DAYS = [
		{ value: 'MON', label: 'จันทร์', shortLabel: 'จ' },
		{ value: 'TUE', label: 'อังคาร', shortLabel: 'อ' },
		{ value: 'WED', label: 'พุธ', shortLabel: 'พ' },
		{ value: 'THU', label: 'พฤหัสบดี', shortLabel: 'พฤ' },
		{ value: 'FRI', label: 'ศุกร์', shortLabel: 'ศ' }
	];

	// State
	let loading = $state(true);
	let timetableEntries = $state<TimetableEntry[]>([]);
	let periods = $state<AcademicPeriod[]>([]);
	let classrooms = $state<Classroom[]>([]);
	let courses = $state<any[]>([]);
	let academicYears = $state<any[]>([]);

	let selectedYearId = $state('');
	let selectedClassroomId = $state('');

	// Drag & Drop state
	let draggedCourse = $state<any>(null);
	let submitting = $state(false);

	async function loadInitialData() {
		try {
			loading = true;
			const [yearsRes] = await Promise.all([lookupAcademicYears(false)]);

			academicYears = yearsRes.data;

			if (academicYears.length > 0) {
				const activeYear = academicYears.find((y) => y.is_current) || academicYears[0];
				selectedYearId = activeYear.id;
				await loadClassrooms();
			}
		} catch (e) {
			toast.error('โหลดข้อมูลไม่สำเร็จ');
		} finally {
			loading = false;
		}
	}

	async function loadClassrooms() {
		if (!selectedYearId) return;
		try {
			const res = await listClassrooms({ year_id: selectedYearId });
			classrooms = res.data;
		} catch (e) {
			console.error(e);
		}
	}

	async function loadPeriods() {
		if (!selectedYearId) return;
		try {
			const res = await listPeriods({ academic_year_id: selectedYearId, active_only: true });
			periods = res.data.filter((p) => p.type === 'TEACHING');
		} catch (e) {
			console.error(e);
		}
	}

	async function loadCoursesForClassroom() {
		if (!selectedClassroomId) return;
		try {
			const res = await listClassroomCourses(selectedClassroomId);
			courses = res.data;
		} catch (e) {
			console.error(e);
		}
	}

	async function loadTimetable() {
		if (!selectedClassroomId) {
			timetableEntries = [];
			return;
		}

		try {
			const res = await listTimetableEntries({ classroom_id: selectedClassroomId });
			timetableEntries = res.data;
		} catch (e) {
			toast.error('โหลดตารางสอนไม่สำเร็จ');
		}
	}

	async function handleDeleteEntry(entryId: string) {
		try {
			await deleteTimetableEntry(entryId);
			toast.success('ลบออกจากตารางสำเร็จ');
			loadTimetable();
		} catch (e: any) {
			toast.error(e.message || 'ลบไม่สำเร็จ');
		}
	}

	function getEntryForSlot(day: string, periodId: string): TimetableEntry | undefined {
		return timetableEntries.find((e) => e.day_of_week === day && e.period_id === periodId);
	}

	function formatTime(time?: string): string {
		if (!time) return '';
		return time.substring(0, 5);
	}

	// ============================================
	// Drag & Drop Handlers
	// ============================================

	function handleDragStart(event: DragEvent, course: any) {
		draggedCourse = course;
		if (event.dataTransfer) {
			event.dataTransfer.effectAllowed = 'move';
			event.dataTransfer.setData('text/plain', course.id);
		}
	}

	function handleDragEnd() {
		draggedCourse = null;
	}

	function handleDragOver(event: DragEvent) {
		event.preventDefault();
		if (event.dataTransfer) {
			event.dataTransfer.dropEffect = 'move';
		}
	}

	async function handleDrop(event: DragEvent, day: string, periodId: string) {
		event.preventDefault();

		if (!draggedCourse) return;

		// Store course info before clearing (to avoid null reference in toast)
		const courseCode = draggedCourse.subject_code;
		const courseId = draggedCourse.id;

		// Check if slot is already occupied
		const existingEntry = getEntryForSlot(day, periodId);
		if (existingEntry) {
			toast.error('ช่องนี้มีรายการอยู่แล้ว กรุณาลบรายการเดิมออกก่อน');
			draggedCourse = null;
			return;
		}

		const payload = {
			classroom_course_id: courseId,
			day_of_week: day,
			period_id: periodId
		};

		try {
			submitting = true;
			const res = await createTimetableEntry(payload);

			if (res.success === false) {
				toast.error(res.message || 'พบข้อขัดแย้งในตาราง');
				if (res.conflicts && res.conflicts.length > 0) {
					res.conflicts.forEach((c: any) => {
						toast.error(c.message);
					});
				}
			} else {
				await loadTimetable();
				toast.success(`เพิ่ม ${courseCode} ลงตารางสำเร็จ`);
			}
		} catch (e: any) {
			toast.error(e.message || 'เพิ่มลงตารางไม่สำเร็จ');
		} finally {
			submitting = false;
			draggedCourse = null;
		}
	}

	let unscheduledCourses = $derived.by(() => {
		const courseCounts = new Map<string, number>();
		timetableEntries.forEach((entry) => {
			const count = courseCounts.get(entry.classroom_course_id) || 0;
			courseCounts.set(entry.classroom_course_id, count + 1);
		});

		return courses.map((course) => ({
			...course,
			scheduled_count: courseCounts.get(course.id) || 0
		}));
	});

	$effect(() => {
		if (selectedYearId) {
			loadClassrooms();
			loadPeriods();
		}
	});

	$effect(() => {
		if (selectedClassroomId) {
			loadCoursesForClassroom();
			loadTimetable();
		}
	});

	onMount(loadInitialData);
</script>

<div class="h-full flex flex-col space-y-4">
	<div class="flex flex-col gap-2">
		<h2 class="text-3xl font-bold flex items-center gap-2">
			<CalendarDays class="w-8 h-8" />
			จัดตารางสอน
		</h2>
		<p class="text-muted-foreground">
			ลากวิชาจากด้านซ้าย มาวางในช่องตารางด้านขวา (ระบบจะตรวจสอบการชนอัตโนมัติ)
		</p>
	</div>

	<!-- Filters -->
	<div class="flex items-center gap-4 flex-wrap">
		<div class="w-[200px]">
			<Select.Root type="single" bind:value={selectedYearId}>
				<Select.Trigger class="w-full">
					{academicYears.find((y) => y.id === selectedYearId)?.name || 'เลือกปีการศึกษา'}
				</Select.Trigger>
				<Select.Content>
					{#each academicYears as year}
						<Select.Item value={year.id}>{year.name}</Select.Item>
					{/each}
				</Select.Content>
			</Select.Root>
		</div>

		<div class="w-[250px]">
			<Select.Root type="single" bind:value={selectedClassroomId}>
				<Select.Trigger class="w-full">
					<School class="w-4 h-4 mr-2" />
					{classrooms.find((c) => c.id === selectedClassroomId)?.name || 'เลือกห้องเรียน'}
				</Select.Trigger>
				<Select.Content>
					{#each classrooms as classroom}
						<Select.Item value={classroom.id}>{classroom.name}</Select.Item>
					{/each}
				</Select.Content>
			</Select.Root>
		</div>
	</div>

	<!-- Main Content: 2 Columns Layout -->
	{#if !selectedClassroomId}
		<Card.Root>
			<Card.Content class="py-12 text-center">
				<School class="w-16 h-16 mx-auto text-muted-foreground mb-4" />
				<p class="text-muted-foreground">กรุณาเลือกห้องเรียนเพื่อดูและจัดตารางสอน</p>
			</Card.Content>
		</Card.Root>
	{:else if periods.length === 0}
		<Card.Root>
			<Card.Content class="py-12 text-center">
				<Clock class="w-16 h-16 mx-auto text-muted-foreground mb-4" />
				<p class="text-muted-foreground">
					ยังไม่มีคาบเวลาในปีนี้ กรุณาไปที่เมนู "ตั้งค่าคาบเวลา" ก่อน
				</p>
			</Card.Content>
		</Card.Root>
	{:else}
		<div class="grid grid-cols-12 gap-4 flex-1 overflow-hidden">
			<!-- Left Sidebar: Draggable Courses -->
			<div class="col-span-3 flex flex-col">
				<Card.Root class="flex-1 flex flex-col">
					<Card.Header>
						<Card.Title class="flex items-center gap-2">
							<BookOpen class="w-5 h-5" />
							รายวิชาที่ต้องจัด
						</Card.Title>
						<Card.Description>ลากวิชาเหล่านี้ไปวางในตาราง</Card.Description>
					</Card.Header>
					<Card.Content class="flex-1 overflow-y-auto space-y-2">
						{#if courses.length === 0}
							<p class="text-sm text-muted-foreground text-center py-8">
								ยังไม่มีรายวิชาที่จัด<br />
								กรุณาไปที่เมนู "จัดแผนการเรียน" ก่อน
							</p>
						{:else}
							{#each unscheduledCourses as course}
								<div
									role="button"
									tabindex="0"
									draggable="true"
									ondragstart={(e) => handleDragStart(e, course)}
									ondragend={handleDragEnd}
									class="p-3 bg-gradient-to-r from-blue-50 to-blue-100 border border-blue-200 rounded-lg cursor-move hover:shadow-md transition-shadow group"
								>
									<div class="flex items-start gap-2">
										<GripVertical class="w-4 h-4 text-blue-400 flex-shrink-0 mt-0.5" />
										<div class="flex-1 min-w-0">
											<div class="font-medium text-sm text-blue-900">{course.subject_code}</div>
											<div class="text-xs text-blue-700 truncate">{course.subject_name_th}</div>
											{#if course.instructor_name}
												<div class="text-xs text-blue-600 mt-1">ครู: {course.instructor_name}</div>
											{/if}
											{#if course.scheduled_count > 0}
												<Badge
													variant="outline"
													class="mt-1 text-xs bg-green-50 text-green-700 border-green-200"
												>
													จัดแล้ว {course.scheduled_count} คาบ
												</Badge>
											{/if}
										</div>
									</div>
								</div>
							{/each}
						{/if}
					</Card.Content>
				</Card.Root>
			</div>

			<!-- Right: Timetable Grid -->
			<div class="col-span-9 flex flex-col">
				<Card.Root class="flex-1 flex flex-col overflow-hidden">
					<Card.Header>
						<Card.Title>ตารางเรียน</Card.Title>
					</Card.Header>
					<Card.Content class="flex-1 overflow-auto">
						<Table.Root>
							<Table.Header>
								<Table.Row>
									<Table.Head class="w-[100px] sticky left-0 bg-background z-10">วัน/คาบ</Table.Head
									>
									{#each periods as period}
										<Table.Head class="text-center min-w-[140px]">
											<div class="font-bold text-sm">{period.name}</div>
											<div class="text-xs text-muted-foreground">
												{formatTime(period.start_time)}-{formatTime(period.end_time)}
											</div>
										</Table.Head>
									{/each}
								</Table.Row>
							</Table.Header>
							<Table.Body>
								{#if loading}
									<Table.Row>
										<Table.Cell colspan={periods.length + 1} class="h-24 text-center">
											<Loader2 class="animate-spin mx-auto" />
										</Table.Cell>
									</Table.Row>
								{:else}
									{#each DAYS as day}
										<Table.Row>
											<Table.Cell class="sticky left-0 bg-background z-10 border-r">
												<div class="font-bold text-sm">{day.label}</div>
											</Table.Cell>
											{#each periods as period}
												{@const entry = getEntryForSlot(day.value, period.id)}
												<Table.Cell class="p-2">
													{#if entry}
														<div
															class="bg-gradient-to-br from-blue-50 to-indigo-50 border-2 border-blue-300 rounded-lg p-3 relative group hover:shadow-lg transition-all"
														>
															<div class="font-bold text-sm text-blue-900">
																{entry.subject_code}
															</div>
															<div class="text-xs text-blue-700 line-clamp-2">
																{entry.subject_name_th}
															</div>
															{#if entry.instructor_name}
																<div class="text-xs text-blue-600 mt-1 flex items-center gap-1">
																	<span class="w-1 h-1 rounded-full bg-blue-400"></span>
																	{entry.instructor_name}
																</div>
															{/if}
															<button
																onclick={() => handleDeleteEntry(entry.id)}
																class="absolute -top-2 -right-2 opacity-0 group-hover:opacity-100 transition-opacity bg-red-500 hover:bg-red-600 rounded-full p-1.5 shadow-lg"
																title="ลบออกจากตาราง"
															>
																<Trash2 class="w-3 h-3 text-white" />
															</button>
														</div>
													{:else}
														<div
															role="region"
															aria-label="Drop zone"
															ondragover={handleDragOver}
															ondrop={(e) => handleDrop(e, day.value, period.id)}
															class="h-24 border-2 border-dashed border-muted rounded-lg hover:border-blue-300 hover:bg-blue-50/50 transition-colors flex items-center justify-center"
														>
															<span
																class="text-xs text-muted-foreground opacity-0 hover:opacity-100"
															>
																วางที่นี่
															</span>
														</div>
													{/if}
												</Table.Cell>
											{/each}
										</Table.Row>
									{/each}
								{/if}
							</Table.Body>
						</Table.Root>
					</Card.Content>
				</Card.Root>
			</div>
		</div>
	{/if}
</div>
