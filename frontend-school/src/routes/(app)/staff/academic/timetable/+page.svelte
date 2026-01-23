<script lang="ts">
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';
	import {
		type TimetableEntry,
		type AcademicPeriod,
		listTimetableEntries,
		createTimetableEntry,
		updateTimetableEntry,
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

	// Mobile drag & drop support
	// @ts-ignore
	import { polyfill } from 'mobile-drag-drop';
	// @ts-ignore
	import { scrollBehaviourDragImageTranslateOverride } from 'mobile-drag-drop/scroll-behaviour';

	// Initialize polyfill on mount (client-side only)
	if (typeof window !== 'undefined') {
		polyfill({
			dragImageTranslateOverride: scrollBehaviourDragImageTranslateOverride
		});
	}

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
	// Drag & Drop Handlers (Native API)
	// ============================================

	// Drag & Drop Handlers (Native API)
	// ============================================

	// Identify what is being dragged
	// type: 'NEW' (from list) | 'MOVE' (from grid)
	let dragType = $state<'NEW' | 'MOVE'>('NEW');
	let draggedEntryId = $state<string | null>(null);

	function handleDragStart(event: DragEvent, item: any, type: 'NEW' | 'MOVE') {
		dragType = type;
		
		if (type === 'NEW') {
			draggedCourse = item;
			draggedEntryId = null;
		} else {
			draggedCourse = item; // For moving, item is the TimetableEntry (or object with same shape)
			draggedEntryId = item.id;
		}

		if (event.dataTransfer) {
			event.dataTransfer.effectAllowed = type === 'NEW' ? 'copy' : 'move';
			event.dataTransfer.setData('text/plain', JSON.stringify({ 
				type,
				id: type === 'NEW' ? item.id : item.id
			}));
		}
	}

	function handleDragEnd() {
		draggedCourse = null;
		draggedEntryId = null;
	}

	function handleDragOver(event: DragEvent) {
		event.preventDefault(); // Necessary to allow dropping
		if (event.dataTransfer) {
			event.dataTransfer.dropEffect = dragType === 'NEW' ? 'copy' : 'move';
		}
	}

	async function handleDrop(event: DragEvent, day: string, periodId: string) {
		event.preventDefault();

		if (!draggedCourse) return;
        
        // Prevent drop if slot occupied (double check)
        const existingEntry = getEntryForSlot(day, periodId);
		if (existingEntry) {
			toast.error('ช่องนี้มีรายการอยู่แล้ว');
            // reset
            handleDragEnd();
			return;
		}

		try {
			submitting = true;

			if (dragType === 'NEW') {
				// CREATE NEW
				const courseCode = draggedCourse.subject_code;
				const payload = {
					classroom_course_id: draggedCourse.id,
					day_of_week: day,
					period_id: periodId
				};
				
				const res = await createTimetableEntry(payload);
				handleResponse(res, `ลงตาราง ${courseCode} สำเร็จ`);

			} else if (dragType === 'MOVE' && draggedEntryId) {
				// UPDATE EXISTING (MOVE)
				const courseCode = draggedCourse.subject_code;
				
				// Optional: Check if dropped in same slot
				if (draggedCourse.day_of_week === day && draggedCourse.period_id === periodId) {
					return; // No change
				}

				const payload = {
					day_of_week: day,
					period_id: periodId
				};

				const res = await updateTimetableEntry(draggedEntryId, payload);
				handleResponse(res, `ย้าย ${courseCode} สำเร็จ`);
			}

		} catch (e: any) {
			toast.error(e.message || 'บันทึกไม่สำเร็จ');
		} finally {
			submitting = false;
			handleDragEnd();
		}
	}

	async function handleResponse(res: any, successMessage: string) {
		if (res.success === false) {
			toast.error(res.message || 'พบข้อขัดแย้งในตาราง');
			if (res.conflicts && res.conflicts.length > 0) {
				res.conflicts.forEach((c: any) => {
					toast.error(c.message);
				});
			}
		} else {
			await loadTimetable();
			toast.success(successMessage);
		}
	}

	let unscheduledCourses = $derived.by(() => {
		const courseCounts = new Map<string, number>();
		timetableEntries.forEach((entry) => {
			const count = courseCounts.get(entry.classroom_course_id) || 0;
			courseCounts.set(entry.classroom_course_id, count + 1);
		});

		return courses
			.map((course) => {
				const scheduled = courseCounts.get(course.id) || 0;
				// Calculate max periods per week based on credits 
				// 1.0 credit = 2 periods/week (approx 40 hours/term)
				// 1.5 credit = 3 periods/week (approx 60 hours/term)
				// Formula: credit * 2
				const credit = course.subject_credit || 0;
				const maxPeriods = credit > 0 ? Math.ceil(credit * 2) : 3; // Default 3 if unknown
				
				return {
					...course,
					scheduled_count: scheduled,
					max_periods: maxPeriods,
					is_completed: scheduled >= maxPeriods
				};
			})
			.filter(course => !course.is_completed); // Only show courses that are not yet fully scheduled
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

<svelte:head>
	<!-- Mobile drag & drop CSS -->
	<link
		rel="stylesheet"
		href="https://cdn.jsdelivr.net/npm/mobile-drag-drop@3.0.0-rc.0/default.css"
	/>
</svelte:head>

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
		<div
			class="grid grid-cols-12 gap-4 flex-1 overflow-hidden"
			style="height: calc(100vh - 250px);"
		>
			<!-- Fixed height for scrolling -->

			<!-- Left Sidebar: Draggable Courses -->
			<div class="col-span-3 flex flex-col h-full bg-background rounded-lg border overflow-hidden">
				<div class="p-4 border-b bg-muted/30">
					<h3 class="font-semibold flex items-center gap-2">
						<BookOpen class="w-4 h-4" /> รายวิชา
					</h3>
					<p class="text-xs text-muted-foreground mt-1">ลากวิชาไปวางในตาราง</p>
				</div>
				<div class="flex-1 overflow-y-auto p-2">
					{#each unscheduledCourses as course (course.id)}
						<div
							role="button"
							tabindex="0"
							draggable="true"
							ondragstart={(e) => handleDragStart(e, course, 'NEW')}
							ondragend={handleDragEnd}
							class="mb-2 p-3 bg-white border rounded-lg shadow-sm hover:shadow-md transition-shadow cursor-grab active:cursor-grabbing w-full flex items-start gap-2 group mobile-draggable"
						>
							<GripVertical class="w-4 h-4 text-muted-foreground mt-0.5" />
							<div class="min-w-0 pointer-events-none">
								<!-- Pointer events none ensures drag catches container -->
								<div class="font-medium text-sm text-blue-900 truncate">{course.subject_code}</div>
								<div class="text-xs text-muted-foreground truncate">{course.subject_name_th}</div>
								{#if course.instructor_name}
									<div class="text-xs text-blue-600 mt-1">ครู: {course.instructor_name}</div>
								{/if}

								<div class="mt-2 flex items-center justify-between gap-2">
									<div class="flex-1 h-1.5 bg-gray-100 rounded-full overflow-hidden">
										<div
											class="h-full bg-blue-500 rounded-full transition-all"
											style="width: {(course.scheduled_count / course.max_periods) * 100}%"
										></div>
									</div>
									<span class="text-[10px] whitespace-nowrap text-muted-foreground font-medium">
										{course.scheduled_count}/{course.max_periods} คาบ
									</span>
								</div>
							</div>
						</div>
					{/each}
				</div>
			</div>

			<!-- Right: Timetable Grid -->
			<div class="col-span-9 flex flex-col h-full overflow-hidden border rounded-lg">
				<div class="overflow-auto h-full relative">
					<table class="w-full text-sm border-collapse">
						<thead class="bg-muted/50 sticky top-0 z-20 shadow-sm">
							<tr>
								<th class="p-2 border sticky left-0 z-30 bg-background w-[100px]">วัน/คาบ</th>
								{#each periods as period}
									<th class="p-2 border min-w-[140px] text-center">
										<div class="font-bold">{period.name}</div>
										<div class="text-xs text-muted-foreground font-normal">
											{formatTime(period.start_time)}-{formatTime(period.end_time)}
										</div>
									</th>
								{/each}
							</tr>
						</thead>
						<tbody>
							{#if loading}
								<tr
									><td colspan={periods.length + 1} class="p-10 text-center"
										><Loader2 class="animate-spin w-8 h-8 mx-auto" /></td
									></tr
								>
							{:else}
								{#each DAYS as day}
									<tr>
										<td class="p-2 border font-bold sticky left-0 z-10 bg-background">
											{day.label}
										</td>
										{#each periods as period}
											{@const entry = getEntryForSlot(day.value, period.id)}

											<!-- Cell Area -->
											<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
											<td
												class="border p-1 align-top h-[110px] min-w-[140px] relative transition-colors hover:bg-muted/10"
											>
												{#if entry}
													<!-- Filled Slot -->
													<div
														draggable="true"
														ondragstart={(e) => handleDragStart(e, entry, 'MOVE')}
														ondragend={handleDragEnd}
														class="h-full w-full bg-blue-50 border border-blue-200 rounded p-2 relative group flex flex-col cursor-move hover:shadow-md transition-all mobile-draggable"
														role="button"
														tabindex="0"
													>
														<div class="font-bold text-blue-900">{entry.subject_code}</div>
														<div class="text-xs text-blue-700 line-clamp-2 mt-1 flex-1">
															{entry.subject_name_th}
														</div>
														{#if entry.instructor_name}
															<div class="text-xs text-blue-600 mt-1 flex items-center gap-1">
																<span class="w-1.5 h-1.5 rounded-full bg-blue-400"></span>
																{entry.instructor_name}
															</div>
														{/if}

														<button
															onclick={() => handleDeleteEntry(entry.id)}
															class="absolute top-1 right-1 opacity-100 sm:opacity-0 group-hover:opacity-100 p-1 hover:bg-red-100 rounded text-red-500 transition-all cursor-pointer z-10"
															title="ลบ"
														>
															<Trash2 class="w-3.5 h-3.5" />
														</button>
													</div>
												{:else}
													<!-- Empty Drop Zone -->
													<div
														role="region"
														aria-label="Drop zone"
														ondragover={handleDragOver}
														ondrop={(e) => handleDrop(e, day.value, period.id)}
														class="h-full w-full flex items-center justify-center border-2 border-dashed border-transparent hover:border-blue-300 rounded transition-all"
													>
														<span
															class="text-xs text-muted-foreground opacity-20 pointer-events-none"
															>ว่าง</span
														>
													</div>
												{/if}
											</td>
										{/each}
									</tr>
								{/each}
							{/if}
						</tbody>
					</table>
				</div>
			</div>
		</div>
	{/if}
</div>

<style>
    /* Custom Scrollbar */
    ::-webkit-scrollbar {
        width: 8px;
        height: 8px;
    }
    ::-webkit-scrollbar-track {
        background: transparent;
    }
    ::-webkit-scrollbar-thumb {
        background: #e2e8f0;
        border-radius: 4px;
    }
    ::-webkit-scrollbar-thumb:hover {
        background: #cbd5e1;
    }

    /* Mobile handling */
    @media (max-width: 768px) {
		.mobile-draggable {
			touch-action: none;
			-webkit-user-select: none;
			user-select: none;
		}

		/* Helper class added by polyfill */
		:global(.dnd-poly-drag-image) {
			opacity: 0.8 !important;
            transform: scale(1.05);
		}
	}
</style>
