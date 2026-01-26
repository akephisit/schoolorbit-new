<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
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
		type Classroom,
        getAcademicStructure,
        type AcademicYear, 
        type Semester
	} from '$lib/api/academic';
	import { lookupRooms, type RoomLookupItem, lookupStaff, type StaffLookupItem, lookupSubjects, type LookupItem, lookupGradeLevels, type GradeLevelLookupItem } from '$lib/api/lookup';
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
		School,
		BookOpen,
		Users,
        PlusCircle, MapPin
	} from 'lucide-svelte';
    
    import { Checkbox } from '$lib/components/ui/checkbox';
    
    import { authStore } from '$lib/stores/auth';
    import { 
        connectTimetableSocket, 
        disconnectTimetableSocket, 
        sendTimetableEvent,
        activeUsers,
        remoteCursors,
        userDrags,
        refreshTrigger,
        isConnected
    } from '$lib/stores/timetable-socket';

    let { data }: { data: any } = $props();

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
	let rooms = $state<RoomLookupItem[]>([]);
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
	let isDropPending = $state(false);
    // Identify what is being dragged: 'NEW' (from list) | 'MOVE' (from grid)
	let dragType = $state<'NEW' | 'MOVE'>('NEW');
	let draggedEntryId = $state<string | null>(null);

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
			periods = res.data;
		} catch (e) {
			console.error(e);
		}
	}

    async function loadRooms() {
        try {
            const res = await lookupRooms();
            rooms = res;
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
            
            // Notify others to refresh
            if ($authStore.user) {
                sendTimetableEvent({ type: 'TableRefresh', payload: { user_id: $authStore.user.id } });
            }

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

	
	
	// Room Selection State
	let showRoomModal = $state(false);
	let pendingDropContext = $state<{
		day: string;
		periodId: string;
		dragType: 'NEW' | 'MOVE';
		course: any;
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
        let conflicts = new Set<string>();

        // 1. INSTRUCTOR VIEW
        if (viewMode === 'INSTRUCTOR') {
            const classroomId = course.classroom_id;
            if (!classroomId) return;

            try {
                const res = await listTimetableEntries({ classroom_id: classroomId, academic_semester_id: selectedSemesterId });
                res.data.forEach(entry => {
                    if (dragType === 'MOVE' && entry.id === draggedEntryId) return;
                    
                    const isMyCourse = courses.some(c => c.id === entry.classroom_course_id);
                    if (isMyCourse) return;

                    conflicts.add(getSlotKey(entry.day_of_week, entry.period_id));
                });
                occupiedSlots = conflicts;
            } catch (e) {
                console.error('Failed to check conflicts', e);
            }
            return;
        }

        // 2. CLASSROOM VIEW
		const instructorId = course.primary_instructor_id;
		if (!instructorId) {
            occupiedSlots = new Set();
            return;
        }

		try {
			const res = await listTimetableEntries({ instructor_id: instructorId, academic_semester_id: selectedSemesterId });
			res.data.forEach(entry => {
				if (dragType === 'MOVE' && entry.id === draggedEntryId) return;
				
                if (viewMode === 'CLASSROOM') {
                    if (entry.classroom_id === selectedClassroomId) return;
                }

				conflicts.add(getSlotKey(entry.day_of_week, entry.period_id));
			});
			occupiedSlots = conflicts;
		} catch (e) {
			console.error('Failed to check conflicts', e);
		}
	}

	function createDragImage(text: string, subtext: string) {
        const div = document.createElement('div');
        div.className = 'fixed top-[-1000px] left-[-1000px] bg-white border border-primary/50 shadow-xl rounded-lg p-3 w-[180px] z-[9999] flex flex-col gap-1';
        div.innerHTML = `
            <div class="flex items-center gap-2">
                <div class="p-1 rounded bg-primary/10 text-primary">
                    <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M2 3h6a4 4 0 0 1 4 4v14a3 3 0 0 0-3-3H2z"/><path d="M22 3h-6a4 4 0 0 0-4 4v14a3 3 0 0 1 3-3h7z"/></svg>
                </div>
                <span class="font-bold text-sm truncate text-primary">${text}</span>
            </div>
            <div class="text-xs text-muted-foreground truncate pl-1">${subtext}</div>
        `;
        document.body.appendChild(div);
        return div;
    }

	function handleDragStart(event: DragEvent, item: any, type: 'NEW' | 'MOVE') {
		dragType = type;
		
		let courseToCheck: any = null;

		if (type === 'NEW') {
			draggedCourse = item;
			draggedEntryId = null;
			courseToCheck = item;
		} else {
			draggedCourse = item; 
			draggedEntryId = item.id;
			
			const originalCourse = courses.find(c => c.id === item.classroom_course_id);
			courseToCheck = originalCourse || {
                ...item,
                id: item.classroom_course_id,
                subject_code: item.subject_code,
                title: item.subject_name_th,
                title_th: item.subject_name_th
            }; 
		}

		if (courseToCheck) {
			fetchInstructorConflicts(courseToCheck);
		}

		if (event.dataTransfer) {
			event.dataTransfer.effectAllowed = type === 'NEW' ? 'copy' : 'move';
			event.dataTransfer.setData('text/plain', JSON.stringify({ 
				type,
				id: type === 'NEW' ? item.id : item.id
			}));
            
            // Custom Drag Image
            const dragTitle = courseToCheck.subject_code || 'วิชา';
            const dragSub = courseToCheck.title_th || courseToCheck.title || '...';
            const dragElement = createDragImage(dragTitle, dragSub);
            event.dataTransfer.setDragImage(dragElement, 10, 10);
            
            setTimeout(() => document.body.removeChild(dragElement), 0);
		}

        // Notify others
        if ($authStore.user) {
            sendTimetableEvent({
                 type: 'DragStart',
                 payload: {
                     user_id: $authStore.user.id,
                     entry_id: draggedEntryId || undefined,
                     course_id: item.classroom_course_id || item.id,
                     info: {
                         code: courseToCheck.subject_code || '??',
                         title: courseToCheck.title_th || courseToCheck.title_en || courseToCheck.title || 'รายวิชา',
                         color: courseToCheck.color
                     }
                 }
            });
        }
    }

	function handleDragEnd() {
        if (isDropPending) return;

        if ($authStore.user) {
            sendTimetableEvent({ type: 'DragEnd', payload: { user_id: $authStore.user.id } });
        }
		draggedCourse = null;
		draggedEntryId = null;
		occupiedSlots = new Set(); 
	}

	function handleDragOver(event: DragEvent) {
		event.preventDefault(); 
		if (event.dataTransfer) {
			event.dataTransfer.dropEffect = dragType === 'NEW' ? 'copy' : 'move';
		}
	}

	async function handleDrop(event: DragEvent, day: string, periodId: string) {
		event.preventDefault();

		if (!draggedCourse) return;
        
        const existingEntry = getEntryForSlot(day, periodId);
		if (existingEntry) {
			toast.error('ช่องนี้มีรายการอยู่แล้ว');
            // Do not end drag yet if we want to retry? No, valid end if failed.
            // But let's call standard end.
            handleDragEnd();
			return;
		}

		if (isSlotOccupiedByInstructor(day, periodId)) {
			toast.error('ครูติดสอนในคาบนี้แล้ว');
			handleDragEnd();
			return;
		}

        pendingDropContext = {
            day,
            periodId,
            dragType,
            course: draggedCourse,
            entryId: draggedEntryId
        };
        
        if (dragType === 'MOVE' && draggedCourse.room_id) {
            selectedRoomId = draggedCourse.room_id;
        } else {
            selectedRoomId = 'none'; 
        }
        
        showRoomModal = true;
        isDropPending = true;
        updateUnavailableRooms(day, periodId);
	}

    let unavailableRooms = $state<Set<string>>(new Set());
    let loadingRoomsAvailability = $state(false);

    async function updateUnavailableRooms(day: string, periodId: string) {
        loadingRoomsAvailability = true;
        unavailableRooms = new Set(); 
        try {
            const res = await listTimetableEntries({ 
                day_of_week: day, 
                academic_semester_id: selectedSemesterId 
            });
            
            const busyRooms = new Set<string>();
            res.data.forEach(entry => {
                if (entry.period_id === periodId && entry.room_id) {
                    if (dragType === 'MOVE' && entry.id === draggedEntryId) return;
                    busyRooms.add(entry.room_id);
                }
            });
            unavailableRooms = busyRooms;
        } catch(e) {
            console.error(e);
        } finally {
            loadingRoomsAvailability = false;
        }
    }

    async function confirmDropWithRoom() {
        if (!pendingDropContext) return;
        
        const { day, periodId, dragType, course, entryId } = pendingDropContext;
        const roomId = selectedRoomId === 'none' ? undefined : selectedRoomId;
        
        showRoomModal = false;
        
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
				// UPDATE EXISTING
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
            // Notify others
            if ($authStore.user) {
                sendTimetableEvent({ type: 'TableRefresh', payload: { user_id: $authStore.user.id } });
            }
            
			submitting = false;
            pendingDropContext = null;
            isDropPending = false;
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
            if (entry.classroom_course_id) {
			    const count = courseCounts.get(entry.classroom_course_id) || 0;
			    courseCounts.set(entry.classroom_course_id, count + 1);
            }
		});

		return courses
			.map((course) => {
				const scheduled = courseCounts.get(course.id) || 0;
				const credit = course.subject_credit || 0;
				const hours = course.subject_hours || 0;
				// Default 20 weeks per semester
                // Priority: Hours > Credit > Default
				const maxPeriods = hours > 0 
                    ? Math.ceil(hours / 20)
                    : (credit > 0 ? Math.ceil(credit * 2) : 3); 
				
				return {
					...course,
					scheduled_count: scheduled,
					max_periods: maxPeriods,
					is_completed: scheduled >= maxPeriods
				};
			})
			.filter(course => !course.is_completed); 
	});

	$effect(() => {
		if (selectedYearId) {
			loadClassrooms();
			loadPeriods();
            
            const yearSemesters = allSemesters.filter(s => s.academic_year_id === selectedYearId);
            if (!yearSemesters.find(s => s.id === selectedSemesterId)) {
                 const activeOrFirst = yearSemesters.find(s => s.is_active) || yearSemesters[0];
                 selectedSemesterId = activeOrFirst ? activeOrFirst.id : '';
            }
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
    
    $effect(() => {
        if (selectedSemesterId) {
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
    
    // Batch Mode State
    let batchMode = $state<'TEXT' | 'COURSE'>('TEXT');
    let subjectOptions = $state<LookupItem[]>([]);
	let batchSubjectId = $state('');
    let loadingSubjects = $state(false);

    async function ensureSubjectsLoaded() {
        if (subjectOptions.length > 0) return;
        loadingSubjects = true;
        try {
            subjectOptions = await lookupSubjects({ limit: 500, activeOnly: true, subjectType: 'ACTIVITY' });
        } catch(e) {
            console.error(e);
            toast.error('โหลดรายวิชาไม่สำเร็จ');
        } finally {
            loadingSubjects = false;
        }
    }

    // Filter & Override State
    let batchGradeLevels = $state<GradeLevelLookupItem[]>([]);
	let batchGradeFilterId = $state('all');
    let batchForce = $state(false);

    async function loadBatchGradeLevels() {
        if (batchGradeLevels.length > 0) return;
        try {
            // Need to pass academic_year_id if possible, or just list all
            batchGradeLevels = await lookupGradeLevels({ limit: 100, activeOnly: true });
        } catch(e) {
             console.error(e);
        }
    }
    
    // Plan Validation State
    let validBatchClassroomIds = $state<Set<string> | null>(null);
    let loadingBatchValidClassrooms = $state(false);

    $effect(() => {
        const currentSubject = batchSubjectId;
        // Only run validation if Modal is open to save resources
        if (showBatchModal && batchMode === 'COURSE' && currentSubject && selectedSemesterId) {
             loadingBatchValidClassrooms = true;
             validBatchClassroomIds = new Set(); 
             
             // Check which classrooms have this subject in their plan
             listClassroomCourses({ subjectId: currentSubject, semesterId: selectedSemesterId })
                .then(res => {
                    if (batchSubjectId !== currentSubject) return; // Stale check
                    const ids = new Set(res.data.map((c: any) => c.classroom_id));
                    validBatchClassroomIds = ids;
                })
                .catch(err => {
                    console.error(err);
                    toast.error('ตรวจสอบแผนการเรียนไม่สำเร็จ');
                })
                .finally(() => { 
                    if (batchSubjectId === currentSubject) loadingBatchValidClassrooms = false; 
                });
        } else if (batchMode !== 'COURSE' || !batchSubjectId) {
             validBatchClassroomIds = null;
        }
    });

    let filteredBatchClassroomsList = $derived.by(() => {
        let list = classrooms;

        // Priority: Course Plan Validation
        if (batchMode === 'COURSE' && batchSubjectId) {
             const validSet = validBatchClassroomIds;
             if (validSet) {
                 list = list.filter(c => validSet.has(c.id));
             } else {
                 // Pending or Failed -> Empty list
                 list = [];
             }
        }
        
        // Apply Manual Filter (Intersection with Plan)
        if (batchGradeFilterId !== 'all') {
             list = list.filter(c => c.grade_level_id === batchGradeFilterId);
        }
        
        return list;
    });

    function toggleBatchClassroom(id: string) {
        if (batchClassrooms.includes(id)) {
            batchClassrooms = batchClassrooms.filter(c => c !== id);
        } else {
            batchClassrooms = [...batchClassrooms, id];
        }
    }
    
    function selectAllBatchClassrooms() {
        // Only select currently filtered items
        const currentListIds = filteredBatchClassroomsList.map(c => c.id);
        
        // If all currently visible are selected -> Deselect All (visible)
        const allVisibleSelected = currentListIds.every(id => batchClassrooms.includes(id));
        
        if (allVisibleSelected) {
             batchClassrooms = batchClassrooms.filter(id => !currentListIds.includes(id));
        } else {
             // Add missing ones
             const newIds = currentListIds.filter(id => !batchClassrooms.includes(id));
             batchClassrooms = [...batchClassrooms, ...newIds];
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
        
        // Validate based on mode
        if (batchMode === 'TEXT' && !batchTitle) {
            toast.error('กรุณาระบุชื่อกิจกรรม');
            return;
        }
        if (batchMode === 'COURSE' && !batchSubjectId) {
            toast.error('กรุณาเลือกรายวิชา');
            return;
        }

        try {
            submitting = true;
            
            let titleToSend = batchTitle;
            let entryTypeToSend = batchType;
            let subjectIdToSend = undefined;

            if (batchMode === 'COURSE') {
                const subj = subjectOptions.find(s => s.id === batchSubjectId);
                titleToSend = subj?.name || ''; 
                // We send ACTIVITY first, backend might auto-convert to COURSE if it finds a mapping.
                // Or we can send ACTIVITY and let backend handle it as per our new logic.
                entryTypeToSend = 'ACTIVITY'; 
                subjectIdToSend = batchSubjectId;
            }

            await createBatchTimetableEntries({
                classroom_ids: batchClassrooms,
                day_of_week: batchDay,
                period_id: batchPeriodId,
                academic_semester_id: selectedSemesterId,
                entry_type: entryTypeToSend as any,
                title: titleToSend,
                room_id: batchRoomId === 'none' ? undefined : batchRoomId,
                subject_id: subjectIdToSend,
                force: batchForce
            });
            
            toast.success('บันทึกกิจกรรมเรียบร้อย');
            showBatchModal = false;
            
            // Reset fields
            batchTitle = '';
            batchSubjectId = '';
            
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

    // WebSocket Connection
    $effect(() => {
        if (selectedSemesterId && $authStore.user) {
             const user = $authStore.user;
             connectTimetableSocket({

                 semester_id: selectedSemesterId,
                 user_id: user.id,
                 name: `${user.firstName} ${user.lastName}`
             });
        }
    });

    onDestroy(() => {
        disconnectTimetableSocket();
    });

    let lastCursorSend = 0;
    function handleMouseMove(e: MouseEvent) {
        const now = Date.now();
        if (now - lastCursorSend > 50 && $authStore.user) { // 20fps cap
             lastCursorSend = now;
             
             // Dynamic Context
             const currentViewId = viewMode === 'CLASSROOM' ? selectedClassroomId : selectedInstructorId;
             
             sendTimetableEvent({
                 type: 'CursorMove',
                 payload: {
                     user_id: $authStore.user.id,
                     x: e.clientX,
                     y: e.clientY,
                     context: {
                         view_mode: viewMode,
                         view_id: currentViewId
                     }
                 }
             });
        }
    }

    // Auto Refresh Listener
    $effect(() => {
        if ($refreshTrigger > 0) {
            console.log('Auto-refreshing timetable...');
            loadTimetable();
            loadCourses(); 
        }
    });

    function getDragOwner(entryId?: string, courseId?: string) {
        if (!entryId && !courseId) return null;
        for (const [userId, drag] of Object.entries($userDrags)) {
             // Strict check: Only lock if entry_id matches (Move)
             // or if dragging NEW course (courseId matches, but only if we want to lock new drags?)
             
             // Request: Don't lock existing entries if dragging NEW course.
             if (entryId) {
                 // Only lock if someone is dragging THIS SPECIFIC entry (Move)
                 if (drag.entry_id === entryId) return $activeUsers.find(u => u.user_id === userId);
             } 
             // If this is a course list item
             else if (courseId) {
                 // Lock list item if someone is dragging this course
                 if (drag.course_id === courseId && !drag.entry_id) return $activeUsers.find(u => u.user_id === userId);
             }
        }
        return null;
    }

	onMount(loadInitialData);
</script>

<div
	class="h-full flex flex-col space-y-4 relative"
	role="application"
	onmousemove={handleMouseMove}
>
	<div class="flex items-start justify-between gap-4">
		<div class="flex flex-col gap-2">
			<h2 class="text-3xl font-bold flex items-center gap-2">
				<CalendarDays class="w-8 h-8" />
				จัดตารางสอน
			</h2>
			<p class="text-muted-foreground">
				ลากวิชาจากด้านซ้าย มาวางในช่องตารางด้านขวา (ระบบจะตรวจสอบการชนอัตโนมัติ)
			</p>
		</div>

		<!-- Status Indicator -->
		<div
			class="flex items-center gap-3 bg-white/50 backdrop-blur px-3 py-1.5 rounded-full border shadow-sm"
		>
			<div class="flex items-center gap-2">
				<div
					class="w-2.5 h-2.5 rounded-full {$isConnected
						? 'bg-green-500 shadow-[0_0_8px_rgba(34,197,94,0.6)]'
						: 'bg-red-500'}"
				></div>
				<span class="text-xs font-semibold text-muted-foreground">
					{$isConnected ? `Online (${$activeUsers.length})` : 'Offline'}
				</span>
			</div>

			{#if $isConnected && $activeUsers.length > 0}
				<div class="w-px h-4 bg-border mx-1"></div>
				<div class="flex -space-x-1.5">
					{#each $activeUsers.slice(0, 4) as user (user.user_id)}
						<!-- Interactive Avatar -->
						<button
							class="w-6 h-6 rounded-full border-2 border-white flex items-center justify-center text-[9px] text-white font-bold ring-1 ring-border/10 shadow-sm transition-transform hover:scale-110 hover:z-10 cursor-pointer"
							style="background-color: {user.color}"
							title="{user.name} {user.context?.view_id
								? `(อยู่ที่ ${user.context.view_mode === 'CLASSROOM' ? 'ม.' : 'อ.'} ${user.context.view_id})`
								: ''}"
							onclick={() => {
								if (
									user.context?.view_mode &&
									(user.context.view_mode === 'CLASSROOM' ||
										user.context.view_mode === 'INSTRUCTOR')
								) {
									viewMode = user.context.view_mode;
									if (user.context.view_id) {
										if (viewMode === 'CLASSROOM') selectedClassroomId = user.context.view_id;
										else selectedInstructorId = user.context.view_id;
										toast.info(`ย้ายไปดูหน้าจอของ ${user.name}`);
									}
								}
							}}
						>
							{user.name.charAt(0).toUpperCase()}
						</button>
					{/each}
					{#if $activeUsers.length > 4}
						<div
							class="w-6 h-6 rounded-full bg-muted border-2 border-white flex items-center justify-center text-[8px] font-bold shadow-sm"
						>
							+{$activeUsers.length - 4}
						</div>
					{/if}
				</div>
			{/if}
		</div>
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
				<Select.Root type="single" bind:value={selectedSemesterId}>
					<Select.Trigger class="w-full">
						{semesters.find((s) => s.id === selectedSemesterId)?.term || 'เลือกเทอม'}
					</Select.Trigger>
					<Select.Content>
						{#each semesters as term}
							<Select.Item value={term.id}>ภาคเรียนที่ {term.term}</Select.Item>
						{/each}
					</Select.Content>
				</Select.Root>
			</div>

			{#if viewMode === 'CLASSROOM'}
				<div class="w-[250px]">
					<Select.Root type="single" bind:value={selectedClassroomId}>
						<Select.Trigger class="w-full">
							{classrooms.find((c) => c.id === selectedClassroomId)?.name || 'เลือกห้องเรียน'}
						</Select.Trigger>
						<Select.Content class="max-h-[300px] overflow-y-auto">
							{#each classrooms as classroom}
								<Select.Item value={classroom.id}>{classroom.name}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
			{:else}
				<div class="w-[250px]">
					<Select.Root type="single" bind:value={selectedInstructorId}>
						<Select.Trigger class="w-full">
							{instructors.find((i) => i.id === selectedInstructorId)?.name || 'เลือกครูผู้สอน'}
						</Select.Trigger>
						<Select.Content class="max-h-[300px] overflow-y-auto">
							{#each instructors as instructor}
								<Select.Item value={instructor.id}>{instructor.name}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
			{/if}
		</div>
	</div>

	<!-- Main Content Grid -->
	<div class="grid grid-cols-12 gap-6 h-[calc(100vh-250px)] min-h-[600px]">
		<!-- Left Sidebar: Courses -->
		<Card.Root class="col-span-2 flex flex-col h-full overflow-hidden">
			<Card.Header class="py-3 px-4 border-b">
				<Card.Title class="text-base flex items-center gap-2">
					<BookOpen class="w-4 h-4" /> รายวิชา
					<span class="text-xs font-normal text-muted-foreground ml-auto">
						ลากวิชาไปวางในตาราง
					</span>
				</Card.Title>
			</Card.Header>
			<div class="flex-1 overflow-y-auto p-4 space-y-3 bg-muted/20">
				{#each unscheduledCourses as course}
					{@const lockedBy = getDragOwner(undefined, course.id)}
					<div
						class="bg-background border rounded-lg p-3 shadow-sm cursor-grab active:cursor-grabbing hover:shadow-md transition-all group relative {lockedBy
							? 'opacity-50 pointer-events-none'
							: ''}"
						draggable={!lockedBy}
						ondragstart={(e) => handleDragStart(e, course, 'NEW')}
						ondragend={handleDragEnd}
						role="button"
						tabindex="0"
					>
						{#if lockedBy}
							<div
								class="absolute inset-0 flex items-center justify-center z-10 bg-white/50 backdrop-blur-[1px] rounded-lg"
							>
								<span
									class="text-[10px] font-bold px-2 py-1 rounded text-white shadow-sm"
									style="background-color: {lockedBy.color};"
								>
									{lockedBy.name} กำลังใช้
								</span>
							</div>
						{/if}

						<div class="flex justify-between items-start mb-1">
							<Badge variant="outline" class="text-xs">{course.subject_code}</Badge>
							<Badge variant={course.is_completed ? 'secondary' : 'default'} class="text-[10px]">
								{course.scheduled_count}/{course.max_periods} คาบ
							</Badge>
						</div>
						<h4 class="font-medium text-sm line-clamp-2 leading-tight mb-1">
							{course.subject_name_th || course.title_th || course.title || 'ไม่มีชื่อวิชา'}
						</h4>
						<div class="flex flex-col gap-0.5 text-[10px] text-muted-foreground mt-2">
							{#if viewMode === 'CLASSROOM'}
								<div class="flex items-center gap-1">
									<Users class="w-3 h-3" />
									{course.instructor_name || 'ไม่ระบุครู'}
								</div>
							{:else}
								<div class="flex items-center gap-1">
									<School class="w-3 h-3" />
									{course.classroom_name || 'ไม่ระบุห้อง'}
								</div>
							{/if}
							<div>{course.subject_credit} นก.</div>
						</div>

						<!-- Progress Bar -->
						<div class="mt-2 h-1 w-full bg-secondary rounded-full overflow-hidden">
							<div
								class="h-full bg-primary transition-all"
								style="width: {(course.scheduled_count / course.max_periods) * 100}%"
							></div>
						</div>
					</div>
				{:else}
					<div class="text-center text-muted-foreground py-8 text-sm">
						{#if !selectedClassroomId && !selectedInstructorId}
							กรุณาเลือก{viewMode === 'CLASSROOM' ? 'ห้องเรียน' : 'ครูผู้สอน'}
						{:else if courses.length === 0}
							ไม่พบรายวิชา
						{:else}
							จัดตารางครบแล้ว
						{/if}
					</div>
				{/each}
			</div>
		</Card.Root>

		<!-- Right Content: Timetable Grid -->
		<Card.Root class="col-span-10 flex flex-col h-full overflow-hidden border-2 shadow-none">
			<div class="overflow-auto flex-1">
				<div class="min-w-[800px] h-full flex flex-col">
					<!-- Header Row (Periods) -->
					<div class="flex sticky top-0 bg-background z-20 shadow-sm">
						<div
							class="w-24 shrink-0 p-3 border-r border-b font-medium text-sm text-muted-foreground flex items-center justify-center bg-background sticky left-0 z-30"
						>
							วัน/คาบ
						</div>
						{#each periods as period}
							<div class="flex-1 min-w-[100px] p-2 border-r text-center relative group">
								<div class="text-sm font-bold">คาบที่ {period.order_index}</div>
								<div class="text-xs text-muted-foreground">
									{formatTime(period.start_time)}-{formatTime(period.end_time)}
								</div>
							</div>
						{/each}
					</div>

					<!-- Days Rows -->
					{#each DAYS as day}
						<div class="flex flex-1 min-h-[100px]">
							<!-- Day Header -->
							<div
								class="w-24 shrink-0 border-r border-b bg-background font-medium flex items-center justify-center relative sticky left-0 z-10"
							>
								<!-- Day Indicator Line -->
								{#if day.value === 'MON'}<div
										class="absolute left-0 inset-y-0 w-1 bg-yellow-400"
									></div>{/if}
								{#if day.value === 'TUE'}<div
										class="absolute left-0 inset-y-0 w-1 bg-pink-400"
									></div>{/if}
								{#if day.value === 'WED'}<div
										class="absolute left-0 inset-y-0 w-1 bg-green-400"
									></div>{/if}
								{#if day.value === 'THU'}<div
										class="absolute left-0 inset-y-0 w-1 bg-orange-400"
									></div>{/if}
								{#if day.value === 'FRI'}<div
										class="absolute left-0 inset-y-0 w-1 bg-blue-400"
									></div>{/if}

								<div class="text-center">
									<div class="text-base font-bold">{day.label}</div>
								</div>
							</div>

							<!-- Slots -->
							{#each periods as period}
								{@const entry = getEntryForSlot(day.value, period.id)}
								{@const isOccupied = isSlotOccupiedByInstructor(day.value, period.id)}
								{@const isUnavailableRoom = unavailableRooms.has(period.id)}
								{@const lockedBy = entry ? getDragOwner(entry.id) : null}

								<!-- Drop Zone -->
								<div
									class="flex-1 border-r border-b min-w-[100px] relative transition-colors {isOccupied
										? 'bg-red-50/50 from-red-100/20 bg-gradient-to-br'
										: 'hover:bg-accent/50'} {draggedCourse && !entry && !isOccupied
										? 'bg-blue-50/30'
										: ''}"
									ondragover={handleDragOver}
									ondrop={(e) => handleDrop(e, day.value, period.id)}
									role="application"
								>
									{#if entry}
										<!-- Timetable Entry Card -->
										<div
											class="absolute inset-1 bg-blue-50/80 border border-blue-200 rounded p-2 text-xs flex flex-col justify-between shadow-sm hover:shadow-md transition-all group cursor-grab active:cursor-grabbing hover:border-blue-300 hover:bg-blue-100/50 {lockedBy
												? 'opacity-50 pointer-events-none ring-2 ring-offset-1 ring-' +
													lockedBy.color
												: ''}"
											draggable={!lockedBy}
											ondragstart={(e) => handleDragStart(e, entry, 'MOVE')}
											ondragend={handleDragEnd}
											role="button"
											tabindex="0"
										>
											{#if lockedBy}
												<div class="absolute -top-2 -right-2 z-20">
													<span
														class="text-[9px] font-bold px-1.5 py-0.5 rounded text-white shadow-sm"
														style="background-color: {lockedBy.color};"
													>
														{lockedBy.name}
													</span>
												</div>
											{/if}

											<div class="font-bold text-blue-900 truncate mb-0.5">
												{entry.subject_code || (entry.entry_type === 'ACTIVITY' ? 'กิจกรรม' : '')}
											</div>
											<div
												class="line-clamp-1 text-blue-800 text-[10px] mb-auto"
												title={entry.subject_name_th || entry.title}
											>
												{entry.subject_name_th || entry.title || 'ไม่มีชื่อ'}
											</div>
											<div
												class="mt-1 pt-1 border-t border-blue-200/50 gap-0.5 flex flex-col text-[9px] text-blue-700"
											>
												{#if viewMode === 'CLASSROOM'}
													<div class="flex items-center gap-1 truncate">
														<Users class="w-3 h-3 shrink-0" />
														{entry.instructor_name || '-'}
													</div>
												{:else}
													<div class="flex items-center gap-1 truncate">
														<School class="w-3 h-3 shrink-0" />
														{entry.classroom_name || '-'}
													</div>
												{/if}

												{#if entry.room_id}
													<div
														class="flex items-center gap-1 truncate text-blue-600"
														title={rooms.find((r) => r.id === entry.room_id)?.name_th}
													>
														<MapPin class="w-3 h-3 shrink-0" />
														{rooms.find((r) => r.id === entry.room_id)?.name_th || '?'}
													</div>
												{/if}
											</div>

											<!-- Delete Button -->
											<button
												class="absolute top-0.5 right-0.5 opacity-0 group-hover:opacity-100 p-0.5 hover:bg-red-100 hover:text-red-500 rounded transition-all z-30"
												onclick={(e) => {
													e.stopPropagation();
													handleDeleteEntry(entry.id);
												}}
											>
												<Trash2 class="w-3 h-3" />
											</button>
										</div>
									{:else if isOccupied}
										<div
											class="absolute inset-0 flex items-center justify-center p-2 text-center opacity-40 select-none"
										>
											<div class="text-xs text-red-500 font-medium">ครูติดสอน</div>
										</div>
									{:else if draggedCourse}
										<div
											class="absolute inset-0 flex items-center justify-center opacity-0 hover:opacity-100 pointer-events-none"
										>
											<div class="text-xs text-blue-500 font-medium">+ วางที่นี่</div>
										</div>
									{/if}
								</div>
							{/each}
						</div>
					{/each}
				</div>
			</div>
		</Card.Root>
	</div>
</div>

<Dialog.Root bind:open={showRoomModal}>
	<Dialog.Content class="sm:max-w-[425px]">
		<Dialog.Header>
			<Dialog.Title>เลือกห้องเรียน ({draggedCourse?.subject_code})</Dialog.Title>
			<Dialog.Description>
				<div class="flex flex-col gap-1 mt-1 text-foreground text-left">
					<span class="font-medium text-sm text-primary"
						>{draggedCourse?.subject_name_th ||
							draggedCourse?.title_th ||
							draggedCourse?.title}</span
					>
					<span class="text-xs text-muted-foreground flex items-center gap-2">
						{#if viewMode === 'CLASSROOM'}
							<span class="flex items-center gap-1"
								><Users class="w-3 h-3" /> {draggedCourse?.instructor_name || '-'}</span
							>
							<span class="flex items-center gap-1"
								><School class="w-3 h-3" />
								{classrooms.find((c) => c.id === selectedClassroomId)?.name || ''}</span
							>
						{:else}
							<span class="flex items-center gap-1"
								><School class="w-3 h-3" /> {draggedCourse?.classroom_name || '-'}</span
							>
						{/if}
					</span>
				</div>
			</Dialog.Description>
		</Dialog.Header>

		<div class="py-4 space-y-4">
			<div class="space-y-2">
				<Label.Root>ห้องเรียน</Label.Root>
				<Select.Root type="single" bind:value={selectedRoomId}>
					<Select.Trigger class="w-full">
						{rooms.find((r) => r.id === selectedRoomId)?.name_th ||
							(selectedRoomId === 'none' ? 'ไม่ระบุห้อง' : 'เลือกห้อง')}
					</Select.Trigger>
					<Select.Content class="max-h-[300px] overflow-y-auto">
						<Select.Item value="none" class="text-muted-foreground">ไม่ระบุห้อง</Select.Item>
						{#each rooms as room}
							{@const isBusy = unavailableRooms.has(room.id)}
							{@const displaySelected = selectedRoomId === room.id}

							{#if !isBusy || displaySelected}
								<Select.Item value={room.id} class="flex justify-between">
									<span>{room.name_th} ({room.building_name})</span>
								</Select.Item>
							{/if}
						{/each}
					</Select.Content>
				</Select.Root>
			</div>
		</div>

		<Dialog.Footer>
			<Button
				variant="outline"
				onclick={() => {
					showRoomModal = false;
					isDropPending = false;
					handleDragEnd(); // Cancel drag
				}}>ยกเลิก</Button
			>
			<Button onclick={confirmDropWithRoom} disabled={submitting}>
				{#if submitting}
					<Loader2 class="mr-2 h-4 w-4 animate-spin" />
				{/if}
				ยืนยัน
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<!-- Batch Assign Modal -->
<Dialog.Root bind:open={showBatchModal}>
	<Dialog.Content class="sm:max-w-[600px]">
		<Dialog.Header>
			<Dialog.Title>เพิ่มกิจกรรมพิเศษ (Batch)</Dialog.Title>
			<Dialog.Description>
				เพิ่มกิจกรรมให้หลายห้องเรียนพร้อมกัน (เช่น กิจกรรมหน้าเสาธง, ประชุมระดับ)
			</Dialog.Description>
		</Dialog.Header>

		<div class="grid gap-4 py-4">
			<!-- Mode Selection -->
			<div class="grid grid-cols-4 items-center gap-4">
				<Label.Root class="text-right">รูปแบบ</Label.Root>
				<div class="col-span-3 flex gap-2">
					<Button
						variant={batchMode === 'TEXT' ? 'default' : 'outline'}
						size="sm"
						onclick={() => (batchMode = 'TEXT')}
					>
						ระบุชื่อเอง
					</Button>
					<Button
						variant={batchMode === 'COURSE' ? 'default' : 'outline'}
						size="sm"
						onclick={() => {
							batchMode = 'COURSE';
							ensureSubjectsLoaded();
						}}
					>
						เลือกจากรายวิชา
					</Button>
				</div>
			</div>

			{#if batchMode === 'TEXT'}
				<div class="grid grid-cols-4 items-center gap-4">
					<Label.Root class="text-right">ชื่อกิจกรรม</Label.Root>
					<div class="col-span-3">
						<input
							type="text"
							class="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background file:border-0 file:bg-transparent file:text-sm file:font-medium placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
							bind:value={batchTitle}
							placeholder="เช่น ประชุมระดับ, กิจกรรมพัฒนาผู้เรียน"
						/>
					</div>
				</div>

				<div class="grid grid-cols-4 items-center gap-4">
					<Label.Root class="text-right">ประเภท</Label.Root>
					<div class="col-span-3">
						<Select.Root type="single" bind:value={batchType}>
							<Select.Trigger class="w-full">
								{batchType === 'ACTIVITY'
									? 'กิจกรรม'
									: batchType === 'ACADEMIC'
										? 'วิชาการ'
										: 'อื่นๆ'}
							</Select.Trigger>
							<Select.Content>
								<Select.Item value="ACTIVITY">กิจกรรม</Select.Item>
								<Select.Item value="ACADEMIC">วิชาการ</Select.Item>
							</Select.Content>
						</Select.Root>
					</div>
				</div>
			{:else}
				<div class="grid grid-cols-4 items-center gap-4">
					<Label.Root class="text-right">รายวิชา</Label.Root>
					<div class="col-span-3">
						{#if loadingSubjects}
							<div class="text-sm text-muted-foreground flex items-center gap-2">
								<Loader2 class="w-3 h-3 animate-spin" /> กำลังโหลด...
							</div>
						{:else}
							<Select.Root type="single" bind:value={batchSubjectId}>
								<Select.Trigger class="w-full h-auto py-2">
									<div class="flex flex-col items-start gap-0.5 text-left overflow-hidden">
										<span class="truncate block w-full"
											>{subjectOptions.find((s) => s.id === batchSubjectId)?.name ||
												'เลือกรายวิชา'}</span
										>
										{#if batchSubjectId && subjectOptions.find((s) => s.id === batchSubjectId)?.code}
											<span class="text-xs text-muted-foreground">
												{subjectOptions.find((s) => s.id === batchSubjectId)?.code}
											</span>
										{/if}
									</div>
								</Select.Trigger>
								<Select.Content class="max-h-[300px] w-[350px] overflow-y-auto">
									{#each subjectOptions as subj}
										<Select.Item
											value={subj.id}
											label={subj.name}
											class="flex flex-col items-start py-2 border-b last:border-0"
										>
											<span class="font-medium text-sm">{subj.name}</span>
											{#if subj.code}
												<span class="text-xs text-muted-foreground">{subj.code}</span>
											{/if}
										</Select.Item>
									{/each}
								</Select.Content>
							</Select.Root>
						{/if}
						<p class="text-[10px] text-muted-foreground mt-1.5 leading-relaxed">
							*ระบบจะค้นหา <b>"โครงสร้างรายวิชา"</b> ของแต่ละห้องเรียนให้โดยอัตโนมัติ <br />
							(หากห้องใดไม่ได้ลงทะเบียนวิชานี้ในโครงสร้าง ระบบจะข้ามไปหรือสร้างเป็นกิจกรรมแทน)
						</p>
					</div>
				</div>
			{/if}

			<div class="grid grid-cols-4 items-center gap-4">
				<Label.Root class="text-right">วัน/เวลา</Label.Root>
				<div class="col-span-3 flex gap-2">
					<Select.Root type="single" bind:value={batchDay}>
						<Select.Trigger class="w-[120px]">
							{DAYS.find((d) => d.value === batchDay)?.label}
						</Select.Trigger>
						<Select.Content>
							{#each DAYS as day}
								<Select.Item value={day.value}>{day.label}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>

					<Select.Root type="single" bind:value={batchPeriodId}>
						<Select.Trigger class="flex-1">
							{periods.find((p) => p.id === batchPeriodId)
								? `คาบ ${periods.find((p) => p.id === batchPeriodId)?.order_index} (${formatTime(periods.find((p) => p.id === batchPeriodId)?.start_time)})`
								: 'เลือกคาบ'}
						</Select.Trigger>
						<Select.Content class="max-h-[200px] overflow-y-auto">
							{#each periods as period}
								<Select.Item value={period.id}>
									คาบ {period.order_index} ({formatTime(period.start_time)}-{formatTime(
										period.end_time
									)})
								</Select.Item>
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
				{#if !(batchMode === 'COURSE' && batchSubjectId)}
					<div class="flex items-center gap-2 mb-3">
						<Label.Root>กรองระดับชั้น:</Label.Root>
						<select
							class="flex h-8 w-full items-center justify-between rounded-md border border-input bg-background px-3 py-1 text-sm ring-offset-background placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
							value={batchGradeFilterId}
							onchange={(e) => (batchGradeFilterId = e.currentTarget.value)}
							onmouseenter={loadBatchGradeLevels}
						>
							<option value="all">ทุกระดับชั้น ({classrooms.length} ห้อง)</option>
							{#each batchGradeLevels as gl}
								<option value={gl.id}>{gl.name}</option>
							{/each}
						</select>
					</div>
				{:else}
					<div
						class="flex items-center gap-2 mb-3 px-3 py-2 bg-blue-50/50 rounded border border-blue-100 text-xs text-blue-700"
					>
						<span class="font-bold">Info:</span> ระบบแสดงเฉพาะห้องเรียนที่มีวิชานี้ในแผนการเรียน
					</div>
				{/if}

				<div class="flex justify-between items-center mb-2">
					<Label.Root>เลือกห้องที่ต้องการ ({batchClassrooms.length})</Label.Root>
					<Button variant="ghost" size="sm" class="h-6 text-xs" onclick={selectAllBatchClassrooms}>
						เลือกทั้งหมด
					</Button>
				</div>
				<div
					class="border rounded-md max-h-[200px] min-h-[100px] overflow-y-auto p-2 bg-muted/20 grid grid-cols-2 gap-2"
				>
					{#if loadingBatchValidClassrooms}
						<div
							class="col-span-2 flex items-center justify-center py-8 text-muted-foreground text-sm"
						>
							<Loader2 class="w-4 h-4 mr-2 animate-spin" /> กำลังตรวจสอบแผนการเรียน...
						</div>
					{:else}
						{#each filteredBatchClassroomsList as classroom}
							<div class="flex items-center space-x-2 bg-background p-1.5 rounded border shadow-sm">
								<Checkbox
									id="batch-class-{classroom.id}"
									checked={batchClassrooms.includes(classroom.id)}
									onCheckedChange={() => toggleBatchClassroom(classroom.id)}
								/>
								<label
									for="batch-class-{classroom.id}"
									class="text-sm font-medium leading-none peer-disabled:cursor-not-allowed peer-disabled:opacity-70 cursor-pointer flex-1"
								>
									{classroom.name}
								</label>
							</div>
						{:else}
							<div
								class="col-span-2 flex flex-col items-center justify-center text-muted-foreground py-4 opacity-70"
							>
								<School class="w-8 h-8 mb-2 opacity-20" />
								<span class="text-xs">ไม่พบห้องเรียนที่มีวิชานี้ในแผนการเรียน</span>
							</div>
						{/each}
					{/if}
				</div>

				<!-- Override Option -->
				<div
					class="flex items-start space-x-2 mt-4 p-3 rounded-md border border-red-200 bg-red-50/50"
				>
					<Checkbox
						id="batch-force"
						bind:checked={batchForce}
						class="mt-0.5 data-[state=checked]:bg-red-600 data-[state=checked]:border-red-600"
					/>
					<div class="grid gap-1.5 leading-none">
						<label
							for="batch-force"
							class="text-sm font-medium leading-none text-red-600 cursor-pointer"
						>
							บังคับลงตาราง (Override)
						</label>
						<p class="text-xs text-muted-foreground">
							หากเลือก: ระบบจะลบรายการเดิมที่เวลาชนกันออกโดยอัตโนมัติ (ทั้งตารางนักเรียน, ครู
							และห้องเรียน) เพื่อให้กิจกรรมนี้ลงได้
						</p>
					</div>
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

<!-- GHOST UI OVERLAY -->
<div class="pointer-events-none fixed inset-0 z-[9999] overflow-hidden">
	{#each $activeUsers as user (user.user_id)}
		{@const cursor = $remoteCursors[user.user_id]}

		{#if cursor && user.user_id !== $authStore.user?.id}
			<!-- Context Check: Only show if in same view -->
			{#if cursor.context?.view_mode === viewMode && cursor.context?.view_id === (viewMode === 'CLASSROOM' ? selectedClassroomId : selectedInstructorId)}
				<div
					class="absolute transition-transform duration-100 ease-linear flex flex-col items-start gap-1"
					style="transform: translate({cursor.x}px, {cursor.y}px);"
				>
					<!-- Cursor Icon -->
					<svg
						xmlns="http://www.w3.org/2000/svg"
						class="h-5 w-5 drop-shadow-md"
						fill={user.color}
						viewBox="0 0 24 24"
						stroke="white"
						stroke-width="2"
					>
						<path d="M3 3l7.07 16.97 2.51-7.39 7.39-2.51L3 3z" />
					</svg>

					<!-- Name Tag -->
					<div
						class="px-2 py-0.5 rounded text-[10px] text-white font-bold whitespace-nowrap shadow-sm"
						style="background-color: {user.color}"
					>
						{user.name}
					</div>

					<!-- GHOST DRAG ITEM -->
					{#if $userDrags[user.user_id]}
						{@const drag = $userDrags[user.user_id]}
						<div
							class="bg-background border rounded shadow-lg p-2.5 flex items-center gap-3 mt-2 opacity-95 scale-90 origin-top-left animate-in fade-in zoom-in duration-200 min-w-[150px]"
						>
							<div class="p-1.5 rounded bg-muted">
								<BookOpen class="w-4 h-4 text-primary" />
							</div>
							<div class="flex flex-col">
								<span class="text-xs font-bold leading-tight line-clamp-1"
									>{drag.info?.code || 'วิชา'}</span
								>
								<span class="text-[10px] text-muted-foreground line-clamp-1"
									>{drag.info?.title || 'กำลังลาก...'}</span
								>
							</div>
						</div>
					{/if}
				</div>
			{/if}
		{/if}
	{/each}
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
