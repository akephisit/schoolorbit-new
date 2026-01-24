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
	import { listRooms, type Room } from '$lib/api/facility';
    import * as Dialog from '$lib/components/ui/dialog';
    import * as Label from '$lib/components/ui/label';

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
		BookOpen,
		MapPin,
		Users
	} from 'lucide-svelte';

    import { lookupStaff, type StaffLookupItem } from '$lib/api/lookup';

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
	let rooms = $state<Room[]>([]);
    let instructors = $state<StaffLookupItem[]>([]);

    // View Mode: 'CLASSROOM' or 'INSTRUCTOR'
    let viewMode = $state<'CLASSROOM' | 'INSTRUCTOR'>('CLASSROOM');

	let selectedYearId = $state('');
	let selectedClassroomId = $state('');
    let selectedInstructorId = $state('');

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
				await Promise.all([
                    loadClassrooms(),
                    loadRooms(),
                    loadInstructors()
                ]);
			}
		} catch (e) {
			toast.error('โหลดข้อมูลไม่สำเร็จ');
		} finally {
			loading = false;
		}
	}
    
    async function loadInstructors() {
        try {
            // Fetch staff for dropdown (safer, only needs authenticated user)
            const data = await lookupStaff({ limit: 500 }); 
            instructors = data;
        } catch(e) {
            console.error('Failed to load instructors', e);
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

    async function loadRooms() {
        try {
            const res = await listRooms();
            rooms = res.data;
        } catch (e) {
            console.error('Failed to load rooms', e);
        }
    }

	async function loadCourses() {
        // Mode: CLASSROOM
		if (viewMode === 'CLASSROOM' && !selectedClassroomId) return;
        // Mode: INSTRUCTOR
        if (viewMode === 'INSTRUCTOR' && !selectedInstructorId) return;

		try {
            let res;
            if (viewMode === 'CLASSROOM') {
			    res = await listClassroomCourses(selectedClassroomId);
            } else {
                res = await listClassroomCourses({ instructorId: selectedInstructorId });
            }
			courses = res.data;
		} catch (e) {
			console.error(e);
            toast.error('โหลดรายวิชาไม่สำเร็จ');
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
	
	// Room Selection State
	let showRoomModal = $state(false);
	// Store all necessary context because drag state is cleared on dragend
	let pendingDropContext = $state<{
		day: string;
		periodId: string;
		dragType: 'NEW' | 'MOVE';
		course: any;      // The item being dragged
		entryId: string | null;
	} | null>(null);
	
	let selectedRoomId = $state<string>(''); // empty string = no room (default)

	// Availability State
	let occupiedSlots = $state<Set<string>>(new Set()); // Format: "DAY_PERIODID"
	
	function getSlotKey(day: string, periodId: string) {
		return `${day}_${periodId}`;
	}

	function isSlotOccupiedByInstructor(day: string, periodId: string) {
		return occupiedSlots.has(getSlotKey(day, periodId));
	}

	async function fetchInstructorConflicts(course: any) {
		const instructorId = course.primary_instructor_id;
		if (!instructorId) return;

		try {
			const res = await listTimetableEntries({ instructor_id: instructorId });
			const conflicts = new Set<string>();
			res.data.forEach(entry => {
				// Don't mark as conflict if it's the entry being moved itself
				if (dragType === 'MOVE' && entry.id === draggedEntryId) return;
				
				conflicts.add(getSlotKey(entry.day_of_week, entry.period_id));
			});
			occupiedSlots = conflicts;
		} catch (e) {
			console.error('Failed to check conflicts', e);
		}
	}

	function handleDragStart(event: DragEvent, item: any, type: 'NEW' | 'MOVE') {
		dragType = type;
		
		// Determine course object to check instructor
		let courseToCheck: any = null;

		if (type === 'NEW') {
			draggedCourse = item;
			draggedEntryId = null;
			courseToCheck = item;
		} else {
			draggedCourse = item; 
			draggedEntryId = item.id;
			
			const originalCourse = courses.find(c => c.id === item.classroom_course_id);
			courseToCheck = originalCourse || item; 
		}

		// Fetch availability
		if (courseToCheck) {
			fetchInstructorConflicts(courseToCheck);
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
		occupiedSlots = new Set(); // Clear highlights
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
            handleDragEnd();
			return;
		}

		// Check instructor availability
		if (isSlotOccupiedByInstructor(day, periodId)) {
			toast.error('ครูติดสอนในคาบนี้แล้ว');
			handleDragEnd();
			return;
		}

        // Open Room Selection Modal instead of saving immediately
        // Store context because drag state (draggedCourse) will be cleared by ondragend
        pendingDropContext = {
            day,
            periodId,
            dragType,
            course: draggedCourse,
            entryId: draggedEntryId
        };
        
        // Pre-select room if moving existing entry
        if (dragType === 'MOVE' && draggedCourse.room_id) {
            selectedRoomId = draggedCourse.room_id;
        } else {
            selectedRoomId = 'none'; // Default to 'no room'
        }
        
        showRoomModal = true;
	}

    async function confirmDropWithRoom() {
        if (!pendingDropContext) return;
        
        const { day, periodId, dragType, course, entryId } = pendingDropContext;
        // Use undefined instead of null to match interface
        const roomId = selectedRoomId === 'none' ? undefined : selectedRoomId;
        
        showRoomModal = false; // Close modal

		try {
			submitting = true;

			if (dragType === 'NEW') {
				// CREATE NEW
				const courseCode = course.subject_code;
				const payload: any = { 
					classroom_course_id: course.id,
					day_of_week: day,
					period_id: periodId,
                    room_id: roomId
				};
				
				const res = await createTimetableEntry(payload);
				handleResponse(res, `ลงตาราง ${courseCode} สำเร็จ`);

			} else if (dragType === 'MOVE' && entryId) {
				// UPDATE EXISTING (MOVE)
				const courseCode = course.subject_code;

				const payload = {
					day_of_week: day,
					period_id: periodId,
                    room_id: roomId
				};

				const res = await updateTimetableEntry(entryId, payload);
				handleResponse(res, `ย้าย ${courseCode} สำเร็จ`);
			}

		} catch (e: any) {
			toast.error(e.message || 'บันทึกไม่สำเร็จ');
		} finally {
			submitting = false;
			// handleDragEnd(); // already called by system
            pendingDropContext = null;
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
		if (viewMode === 'CLASSROOM' && selectedClassroomId) {
			loadCourses();
			loadTimetable();
		} else if (viewMode === 'INSTRUCTOR' && selectedInstructorId) {
            loadCourses();
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

	<!-- Filters & View Mode -->
	<div class="flex flex-col gap-4">
		<!-- View Mode Switcher -->
		<div class="flex bg-muted p-1 rounded-lg w-fit">
			<button
				class="px-3 py-1 text-sm font-medium rounded transition-all flex items-center gap-2 {viewMode ===
				'CLASSROOM'
					? 'bg-background shadow-sm text-foreground'
					: 'text-muted-foreground hover:text-foreground'}"
				onclick={() => {
					viewMode = 'CLASSROOM';
					selectedInstructorId = '';
					courses = [];
					timetableEntries = [];
				}}
			>
				<School class="w-4 h-4" /> ห้องเรียน
			</button>
			<button
				class="px-3 py-1 text-sm font-medium rounded transition-all flex items-center gap-2 {viewMode ===
				'INSTRUCTOR'
					? 'bg-background shadow-sm text-foreground'
					: 'text-muted-foreground hover:text-foreground'}"
				onclick={() => {
					viewMode = 'INSTRUCTOR';
					selectedClassroomId = '';
					courses = [];
					timetableEntries = [];
				}}
			>
				<Users class="w-4 h-4" /> ครูผู้สอน
			</button>
		</div>

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

			<div class="w-[280px]">
				{#if viewMode === 'CLASSROOM'}
					<Select.Root type="single" bind:value={selectedClassroomId}>
						<Select.Trigger class="w-full">
							<School class="w-4 h-4 mr-2" />
							{classrooms.find((c) => c.id === selectedClassroomId)?.name || 'เลือกห้องเรียน'}
						</Select.Trigger>
						<Select.Content class="max-h-[300px] overflow-y-auto">
							{#each classrooms as classroom}
								<Select.Item value={classroom.id}>{classroom.name}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				{:else}
					<Select.Root type="single" bind:value={selectedInstructorId}>
						<Select.Trigger class="w-full">
							<Users class="w-4 h-4 mr-2" />
							{instructors.find((i) => i.id === selectedInstructorId)?.name || 'เลือกครูผู้สอน'}
						</Select.Trigger>
						<Select.Content class="max-h-[300px] overflow-y-auto">
							<div class="p-2 sticky top-0 bg-background z-10 border-b mb-1">
								<span class="text-xs text-muted-foreground font-medium">รายชื่อบุคลากรทั้งหมด</span>
							</div>
							{#each instructors as staff}
								<Select.Item value={staff.id} label={staff.name}>
									{staff.title || ''}{staff.name}
								</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				{/if}
			</div>
		</div>
	</div>

	{#if (!selectedClassroomId && viewMode === 'CLASSROOM') || (!selectedInstructorId && viewMode === 'INSTRUCTOR')}
		<Card.Root>
			<Card.Content class="py-12 text-center">
				{#if viewMode === 'CLASSROOM'}
					<School class="w-16 h-16 mx-auto text-muted-foreground mb-4" />
					<p class="text-muted-foreground">กรุณาเลือกห้องเรียนเพื่อดูและจัดตารางสอน</p>
				{:else}
					<Users class="w-16 h-16 mx-auto text-muted-foreground mb-4" />
					<p class="text-muted-foreground">กรุณาเลือกครูผู้สอนเพื่อดูภาระงานและจัดการสอน</p>
				{/if}
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

								{#if viewMode === 'INSTRUCTOR' && course.classroom_name}
									<div
										class="text-[10px] items-center gap-1 flex text-amber-700 bg-amber-50 px-1.5 py-0.5 rounded border border-amber-100 mt-1 w-fit"
									>
										<School class="w-3 h-3" />
										{course.classroom_name}
									</div>
								{/if}

								{#if course.instructor_name && viewMode === 'CLASSROOM'}
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
											{@const isOccupied = isSlotOccupiedByInstructor(day.value, period.id)}

											<!-- Cell Area -->
											<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
											<td
												class="border p-1 align-top h-[110px] min-w-[140px] relative transition-colors {isOccupied
													? 'bg-gray-50'
													: 'hover:bg-muted/10'}"
											>
												{#if isOccupied && draggedCourse}
													<div
														class="absolute inset-0 z-20 flex items-center justify-center bg-gray-200/50 cursor-not-allowed pointer-events-none border border-red-200 m-1 rounded-lg"
													>
														<span
															class="text-[10px] text-red-600 font-bold bg-white/90 px-1.5 py-0.5 rounded shadow-sm border border-red-100"
														>
															ครูติดสอน
														</span>
													</div>
												{/if}
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

														<!-- Room Display -->
														{#if entry.room_id}
															{@const roomName =
																entry.room_code ||
																rooms.find((r) => r.id === entry.room_id)?.name_th}
															{#if roomName}
																<div
																	class="text-[10px] text-slate-500 mt-1 flex items-center gap-1 font-medium bg-white/60 px-1.5 py-0.5 rounded border border-slate-100 w-fit max-w-full truncate"
																>
																	<MapPin class="w-3 h-3 flex-shrink-0" />
																	<span class="truncate">{roomName}</span>
																</div>
															{/if}
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

<!-- Room Selection Modal -->
<Dialog.Root bind:open={showRoomModal}>
	<Dialog.Content class="sm:max-w-[425px]">
		<Dialog.Header>
			<Dialog.Title>เลือกห้องเรียน (สถานที่เรียน)</Dialog.Title>
			<Dialog.Description>
				กรุณาเลือกห้องที่ใช้สอนสำหรับคาบนี้ (สามารถเว้นว่างได้)
			</Dialog.Description>
		</Dialog.Header>
		<div class="grid gap-4 py-4">
			<div class="grid grid-cols-4 items-center gap-4">
				<Label.Root for="room" class="text-right">ห้อง</Label.Root>
				<div class="col-span-3">
					<Select.Root type="single" bind:value={selectedRoomId}>
						<Select.Trigger class="w-full">
							{rooms.find((r) => r.id === selectedRoomId)?.name_th ||
								(selectedRoomId === 'none' ? 'ไม่ระบุห้อง' : 'เลือกห้อง')}
						</Select.Trigger>
						<Select.Content class="max-h-[200px] overflow-y-auto">
							<Select.Item value="none" class="text-muted-foreground">ไม่ระบุห้อง</Select.Item>
							{#each rooms as room}
								<Select.Item value={room.id}
									>{room.name_th} {room.room_type ? `(${room.room_type})` : ''}</Select.Item
								>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
			</div>
			{#if rooms.length === 0}
				<div class="bg-yellow-50 text-yellow-800 text-xs p-2 rounded border border-yellow-200">
					<span class="font-bold">คำแนะนำ:</span> ยังไม่มีข้อมูลห้องเรียนในระบบ คุณสามารถไปเพิ่มห้องเรียนได้ที่เมนู
					"ข้อมูลอาคารสถานที่"
				</div>
			{/if}
		</div>
		<Dialog.Footer>
			<Button
				variant="outline"
				onclick={() => {
					showRoomModal = false;
					handleDragEnd();
				}}>ยกเลิก</Button
			>
			<Button onclick={confirmDropWithRoom}>ยืนยัน</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

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
