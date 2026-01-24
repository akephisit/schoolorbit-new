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
		listPeriods,
        createBatchTimetableEntries
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
		Users,
        PlusCircle
	} from 'lucide-svelte';
    
    import { Checkbox } from '$lib/components/ui/checkbox';
    
    import { getAcademicStructure } from '$lib/api/academic';
    import type { AcademicYear, Semester } from '$lib/api/academic';
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
	let academicYears = $state<AcademicYear[]>([]);
    let allSemesters = $state<Semester[]>([]);
	let rooms = $state<Room[]>([]);
    let instructors = $state<StaffLookupItem[]>([]);

    // View Mode: 'CLASSROOM' or 'INSTRUCTOR'
    let viewMode = $state<'CLASSROOM' | 'INSTRUCTOR'>('CLASSROOM');

	let selectedYearId = $state('');
    let selectedSemesterId = $state('');
	let selectedClassroomId = $state('');
    let selectedInstructorId = $state('');
    
    // Derived Semesters based on selected year
    let semesters = $derived(allSemesters.filter(s => s.academic_year_id === selectedYearId));

	// Drag & Drop state
	let draggedCourse = $state<any>(null);
	let submitting = $state(false);

	async function loadInitialData() {
		try {
			loading = true;
			const structureRes = await getAcademicStructure();
            academicYears = structureRes.data.years;
            allSemesters = structureRes.data.semesters;

            // Find active year
            const activeYear = academicYears.find((y) => y.is_active) || academicYears[0];
            
			if (activeYear) {
				selectedYearId = activeYear.id;
                
                // Active semester
                const yearSemesters = allSemesters.filter(s => s.academic_year_id === activeYear.id);
                const activeSemester = yearSemesters.find(s => s.is_active) || yearSemesters[0];
                if (activeSemester) selectedSemesterId = activeSemester.id;

				await Promise.all([
                    loadClassrooms(),
                    loadRooms(),
                    loadInstructors()
                ]);
			}
		} catch (e) {
            console.error(e);
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
			    res = await listClassroomCourses({ classroomId: selectedClassroomId, semesterId: selectedSemesterId });
            } else {
                res = await listClassroomCourses({ instructorId: selectedInstructorId, semesterId: selectedSemesterId });
            }
			courses = res.data;
		} catch (e) {
			console.error(e);
            toast.error('โหลดรายวิชาไม่สำเร็จ');
		}
	}

	async function loadTimetable() {
        if (viewMode === 'CLASSROOM' && !selectedClassroomId) {
			timetableEntries = [];
			return;
		}
        if (viewMode === 'INSTRUCTOR' && !selectedInstructorId) {
			timetableEntries = [];
			return;
		}

		try {
            let res;
            if (viewMode === 'CLASSROOM') {
			    res = await listTimetableEntries({ classroom_id: selectedClassroomId, academic_semester_id: selectedSemesterId });
            } else {
                res = await listTimetableEntries({ instructor_id: selectedInstructorId, academic_semester_id: selectedSemesterId });
            }
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
        // In Instructor View, we already see the instructor's full schedule.
        // No need to check for conflicts specifically (visual redundancy).
        if (viewMode === 'INSTRUCTOR') {
            occupiedSlots = new Set();
            return;
        }

		const instructorId = course.primary_instructor_id;
		if (!instructorId) return;

		try {
			const res = await listTimetableEntries({ instructor_id: instructorId });
			const conflicts = new Set<string>();
			res.data.forEach(entry => {
				// Don't mark as conflict if it's the entry being moved itself
				if (dragType === 'MOVE' && entry.id === draggedEntryId) return;
				
                // Don't mark as conflict if it's a course from the current classroom
                // (It's already shown in the grid as a schedule, not an external conflict)
                if (viewMode === 'CLASSROOM') {
                    const isCurrentClassroomEntry = courses.some(c => c.id === entry.classroom_course_id);
                    if (isCurrentClassroomEntry) return;
                }

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
        // Check availability for this slot
        updateUnavailableRooms(day, periodId);
	}

    let unavailableRooms = $state<Set<string>>(new Set());
    let loadingRoomsAvailability = $state(false);

    async function updateUnavailableRooms(day: string, periodId: string) {
        loadingRoomsAvailability = true;
        unavailableRooms = new Set(); // Reset (reactivity fix: assign new Set)
        try {
            // Fetch schedule for the whole day across the school to check room usage
            const res = await listTimetableEntries({ 
                day_of_week: day, 
                academic_semester_id: selectedSemesterId 
            });
            
            const busyRooms = new Set<string>();
            res.data.forEach(entry => {
                if (entry.period_id === periodId && entry.room_id) {
                    // If moving existing entry, don't count itself as busy
                    if (dragType === 'MOVE' && entry.id === draggedEntryId) return;
                    busyRooms.add(entry.room_id);
                }
            });
            unavailableRooms = busyRooms;
        } catch(e) {
            console.error("Failed to check room availability", e);
        } finally {
            loadingRoomsAvailability = false;
        }
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
				const courseName = course.subject_code || course.title || 'รายการ';

				const payload = {
					day_of_week: day,
					period_id: periodId,
                    room_id: roomId
				};

				const res = await updateTimetableEntry(entryId, payload);
				handleResponse(res, `ย้าย ${courseName} สำเร็จ`);
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
            if (entry.classroom_course_id) {
			    const count = courseCounts.get(entry.classroom_course_id) || 0;
			    courseCounts.set(entry.classroom_course_id, count + 1);
            }
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
            
            // Auto-select semester when year changes (and clear if none)
            const yearSemesters = allSemesters.filter(s => s.academic_year_id === selectedYearId);
            if (!yearSemesters.find(s => s.id === selectedSemesterId)) {
                 const activeOrFirst = yearSemesters.find(s => s.is_active) || yearSemesters[0];
                 selectedSemesterId = activeOrFirst ? activeOrFirst.id : '';
            }
		}
	});

	$effect(() => {
        // Reload when semester changes or view selection changes
		if (viewMode === 'CLASSROOM' && selectedClassroomId) {
			loadCourses();
			loadTimetable();
		} else if (viewMode === 'INSTRUCTOR' && selectedInstructorId) {
            loadCourses();
            loadTimetable();
        }
	});
    
    // Add semester reload effect
    $effect(() => {
        if (selectedSemesterId) {
             // Just triggering the above effect is enough if dependencies are correct. 
             // But above effect only listens on viewMode/selection IDs (and implicitly functions closure?)
             // In Svelte 5, we should include all dependencies or call load directly
             if ((viewMode === 'CLASSROOM' && selectedClassroomId) || (viewMode === 'INSTRUCTOR' && selectedInstructorId)) {
                 loadCourses();
                 loadTimetable();
             }
        }
    });

    // Batch Assign State
    let showBatchModal = $state(false);
    let batchClassrooms = $state<string[]>([]);
    let batchDay = $state('MON');
    let batchPeriodId = $state('');
    let batchType = $state('ACTIVITY');
    let batchTitle = $state('');
    let batchRoomId = $state('none');

    function toggleBatchClassroom(id: string) {
        if (batchClassrooms.includes(id)) {
            batchClassrooms = batchClassrooms.filter(c => c !== id);
        } else {
            batchClassrooms = [...batchClassrooms, id];
        }
    }
    
    function selectAllBatchClassrooms() {
        if (batchClassrooms.length === classrooms.length) {
            batchClassrooms = [];
        } else {
            batchClassrooms = classrooms.map(c => c.id);
        }
    }

    async function handleBatchSubmit() {
        if (batchClassrooms.length === 0) {
            toast.error('กรุณาเลือกห้องเรียนอย่างน้อย 1 ห้อง');
            return;
        }
        if (!batchPeriodId) {
            toast.error('กรุณาเลือกคาบเวลา');
            return;
        }
        if (!batchTitle) {
            toast.error('กรุณาระบุชื่อกิจกรรม');
            return;
        }

        try {
            submitting = true;
            await createBatchTimetableEntries({
                classroom_ids: batchClassrooms,
                day_of_week: batchDay,
                period_id: batchPeriodId,
                academic_semester_id: selectedSemesterId,
                entry_type: batchType as any,
                title: batchTitle,
                room_id: batchRoomId === 'none' ? undefined : batchRoomId
            });
            
            toast.success('บันทึกกิจกรรมเรียบร้อย');
            showBatchModal = false;
            // Reset minimal fields
            batchTitle = '';
            
            // Reload if current view is affected
            if (viewMode === 'CLASSROOM' && selectedClassroomId && batchClassrooms.includes(selectedClassroomId)) {
                loadTimetable();
            }
        } catch(e: any) {
            toast.error(e.message || 'บันทึกไม่สำเร็จ');
        } finally {
            submitting = false;
        }
    }

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
		<!-- View Mode Switcher & Tools -->
		<div class="flex items-center justify-between">
			<div class="flex bg-muted p-1 rounded-lg w-fit transition-colors">
				<button
					class="px-3 py-1 text-sm font-medium rounded transition-all flex items-center gap-2 {viewMode ===
					'CLASSROOM'
						? 'bg-background shadow-sm text-foreground'
						: 'text-muted-foreground hover:text-foreground'}"
					onclick={() => {
						viewMode = 'CLASSROOM';
						courses = [];
						timetableEntries = [];
						// Do not reset selectedInstructorId so we can return to it
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
						courses = [];
						timetableEntries = [];
						// Do not reset selectedClassroomId so we can return to it
					}}
				>
					<Users class="w-4 h-4" /> ครูผู้สอน
				</button>
			</div>

			<Button variant="outline" size="sm" onclick={() => (showBatchModal = true)}>
				<PlusCircle class="w-4 h-4 mr-2" /> เพิ่มกิจกรรมพิเศษ (Batch)
			</Button>
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

			<div class="w-[200px]">
				<Select.Root type="single" bind:value={selectedSemesterId} disabled={!selectedYearId}>
					<Select.Trigger class="w-full">
						{semesters.find((s) => s.id === selectedSemesterId)?.name || 'เลือกภาคเรียน'}
					</Select.Trigger>
					<Select.Content>
						{#each semesters as semester}
							<Select.Item value={semester.id}>{semester.name}</Select.Item>
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
			<div class="col-span-2 flex flex-col h-full bg-background rounded-lg border overflow-hidden">
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
			<div class="col-span-10 flex flex-col h-full overflow-hidden border rounded-lg">
				<div class="overflow-auto h-full relative">
					<table class="w-full text-sm border-collapse">
						<thead class="bg-muted/50 sticky top-0 z-20 shadow-sm">
							<tr>
								<th class="p-2 border sticky left-0 z-30 bg-background w-[100px] text-center"
									>วัน/คาบ</th
								>
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
										<td
											class="p-2 border font-bold sticky left-0 z-10 bg-background text-center align-middle"
										>
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
														{#if entry.entry_type && entry.entry_type !== 'COURSE'}
															<div
																class="flex-1 flex flex-col items-center justify-center p-1 text-center w-full"
															>
																<div
																	class="font-bold text-sm mb-1 px-2 py-0.5 rounded-md w-full truncate
                                                                    {entry.entry_type === 'BREAK'
																		? 'bg-pink-50 text-pink-700 border border-pink-100'
																		: entry.entry_type === 'HOMEROOM'
																			? 'bg-purple-50 text-purple-700 border border-purple-100'
																			: 'bg-green-50 text-green-700 border border-green-100'}"
																>
																	{entry.title || entry.entry_type}
																</div>

																{#if entry.room_id}
																	{@const roomName =
																		entry.room_code ||
																		rooms.find((r) => r.id === entry.room_id)?.name_th}
																	{#if roomName}
																		<div
																			class="text-[10px] text-slate-500 mt-1 flex items-center justify-center gap-1 font-medium w-full truncate"
																		>
																			<MapPin class="w-3 h-3 flex-shrink-0" />
																			<span class="truncate">{roomName}</span>
																		</div>
																	{/if}
																{/if}
															</div>
														{:else}
															<div class="font-bold text-blue-900">{entry.subject_code}</div>
															<div class="text-xs text-blue-700 line-clamp-2 mt-1 flex-1">
																{entry.subject_name_th}
															</div>

															{#if viewMode === 'INSTRUCTOR' && entry.classroom_name}
																<div
																	class="text-[10px] text-orange-600 font-medium mt-1 bg-orange-50 px-1 rounded border border-orange-100 w-fit truncate max-w-full"
																>
																	สอน: {entry.classroom_name}
																</div>
															{/if}

															{#if entry.instructor_name && viewMode === 'CLASSROOM'}
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
								{#if !unavailableRooms.has(room.id)}
									<Select.Item value={room.id}>
										{room.name_th}
										{room.room_type ? `(${room.room_type})` : ''}
									</Select.Item>
								{/if}
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

<!-- Batch Assign Modal -->
<Dialog.Root bind:open={showBatchModal}>
	<Dialog.Content class="sm:max-w-[500px] max-h-[90vh] overflow-y-auto">
		<Dialog.Header>
			<Dialog.Title>เพิ่มกิจกรรมพิเศษ (Batch)</Dialog.Title>
			<Dialog.Description>
				กำหนดกิจกรรม (เช่น พัก, โฮมรูม) ให้กับหลายห้องเรียนพร้อมกัน
			</Dialog.Description>
		</Dialog.Header>

		<div class="grid gap-4 py-4">
			<!-- Details -->
			<div class="grid grid-cols-4 items-center gap-4">
				<Label.Root class="text-right">หัวข้อ *</Label.Root>
				<div class="col-span-3">
					<input
						class="flex h-9 w-full rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-sm transition-colors placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50"
						bind:value={batchTitle}
						placeholder="เช่น พักเที่ยง, กิจกรรมพัฒนาผู้เรียน"
					/>
				</div>
			</div>

			<div class="grid grid-cols-4 items-center gap-4">
				<Label.Root class="text-right">ประเภท</Label.Root>
				<div class="col-span-3">
					<Select.Root type="single" bind:value={batchType}>
						<Select.Trigger class="w-full">
							{batchType === 'BREAK' ? 'พัก' : batchType === 'HOMEROOM' ? 'โฮมรูม' : 'กิจกรรม'}
						</Select.Trigger>
						<Select.Content>
							<Select.Item value="ACTIVITY">กิจกรรม</Select.Item>
							<Select.Item value="BREAK">พักเบรค/พักเที่ยง</Select.Item>
							<Select.Item value="HOMEROOM">โฮมรูม</Select.Item>
						</Select.Content>
					</Select.Root>
				</div>
			</div>

			<!-- Time -->
			<div class="grid grid-cols-4 items-center gap-4">
				<Label.Root class="text-right">วัน/คาบ *</Label.Root>
				<div class="col-span-3 flex gap-2">
					<Select.Root type="single" bind:value={batchDay}>
						<Select.Trigger class="w-[100px]">
							{DAYS.find((d) => d.value === batchDay)?.label || batchDay}
						</Select.Trigger>
						<Select.Content>
							{#each DAYS as d}
								<Select.Item value={d.value}>{d.label}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>

					<Select.Root type="single" bind:value={batchPeriodId}>
						<Select.Trigger class="flex-1">
							{periods.find((p) => p.id === batchPeriodId)?.name || 'เลือกคาบ'}
						</Select.Trigger>
						<Select.Content class="max-h-[200px] overflow-y-auto">
							{#each periods as p}
								<Select.Item value={p.id}
									>{p.name} ({formatTime(p.start_time)}-{formatTime(p.end_time)})</Select.Item
								>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
			</div>

			<!-- Room (Optional) -->
			<div class="grid grid-cols-4 items-center gap-4">
				<Label.Root class="text-right">ห้อง (ถ้ามี)</Label.Root>
				<div class="col-span-3">
					<Select.Root type="single" bind:value={batchRoomId}>
						<Select.Trigger class="w-full">
							{rooms.find((r) => r.id === batchRoomId)?.name_th ||
								(batchRoomId === 'none' ? 'ไม่ระบุห้อง' : 'เลือกห้อง')}
						</Select.Trigger>
						<Select.Content class="max-h-[200px] overflow-y-auto">
							<Select.Item value="none" class="text-muted-foreground">ไม่ระบุห้อง</Select.Item>
							{#each rooms as room}
								<Select.Item value={room.id}>
									{room.name_th}
								</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
			</div>

			<!-- Classrooms Selection -->
			<div class="border-t pt-4 mt-2">
				<div class="flex justify-between items-center mb-2">
					<Label.Root>เลือกห้องเรียน ({batchClassrooms.length})</Label.Root>
					<Button variant="ghost" size="sm" class="h-6 text-xs" onclick={selectAllBatchClassrooms}>
						{batchClassrooms.length === classrooms.length ? 'ยกเลิกทั้งหมด' : 'เลือกทั้งหมด'}
					</Button>
				</div>
				<div
					class="border rounded-md max-h-[200px] overflow-y-auto p-2 bg-muted/20 grid grid-cols-2 gap-2"
				>
					{#each classrooms as classroom}
						<div class="flex items-center space-x-2">
							<Checkbox
								id="batch-class-{classroom.id}"
								checked={batchClassrooms.includes(classroom.id)}
								onCheckedChange={() => toggleBatchClassroom(classroom.id)}
							/>
							<label
								for="batch-class-{classroom.id}"
								class="text-sm font-medium leading-none peer-disabled:cursor-not-allowed peer-disabled:opacity-70 cursor-pointer"
							>
								{classroom.name}
							</label>
						</div>
					{/each}
				</div>
			</div>
		</div>

		<Dialog.Footer>
			<Button variant="outline" onclick={() => (showBatchModal = false)}>ยกเลิก</Button>
			<Button onclick={handleBatchSubmit} disabled={submitting}>
				{#if submitting}
					<Loader2 class="mr-2 h-4 w-4 animate-spin" />
				{/if}
				บันทึก
			</Button>
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
