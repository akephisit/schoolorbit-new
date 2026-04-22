<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { goto } from '$app/navigation';
	import { toast } from 'svelte-sonner';
	import {
		type TimetableEntry,
		type AcademicPeriod,
		listTimetableEntries,
		createTimetableEntry,
		updateTimetableEntry,
		deleteTimetableEntry,
		listPeriods,
		createBatchTimetableEntries,
		deleteBatchTimetableEntries,
		removeEntryInstructor,
		addEntryInstructor,
		restoreInstructorToSlot,
		hideInstructorFromSlot,
		swapTimetableEntries,
		validateTimetableMoves
	} from '$lib/api/timetable';
	import {
		lookupAcademicYears,
		listClassrooms,
		listClassroomCourses,
		listCourseInstructors,
		type CourseInstructor,
		getSchoolDays,
		type Classroom,
		getAcademicStructure,
		type AcademicYear,
		type Semester,
		listActivitySlots,
		type ActivitySlot,
		ACTIVITY_TYPE_LABELS,
		listActivityGroups,
		listSlotClassroomAssignments,
		listSlotInstructors
	} from '$lib/api/academic';
	import {
		lookupRooms,
		type RoomLookupItem,
		lookupStaff,
		type StaffLookupItem,
		lookupSubjects,
		type LookupItem,
		lookupGradeLevels,
		type GradeLevelLookupItem
	} from '$lib/api/lookup';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Label from '$lib/components/ui/label';

	import * as Card from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import * as Select from '$lib/components/ui/select';
	import { Badge } from '$lib/components/ui/badge';
	import * as Tooltip from '$lib/components/ui/tooltip';

	import {
		CalendarDays,
		Trash2,
		Loader2,
		School,
		BookOpen,
		Users,
		PlusCircle,
		MapPin,
		Download,
		Zap,
		Lock
	} from 'lucide-svelte';
	import { tick } from 'svelte';
	import { generateTimetablePDF } from '$lib/utils/pdf';

	import { Checkbox } from '$lib/components/ui/checkbox';

	import { authStore } from '$lib/stores/auth';
	import {
		connectTimetableSocket,
		disconnectTimetableSocket,
		sendTimetableEvent,
		activeUsers,
		remoteCursors,
		dragPositions,
		userDrags,
		refreshTrigger,
		isConnected,
		lastPatch,
		setInitialSeq,
		type TimetablePatch
	} from '$lib/stores/timetable-socket';

	let { data } = $props();

	// Helper: Generate consistent pastel color from string
	function getSubjectColor(code: string, type?: string): string {
		if (type === 'BREAK') return '#fef3c7'; // amber-100
		if (type === 'ACTIVITY' || type === 'HOMEROOM') return '#d1fae5'; // emerald-100

		if (!code) return '#eff6ff'; // default blue-50
		let hash = 0;
		for (let i = 0; i < code.length; i++) {
			hash = code.charCodeAt(i) + ((hash << 5) - hash);
		}
		const h = Math.abs(hash) % 360;
		return `hsl(${h}, 85%, 94%)`; // Light pastel
	}

	function getSubjectBorderColor(code: string, type?: string): string {
		if (type === 'BREAK') return '#fcd34d'; // amber-300
		if (type === 'ACTIVITY' || type === 'HOMEROOM') return '#6ee7b7'; // emerald-300

		if (!code) return '#bfdbfe'; // default blue-200
		let hash = 0;
		for (let i = 0; i < code.length; i++) {
			hash = code.charCodeAt(i) + ((hash << 5) - hash);
		}
		const h = Math.abs(hash) % 360;
		return `hsl(${h}, 60%, 80%)`; // Slightly darker for border
	}

	// Export State
	let showExportModal = $state(false);
	let exportType = $state<'CLASSROOM' | 'INSTRUCTOR'>('CLASSROOM');
	let exportTargetIds = $state<string[]>([]);
	let isExporting = $state(false);

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

	// Activity slots for sidebar
	let sidebarActivitySlots = $state<ActivitySlot[]>([]);

	// Instructor's activity groups map: slot_id → group name (for INSTRUCTOR view)
	let instructorGroupsMap = $state<Record<string, string>>({});

	// Delete activity dialog (synchronized: ask single vs batch)
	let showDeleteActivityDialog = $state(false);
	let deleteActivityTarget = $state<TimetableEntry | null>(null);

	// View Mode: 'CLASSROOM' or 'INSTRUCTOR'
	let viewMode = $state<'CLASSROOM' | 'INSTRUCTOR'>('CLASSROOM');

	// Per-cell instructor editor (Popover dialog)
	let entryPopoverOpen = $state(false);
	let entryPopoverTarget = $state<TimetableEntry | null>(null);
	let entryPopoverTeam = $state<CourseInstructor[]>([]);
	let entryPopoverLoading = $state(false);
	let entryPopoverSaving = $state('');

	// INSTRUCTOR view: toggle "แสดงคาบในทีม" (ghost cells)
	let showTeamGhosts = $state(false);
	// Raw entries (รวม ghost เสมอ) ใช้คำนวณ unscheduled ให้เสถียร — ไม่กระพริบเวลา toggle
	let rawTeamEntries = $state<TimetableEntry[]>([]);

	let popoverInCell = $derived(entryPopoverTarget?.instructor_ids ?? []);
	let popoverInCellNames = $derived(entryPopoverTarget?.instructor_names ?? []);
	let popoverNotInCell = $derived(
		entryPopoverTeam.filter(
			(t) => !(entryPopoverTarget?.instructor_ids ?? []).includes(t.instructor_id)
		)
	);

	let selectedYearId = $state('');
	let selectedSemesterId = $state('');
	let selectedClassroomId = $state('');
	let selectedInstructorId = $state('');

	// Derived
	let DAYS = $derived(
		getSchoolDays(academicYears.find((y) => y.id === selectedYearId)?.school_days)
	);
	let semesters = $derived(allSemesters.filter((s) => s.academic_year_id === selectedYearId));

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
				const yearSemesters = allSemesters.filter((s) => s.academic_year_id === activeYear.id);
				const activeSemester = yearSemesters.find((s) => s.is_active) || yearSemesters[0];
				if (activeSemester) selectedSemesterId = activeSemester.id;

				await Promise.all([loadClassrooms(), loadRooms(), loadInstructors()]);
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
		} catch (e) {
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
				res = await listClassroomCourses({
					classroomId: selectedClassroomId,
					semesterId: selectedSemesterId
				});
			} else {
				res = await listClassroomCourses({
					instructorId: selectedInstructorId,
					semesterId: selectedSemesterId
				});
			}
			courses = res.data;
		} catch (e) {
			console.error(e);
			toast.error('โหลดรายวิชาไม่สำเร็จ');
		}
	}

	async function checkClassroomHasInstructor(slotId: string, classroomIdOverride?: string): Promise<boolean> {
		try {
			const res = await listSlotClassroomAssignments(slotId);
			const clsId = classroomIdOverride || selectedClassroomId;
			return (res.data ?? []).some((a) => a.classroom_id === clsId);
		} catch { return false; }
	}

	// For INSTRUCTOR view: independent slots with per-classroom items
	let instructorActivityItems = $state<Array<{
		slot: ActivitySlot;
		classroom_id: string;
		classroom_name: string;
	}>>([]);

	async function loadSidebarActivitySlots() {
		if (!selectedSemesterId) {
			sidebarActivitySlots = [];
			instructorActivityItems = [];
			return;
		}

		if (viewMode === 'CLASSROOM' && selectedClassroomId) {
			try {
				const res = await listActivitySlots({ semester_id: selectedSemesterId });
				// ใช้ classroom_ids (actual participation) ไม่ใช่ catalog template
				sidebarActivitySlots = res.data.filter((slot) =>
					(slot.classroom_ids ?? []).includes(selectedClassroomId)
				);
			} catch (e) {
				console.error('Failed to load activity slots for sidebar', e);
			}
		} else if (viewMode === 'INSTRUCTOR' && selectedInstructorId) {
			try {
				const res = await listActivitySlots({ semester_id: selectedSemesterId });
				const allSlots = res.data;

				// Synchronized: slots where instructor is in slot_instructors
				const syncSlots = allSlots.filter((s) => s.scheduling_mode === 'synchronized');
				// Check which slots this instructor belongs to
				const relevantSyncSlots: ActivitySlot[] = [];
				for (const slot of syncSlots) {
					try {
						const instrRes = await listSlotInstructors(slot.id);
						if ((instrRes.data ?? []).some((i) => i.user_id === selectedInstructorId)) {
							relevantSyncSlots.push(slot);
						}
					} catch {}
				}

				// Independent: slots where instructor is assigned to classrooms
				const indepSlots = allSlots.filter((s) => s.scheduling_mode === 'independent');
				const items: typeof instructorActivityItems = [];
				for (const slot of indepSlots) {
					try {
						const assignRes = await listSlotClassroomAssignments(slot.id);
						for (const a of assignRes.data ?? []) {
							if (a.instructor_id === selectedInstructorId) {
								items.push({ slot, classroom_id: a.classroom_id, classroom_name: a.classroom_name ?? '' });
							}
						}
					} catch {}
				}

				sidebarActivitySlots = relevantSyncSlots;
				instructorActivityItems = items;
			} catch (e) {
				console.error('Failed to load activity slots for sidebar', e);
			}
		} else {
			sidebarActivitySlots = [];
			instructorActivityItems = [];
		}
	}

	async function loadInstructorGroups() {
		if (viewMode !== 'INSTRUCTOR' || !selectedInstructorId || !selectedSemesterId) {
			instructorGroupsMap = {};
			return;
		}
		try {
			const res = await listActivityGroups({ instructor_id: selectedInstructorId, semester_id: selectedSemesterId });
			const map: Record<string, string> = {};
			for (const g of res.data ?? []) {
				if (g.slot_id) map[g.slot_id] = g.name;
			}
			instructorGroupsMap = map;
		} catch (e) {
			console.error('Failed to load instructor groups', e);
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
				res = await listTimetableEntries({
					classroom_id: selectedClassroomId,
					academic_semester_id: selectedSemesterId
				});
			} else {
				// มุมมองครู: fetch ด้วย include_team_ghosts=true เสมอ (superset)
				// แล้ว filter สำหรับแสดง cell ตาม toggle
				res = await listTimetableEntries({
					instructor_id: selectedInstructorId,
					academic_semester_id: selectedSemesterId,
					include_team_ghosts: true
				});
			}
			if (viewMode === 'INSTRUCTOR') {
				rawTeamEntries = res.data;
				timetableEntries = showTeamGhosts
					? res.data
					: res.data.filter((e) => (e.instructor_ids ?? []).includes(selectedInstructorId));
			} else {
				rawTeamEntries = [];
				timetableEntries = res.data;
			}
			// Sync seq: ตั้งจุดเริ่ม tracking patch events จาก response
			if (typeof (res as any).current_seq === 'number') {
				setInitialSeq((res as any).current_seq);
			}
		} catch (e) {
			toast.error('โหลดตารางสอนไม่สำเร็จ');
		}
	}

	// ===== Per-cell instructor popover =====
	async function openEntryPopover(entry: TimetableEntry) {
		// Only support COURSE entries (activity entries have different instructor logic)
		if (entry.entry_type !== 'COURSE' || !entry.classroom_course_id) return;
		entryPopoverTarget = entry;
		entryPopoverTeam = [];
		entryPopoverOpen = true;
		entryPopoverLoading = true;
		try {
			const res = await listCourseInstructors(entry.classroom_course_id);
			entryPopoverTeam = res.data ?? [];
		} catch {
			toast.error('โหลดรายชื่อครูไม่สำเร็จ');
		} finally {
			entryPopoverLoading = false;
		}
	}

	async function handlePopoverRemoveInstructor(userId: string) {
		if (!entryPopoverTarget) return;
		const entry = entryPopoverTarget;
		if (viewMode === 'INSTRUCTOR' && userId === selectedInstructorId) {
			if (!confirm('ลบตัวเองจากคาบนี้? — คาบจะหายจากตารางของคุณ')) return;
		}
		entryPopoverSaving = userId;
		try {
			await removeEntryInstructor(entry.id, userId);
			// Update local state on the entry object (shared reference ระหว่าง rawTeamEntries + timetableEntries)
			const idx = entry.instructor_ids?.indexOf(userId) ?? -1;
			if (idx >= 0) {
				entry.instructor_ids = entry.instructor_ids!.filter((i) => i !== userId);
				entry.instructor_names = entry.instructor_names?.filter((_, i) => i !== idx);
			}
			// Trigger reactivity ทั้ง raw + filtered
			rawTeamEntries = [...rawTeamEntries];
			timetableEntries =
				viewMode === 'INSTRUCTOR' && !showTeamGhosts
					? rawTeamEntries.filter((e) => (e.instructor_ids ?? []).includes(selectedInstructorId))
					: [...timetableEntries];
			// ลบตัวเองในมุมมองครู + ghost ปิด → cell หายจาก grid → ปิด popover
			if (viewMode === 'INSTRUCTOR' && userId === selectedInstructorId && !showTeamGhosts) {
				entryPopoverOpen = false;
			}
			toast.success('ลบครูแล้ว');
		} catch {
			toast.error('ลบครูไม่สำเร็จ');
		} finally {
			entryPopoverSaving = '';
		}
	}

	async function handlePopoverAddInstructor(userId: string, role: 'primary' | 'secondary') {
		if (!entryPopoverTarget) return;
		const entry = entryPopoverTarget;
		entryPopoverSaving = userId;
		try {
			await addEntryInstructor(entry.id, userId, role);
			entry.instructor_ids = [...(entry.instructor_ids ?? []), userId];
			const member = entryPopoverTeam.find((t) => t.instructor_id === userId);
			if (member?.instructor_name) {
				entry.instructor_names = [...(entry.instructor_names ?? []), member.instructor_name];
			}
			rawTeamEntries = [...rawTeamEntries];
			// เพิ่มตัวเองในมุมมองครู + ghost ปิด → cell กลายเป็นของตัวเอง → ปรากฏใน grid
			timetableEntries =
				viewMode === 'INSTRUCTOR' && !showTeamGhosts
					? rawTeamEntries.filter((e) => (e.instructor_ids ?? []).includes(selectedInstructorId))
					: [...timetableEntries];
			toast.success('เพิ่มครูแล้ว');
		} catch {
			toast.error('เพิ่มครูไม่สำเร็จ');
		} finally {
			entryPopoverSaving = '';
		}
	}

	// Toggle ghost — filter frontend จาก rawTeamEntries (ไม่ re-fetch)
	$effect(() => {
		void showTeamGhosts;
		if (viewMode === 'INSTRUCTOR' && rawTeamEntries.length > 0) {
			timetableEntries = showTeamGhosts
				? rawTeamEntries
				: rawTeamEntries.filter((e) => (e.instructor_ids ?? []).includes(selectedInstructorId));
		}
	});

	async function handleDeleteEntry(entry: TimetableEntry) {
		if (viewMode === 'INSTRUCTOR') {
			if (entry.activity_slot_id) {
				const slot = sidebarActivitySlots.find((s) => s.id === entry.activity_slot_id)
					|| instructorActivityItems.find((i) => i.slot.id === entry.activity_slot_id)?.slot;
				if (slot?.scheduling_mode === 'synchronized') {
					if (!selectedInstructorId) return;
					try {
						await hideInstructorFromSlot(entry.activity_slot_id, selectedInstructorId);
						toast.success('ลบครูออกจากกิจกรรมนี้แล้ว (ทุกห้อง)');
					} catch (err: any) {
						toast.error(err.message || 'ลบไม่สำเร็จ');
						return;
					}
				} else {
					// Independent: one entry = one classroom; delete entry
					try {
						await deleteTimetableEntry(entry.id);
						toast.success('ลบกิจกรรมออกจากตารางสำเร็จ');
					} catch (e: any) {
						toast.error(e.message || 'ลบไม่สำเร็จ');
						return;
					}
				}
			} else {
				// Regular course: ลบ entry ทั้งอัน (กระทบครูทุกคน) — ถ้าจะลบแค่ตัวเอง ใช้ × ใน popover
				if (!confirm('ลบคาบนี้ทั้งอัน? — กระทบครูทุกคนในคาบ\n(ลบเฉพาะตัวเองใช้ × ใน popover)')) return;
				try {
					await deleteTimetableEntry(entry.id);
					toast.success('ลบคาบแล้ว');
				} catch (e: any) {
					toast.error(e.message || 'ลบไม่สำเร็จ');
					return;
				}
			}
			// Backend broadcasts patch event ให้แล้ว — ไม่ต้องส่ง TableRefresh ซ้ำ
			loadTimetable();
			loadSidebarActivitySlots();
			return;
		}

		if (entry.activity_slot_id) {
			const slot = sidebarActivitySlots.find((s) => s.id === entry.activity_slot_id)
				|| instructorActivityItems.find((i) => i.slot.id === entry.activity_slot_id)?.slot;
			if (slot?.scheduling_mode === 'independent') {
				// Independent: ลบ single entry ตรง ๆ
				await doDeleteEntry(entry.id, false);
			} else {
				// CLASSROOM view + synchronized: ถามก่อน
				deleteActivityTarget = entry;
				showDeleteActivityDialog = true;
			}
		} else {
			await doDeleteEntry(entry.id, false);
		}
	}

	async function doDeleteEntry(entryId: string, batch: boolean) {
		try {
			if (batch && deleteActivityTarget?.activity_slot_id) {
				const res = await deleteBatchTimetableEntries({
					activity_slot_id: deleteActivityTarget.activity_slot_id,
					day_of_week: deleteActivityTarget.day_of_week,
					academic_semester_id: deleteActivityTarget.academic_semester_id
				});
				toast.success(`ลบกิจกรรมทั้ง batch สำเร็จ (${res.deleted_count} รายการ)`);
			} else {
				await deleteTimetableEntry(entryId);
				toast.success('ลบออกจากตารางสำเร็จ');
			}

			// Backend broadcasts patch event ให้แล้ว — ไม่ต้องส่ง TableRefresh ซ้ำ

			showDeleteActivityDialog = false;
			deleteActivityTarget = null;
			loadTimetable();
		} catch (e: any) {
			toast.error(e.message || 'ลบไม่สำเร็จ');
		}
	}

	function getEntryForSlot(day: string, periodId: string): TimetableEntry | undefined {
		return displayEntries.find((e) => e.day_of_week === day && e.period_id === periodId);
	}

	// In INSTRUCTOR view, deduplicate ACTIVITY entries that share the same slot+day+period
	let displayEntries = $derived.by(() => {
		if (viewMode !== 'INSTRUCTOR') return timetableEntries;
		const seen = new Set<string>();
		return timetableEntries.filter((e) => {
			if (e.entry_type === 'ACTIVITY' && e.activity_slot_id) {
				const key = `${e.activity_slot_id}:${e.day_of_week}:${e.period_id}`;
				if (seen.has(key)) return false;
				seen.add(key);
			}
			return true;
		});
	});

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

	// Drag validity map: key "DAY|PERIODID" → cell state (from POST /timetable/validate-moves)
	// Populated on drag start for MOVE type; cleared on drag end.
	let moveValidityMap = $state<Map<string, import('$lib/api/timetable').MoveValidityCell>>(new Map());

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
				const res = await listTimetableEntries({
					classroom_id: classroomId,
					academic_semester_id: selectedSemesterId
				});
				res.data.forEach((entry) => {
					if (dragType === 'MOVE' && entry.id === draggedEntryId) return;

					// Conflict = entry ที่ viewer ไม่ได้อยู่ใน tei (ครูคนอื่น หรือ ghost ของเราเอง)
					// เช็คจาก instructor_ids ตรง ๆ ไม่ใช่ "isMyCourse" เพราะ ghost คือ course
					// ของฉันแต่ฉันไม่อยู่ใน cell นั้น — ไม่ควรวางทับ
					const iAmOnEntry = (entry.instructor_ids ?? []).includes(selectedInstructorId);
					if (iAmOnEntry) return;

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
			const res = await listTimetableEntries({
				instructor_id: instructorId,
				academic_semester_id: selectedSemesterId
			});
			res.data.forEach((entry) => {
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
		div.className =
			'fixed top-[-1000px] left-[-1000px] bg-white border border-primary/50 shadow-xl rounded-lg p-3 w-[180px] z-[9999] flex flex-col gap-1';
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

	function handleActivityDragStart(event: DragEvent, activity: typeof unscheduledActivities[number]) {
		dragType = 'NEW';
		// For INSTRUCTOR view independent: classroom comes from _classroom_id
		const classroomId = activity._classroom_id || selectedClassroomId;
		draggedCourse = {
			id: activity.id,
			_isActivity: true,
			activity_slot_id: activity.id,
			_classroom_id: classroomId,
			subject_code: ACTIVITY_TYPE_LABELS[activity.activity_type] ?? activity.activity_type,
			title_th: activity.name,
			title: activity.name
		};
		draggedEntryId = null;

		// Lookup instructor for this classroom from assignments → show conflict highlights
		if (activity.scheduling_mode === 'independent' && classroomId) {
			listSlotClassroomAssignments(activity.id).then((res) => {
				const assignment = (res.data ?? []).find((a) => a.classroom_id === classroomId);
				if (assignment) {
					fetchInstructorConflicts({ primary_instructor_id: assignment.instructor_id });
				}
			}).catch(() => {});
		}

		if (event.dataTransfer) {
			event.dataTransfer.effectAllowed = 'copy';
			event.dataTransfer.setData('text/plain', JSON.stringify({ type: 'NEW', id: activity.id }));
			const dragElement = createDragImage(
				ACTIVITY_TYPE_LABELS[activity.activity_type] ?? activity.activity_type,
				activity.name
			);
			event.dataTransfer.setDragImage(dragElement, 10, 10);
			setTimeout(() => document.body.removeChild(dragElement), 0);
		}

		// Notify others
		if ($authStore.user) {
			sendTimetableEvent({
				type: 'DragStart',
				payload: {
					user_id: $authStore.user.id,
					course_id: activity.id,
					info: {
						code: ACTIVITY_TYPE_LABELS[activity.activity_type] ?? activity.activity_type,
						title: activity.name
					}
				}
			});
		}
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

			const originalCourse = courses.find((c) => c.id === item.classroom_course_id);
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

		// Precompute drop validity for MOVE drags (colorize cells 🟢🔵🔴)
		if (type === 'MOVE' && draggedEntryId) {
			validateTimetableMoves(draggedEntryId)
				.then((res) => {
					const m = new Map<string, import('$lib/api/timetable').MoveValidityCell>();
					for (const c of res.data ?? []) {
						m.set(`${c.day_of_week}|${c.period_id}`, c);
					}
					moveValidityMap = m;
				})
				.catch(() => {
					moveValidityMap = new Map();
				});
		}

		if (event.dataTransfer) {
			event.dataTransfer.effectAllowed = type === 'NEW' ? 'copy' : 'move';
			event.dataTransfer.setData(
				'text/plain',
				JSON.stringify({
					type,
					id: type === 'NEW' ? item.id : item.id
				})
			);

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
						title:
							courseToCheck.title_th || courseToCheck.title_en || courseToCheck.title || 'รายวิชา',
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
		currentDragTarget = null;
		occupiedSlots = new Set();
		moveValidityMap = new Map();
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

		// Instructor view: เช็ค hidden ghost ใน rawTeamEntries (sync, ครอบคลุม ghost mode off
		// ที่ getEntryForSlot return undefined เพราะ filter display)
		if (viewMode === 'INSTRUCTOR' && draggedCourse) {
			const draggedClassroomId = draggedCourse.classroom_id;
			if (draggedClassroomId) {
				const hiddenEntry = rawTeamEntries.find(
					(e) =>
						e.day_of_week === day &&
						e.period_id === periodId &&
						e.classroom_id === draggedClassroomId &&
						e.id !== draggedEntryId
				);
				if (hiddenEntry && !(hiddenEntry.instructor_ids ?? []).includes(selectedInstructorId)) {
					toast.error('ห้องนี้มีวิชาอื่นอยู่ในคาบนี้แล้ว (คุณไม่ได้สอน) — วางทับไม่ได้');
					handleDragEnd();
					return;
				}
			}
		}

		// Block drop onto ghost cells ที่ displayed (ghost mode on) — ghost ไม่ใช่คาบของเรา
		if (
			existingEntry &&
			viewMode === 'INSTRUCTOR' &&
			selectedInstructorId &&
			!(existingEntry.instructor_ids ?? []).includes(selectedInstructorId)
		) {
			toast.error('สลับกับคาบในทีมไม่ได้ — คาบนี้ไม่ใช่คาบของคุณ');
			handleDragEnd();
			return;
		}

		// Case A: MOVE drag (from table) onto occupied → SWAP
		if (existingEntry && dragType === 'MOVE' && draggedEntryId && existingEntry.id !== draggedEntryId) {
			const validity = moveValidityMap.get(`${day}|${periodId}`);
			if (validity && !validity.valid) {
				toast.error(validity.reason || 'สลับไม่ได้');
				handleDragEnd();
				return;
			}
			try {
				submitting = true;
				await swapTimetableEntries(draggedEntryId, existingEntry.id);
				toast.success('สลับคาบเรียบร้อย');
				await loadTimetable();
				if ($authStore.user) {
					sendTimetableEvent({ type: 'TableRefresh', payload: { user_id: $authStore.user.id } });
				}
			} catch (e: any) {
				toast.error(e.message || 'สลับไม่สำเร็จ');
			} finally {
				submitting = false;
				handleDragEnd();
			}
			return;
		}

		// Case B: NEW drag (from sidebar) onto occupied → REPLACE
		if (existingEntry && dragType === 'NEW') {
			try {
				submitting = true;
				const payload: any = {};
				if (draggedCourse._isActivity) {
					payload.activity_slot_id = draggedCourse.activity_slot_id;
					payload.classroom_course_id = null;
				} else {
					payload.classroom_course_id = draggedCourse.id;
					payload.activity_slot_id = null;
					// ถ้า replace ข้ามห้อง (instructor view) → update classroom_id ให้ตรง course ใหม่
					if (
						draggedCourse.classroom_id &&
						draggedCourse.classroom_id !== existingEntry.classroom_id
					) {
						payload.classroom_id = draggedCourse.classroom_id;
					}
				}
				const result: any = await updateTimetableEntry(existingEntry.id, payload);
				if (result?.success === false) {
					// Backend rejected — รวบข้อความ conflict ทุกอันเป็นไทยเดียว
					// ไม่ใช้ result.message ("Conflict detected") เพราะไม่สื่อ
					const msgs: string[] = (result.conflicts ?? [])
						.map((c: any) => c.message)
						.filter(Boolean);
					toast.error(msgs.length > 0 ? msgs.join(' · ') : 'แทนที่ไม่สำเร็จ');
				} else {
					toast.success('แทนที่รายการเดิมแล้ว');
					await loadTimetable();
					if ($authStore.user) {
						sendTimetableEvent({ type: 'TableRefresh', payload: { user_id: $authStore.user.id } });
					}
				}
			} catch (e: any) {
				toast.error(e.message || 'แทนที่ไม่สำเร็จ');
			} finally {
				submitting = false;
				handleDragEnd();
			}
			return;
		}

		// Case C: dropping onto own source — no-op
		if (existingEntry && existingEntry.id === draggedEntryId) {
			handleDragEnd();
			return;
		}

		if (isSlotOccupiedByInstructor(day, periodId)) {
			toast.error(viewMode === 'INSTRUCTOR' ? 'ห้องนี้มีวิชาอื่นในคาบนี้แล้ว' : 'ครูติดสอนในคาบนี้แล้ว');
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
			res.data.forEach((entry) => {
				if (entry.period_id === periodId && entry.room_id) {
					if (dragType === 'MOVE' && entry.id === draggedEntryId) return;
					busyRooms.add(entry.room_id);
				}
			});
			unavailableRooms = busyRooms;
		} catch (e) {
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
				let payload: any;

				if (course._isActivity) {
					const activityClassroomId = course._classroom_id || selectedClassroomId;
					// Check if independent slot has instructor assigned for this classroom
					const slot = sidebarActivitySlots.find((s) => s.id === course.activity_slot_id)
						|| instructorActivityItems.find((i) => i.slot.id === course.activity_slot_id)?.slot;
					if (slot?.scheduling_mode === 'independent') {
						const hasInstructor = await checkClassroomHasInstructor(course.activity_slot_id, activityClassroomId);
						if (!hasInstructor) {
							toast.error('กรุณากำหนดครูประจำห้องนี้ก่อนในหน้ากิจกรรม');
							submitting = false;
							pendingDropContext = null;
							isDropPending = false;
							handleDragEnd();
							return;
						}
					}
					// Activity slot drop
					payload = {
						activity_slot_id: course.activity_slot_id,
						classroom_id: activityClassroomId,
						academic_semester_id: selectedSemesterId,
						day_of_week: day,
						period_id: periodId,
						room_id: roomId,
						entry_type: 'ACTIVITY',
						title: course.title_th
					};
				} else {
					payload = {
						classroom_course_id: course.id,
						day_of_week: day,
						period_id: periodId,
						room_id: roomId
					};
				}

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
			// Backend broadcasts patch event ให้แล้ว — ไม่ต้องส่ง TableRefresh ซ้ำ

			submitting = false;
			pendingDropContext = null;
			isDropPending = false;
			handleDragEnd();
		}
	}

	async function handleResponse(res: any, successMessage: string) {
		if (res.success === false) {
			// รวบ conflict messages เป็นไทยเดียว ไม่โชว์ "Conflict detected"
			const msgs: string[] = (res.conflicts ?? []).map((c: any) => c.message).filter(Boolean);
			toast.error(msgs.length > 0 ? msgs.join(' · ') : 'พบข้อขัดแย้งในตาราง');
		} else {
			await loadTimetable();
			toast.success(successMessage);
		}
	}

	async function handleExportPDF() {
		showExportModal = true;
		// Default to current view settings
		exportType = viewMode;
		exportTargetIds = [];
		if (viewMode === 'CLASSROOM' && selectedClassroomId) {
			exportTargetIds = [selectedClassroomId];
		} else if (viewMode === 'INSTRUCTOR' && selectedInstructorId) {
			exportTargetIds = [selectedInstructorId];
		}
	}

	async function confirmExport() {
		if (exportTargetIds.length === 0) return;

		try {
			isExporting = true;
			let successCount = 0;
			let failCount = 0;

			const total = exportTargetIds.length;

			// Loop through each target
			for (let i = 0; i < total; i++) {
				const id = exportTargetIds[i];
				try {
					// Fetch Data
					let entries: TimetableEntry[] = [];
					let targetName = '';

					if (exportType === 'CLASSROOM') {
						const room = classrooms.find((c) => c.id === id);
						if (!room) continue;
						targetName = `ห้อง ${room.name}`;
						const res = await listTimetableEntries({
							classroom_id: id,
							academic_semester_id: selectedSemesterId
						});
						entries = res.data;
					} else {
						const teacher = instructors.find((inst) => inst.id === id);
						if (!teacher) continue;
						targetName = teacher.name.startsWith('ครู') ? teacher.name : `ครู${teacher.name}`;
						const res = await listTimetableEntries({
							instructor_id: id,
							academic_semester_id: selectedSemesterId
						});
						entries = res.data;
					}

					// Prepare PDF Metadata
					const semesterName = semesters.find((s) => s.id === selectedSemesterId)?.term || '';
					const yearObj = academicYears.find((y) => y.id === selectedYearId);
					const yearName = (yearObj?.name || '').replace('ปีการศึกษา', '').trim();

					let title = '';
					if (exportType === 'CLASSROOM') {
						// Assuming room.name is something like "ม.4/1" or "4/1"
						// User request: "ตารางเรียน ชั้นมัธยมศึกษาปีที่ 4/1"
						// We will use "ชั้น" + targetName (which is "ห้อง ...")
						// Wait, targetName was constructed as `ห้อง ${room.name}`.
						// Let's reconstruct cleanly.
						const room = classrooms.find((c) => c.id === id);
						let roomName = room ? room.name : '';

						// Expand abbreviated prefixes
						if (roomName.startsWith('ม.')) roomName = roomName.replace('ม.', 'มัธยมศึกษาปีที่ ');
						else if (roomName.startsWith('ป.'))
							roomName = roomName.replace('ป.', 'ประถมศึกษาปีที่ ');
						else if (roomName.startsWith('อ.')) roomName = roomName.replace('อ.', 'อนุบาลปีที่ ');
						else if (/^\d/.test(roomName)) roomName = `มัธยมศึกษาปีที่ ${roomName}`; // Fallback for plain "4/1"

						title = `ตารางเรียน ชั้น${roomName}`;
					} else {
						title = `ตารางสอน ${targetName}`;
					}
					const subTitle = `ภาคเรียนที่ ${semesterName} ปีการศึกษา ${yearName}`;

					// Generate PDF
					await generateTimetablePDF(title, subTitle, periods, entries, exportType);
					successCount++;

					// Small delay to prevent browser choking/blocking multiple downloads
					if (i < total - 1) await new Promise((r) => setTimeout(r, 500));
				} catch (err) {
					console.error(`Failed to export ${id}`, err);
					failCount++;
				}
			}

			if (failCount === 0) {
				toast.success(`ดาวน์โหลดเสร็จสิ้น ${successCount} รายการ`);
				showExportModal = false;
			} else {
				toast.warning(`ดาวน์โหลดสำเร็จ ${successCount} รายการ, ล้มเหลว ${failCount} รายการ`);
			}
		} catch (e: any) {
			toast.error('เกิดข้อผิดพลาดในการดาวน์โหลด');
			console.error(e);
		} finally {
			isExporting = false;
		}
	}

	let unscheduledCourses = $derived.by(() => {
		// ในมุมมองครูใช้ rawTeamEntries (รวม ghost) เพื่อให้นับครบทุก cell ของ course
		// ไม่ได้นับเฉพาะ cell ที่ตัวเองอยู่ใน tei — ป้องกัน drag ซ้ำแล้วเกินคาบ
		const sourceEntries = viewMode === 'INSTRUCTOR' ? rawTeamEntries : timetableEntries;
		const courseCounts = new Map<string, number>();
		sourceEntries.forEach((entry) => {
			if (entry.classroom_course_id) {
				const count = courseCounts.get(entry.classroom_course_id) || 0;
				courseCounts.set(entry.classroom_course_id, count + 1);
			}
		});

		return courses
			.map((course) => {
				const scheduled = courseCounts.get(course.id) || 0;
				const credit = course.subject_credit || 0;
				const hours = course.subject_hours;
				// Explicit 0 = วิชานี้ไม่ต้องจัดตาราง (เก็บคะแนนอย่างเดียว) → maxPeriods = 0
				// null/undefined → fallback คำนวณจาก credit
				// >0 → คำนวณจาก hours/20
				const maxPeriods =
					hours === 0
						? 0
						: hours && hours > 0
							? Math.ceil(hours / 20)
							: credit > 0
								? Math.ceil(credit * 2)
								: 3;

				return {
					...course,
					scheduled_count: scheduled,
					max_periods: maxPeriods,
					is_completed: maxPeriods === 0 || scheduled >= maxPeriods
				};
			})
			.filter((c) => !c.is_completed)
			.sort((a, b) => {
				// Sort by code?
				return a.subject_code.localeCompare(b.subject_code);
			});
	});

	$effect(() => {
		if (selectedYearId) {
			loadClassrooms();
			loadPeriods();

			const yearSemesters = allSemesters.filter((s) => s.academic_year_id === selectedYearId);
			if (!yearSemesters.find((s) => s.id === selectedSemesterId)) {
				const activeOrFirst = yearSemesters.find((s) => s.is_active) || yearSemesters[0];
				selectedSemesterId = activeOrFirst ? activeOrFirst.id : '';
			}
		}
	});

	type UnscheduledActivity = ActivitySlot & {
		scheduled_count: number;
		max_periods: number;
		is_completed: boolean;
		is_draggable: boolean;
		_classroom_id?: string;
		_classroom_name?: string;
	};

	let unscheduledActivities: UnscheduledActivity[] = $derived.by(() => {
		const slotCounts = new Map<string, number>();
		timetableEntries.forEach((entry) => {
			if (entry.activity_slot_id) {
				// For INSTRUCTOR view with independent: count per slot+classroom
				const key = viewMode === 'INSTRUCTOR' && entry.classroom_id
					? `${entry.activity_slot_id}:${entry.classroom_id}`
					: entry.activity_slot_id;
				slotCounts.set(key, (slotCounts.get(key) || 0) + 1);
			}
		});

		if (viewMode === 'INSTRUCTOR') {
			const items: UnscheduledActivity[] = [];

			// Synchronized slots (read-only)
			for (const slot of sidebarActivitySlots) {
				const scheduled = slotCounts.get(slot.id) || 0;
				if (scheduled < slot.periods_per_week) {
					items.push({
						...slot,
						scheduled_count: scheduled,
						max_periods: slot.periods_per_week,
						is_completed: false,
						is_draggable: false,
						_classroom_id: undefined,
						_classroom_name: undefined,
					});
				}
			}

			// Independent items per classroom (draggable)
			for (const item of instructorActivityItems) {
				const key = `${item.slot.id}:${item.classroom_id}`;
				const scheduled = slotCounts.get(key) || 0;
				if (scheduled < item.slot.periods_per_week) {
					items.push({
						...item.slot,
						name: `${item.slot.name} — ${item.classroom_name}`,
						scheduled_count: scheduled,
						max_periods: item.slot.periods_per_week,
						is_completed: false,
						is_draggable: true,
						_classroom_id: item.classroom_id,
						_classroom_name: item.classroom_name,
					});
				}
			}

			return items;
		}

		return sidebarActivitySlots.map((slot) => {
			const scheduled = slotCounts.get(slot.id) || 0;
			const maxPeriods = slot.periods_per_week;
			return {
				...slot,
				scheduled_count: scheduled,
				max_periods: maxPeriods,
				is_completed: scheduled >= maxPeriods,
				is_draggable: slot.scheduling_mode === 'independent',
				_classroom_id: undefined as string | undefined,
				_classroom_name: undefined as string | undefined,
			};
		}).filter((s) => !s.is_completed);
	});

	$effect(() => {
		if (viewMode === 'CLASSROOM' && selectedClassroomId) {
			loadCourses();
			loadTimetable();
			loadSidebarActivitySlots();

			// Broadcast View Context
			if ($authStore.user) {
				sendTimetableEvent({
					type: 'CursorMove',
					payload: {
						user_id: $authStore.user.id,
						x: 0,
						y: 0,
						context: {
							view_mode: 'CLASSROOM',
							view_id: selectedClassroomId
						}
					}
				});
			}
		} else if (viewMode === 'INSTRUCTOR' && selectedInstructorId) {
			loadCourses();
			loadTimetable();
			loadInstructorGroups();
			loadSidebarActivitySlots();

			// Broadcast View Context
			if ($authStore.user) {
				sendTimetableEvent({
					type: 'CursorMove',
					payload: {
						user_id: $authStore.user.id,
						x: 0,
						y: 0,
						context: {
							view_mode: 'INSTRUCTOR',
							view_id: selectedInstructorId
						}
					}
				});
			}
		}
	});

	$effect(() => {
		if (selectedSemesterId) {
			if (
				(viewMode === 'CLASSROOM' && selectedClassroomId) ||
				(viewMode === 'INSTRUCTOR' && selectedInstructorId)
			) {
				loadCourses();
				loadTimetable();
			}
		}
	});

	// Batch Assign State
	let showBatchModal = $state(false);
	let batchClassrooms = $state<string[]>([]);
	let batchDay = $state('MON');
	let batchPeriodIds = $state<string[]>([]);
	let batchType = $state('ACTIVITY');
	let batchTitle = $state('');
	let batchRoomId = $state('none');

	// Batch Mode State
	let batchMode = $state<'TEXT' | 'COURSE' | 'SLOT'>('TEXT');
	let subjectOptions = $state<LookupItem[]>([]);
	let batchSubjectId = $state('');
	let loadingSubjects = $state(false);

	// Activity Slot mode
	let activitySlots = $state<ActivitySlot[]>([]);
	let batchSlotId = $state('');
	let loadingSlots = $state(false);

	async function ensureActivitySlotsLoaded() {
		if (activitySlots.length > 0 || !selectedSemesterId) return;
		loadingSlots = true;
		try {
			const res = await listActivitySlots({ semester_id: selectedSemesterId });
			activitySlots = res.data;
		} catch (e) {
			console.error(e);
			toast.error('โหลดข้อมูล Activity Slot ไม่สำเร็���');
		} finally {
			loadingSlots = false;
		}
	}

	async function ensureSubjectsLoaded() {
		if (subjectOptions.length > 0) return;
		loadingSubjects = true;
		try {
			subjectOptions = await lookupSubjects({
				limit: 500,
				activeOnly: true,
				subjectType: 'ACTIVITY'
			});
		} catch (e) {
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
		} catch (e) {
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
				.then((res) => {
					if (batchSubjectId !== currentSubject) return; // Stale check
					const ids = new Set(res.data.map((c: any) => c.classroom_id));
					validBatchClassroomIds = ids;
				})
				.catch((err) => {
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
				list = list.filter((c) => validSet.has(c.id));
			} else {
				list = [];
			}
		}

		// SLOT mode: filter by ห้องที่เข้าร่วม slot จริง (junction) ไม่ใช่ catalog template
		if (batchMode === 'SLOT' && batchSlotId) {
			const slot = activitySlots.find((s) => s.id === batchSlotId);
			if (slot?.classroom_ids) {
				list = list.filter((c) => slot.classroom_ids!.includes(c.id));
			}
		}

		// Apply Manual Filter (Intersection with Plan)
		if (batchGradeFilterId !== 'all') {
			list = list.filter((c) => c.grade_level_id === batchGradeFilterId);
		}

		return list;
	});

	function toggleBatchClassroom(id: string) {
		if (batchClassrooms.includes(id)) {
			batchClassrooms = batchClassrooms.filter((c) => c !== id);
		} else {
			batchClassrooms = [...batchClassrooms, id];
		}
	}

	function selectAllBatchClassrooms() {
		// Only select currently filtered items
		const currentListIds = filteredBatchClassroomsList.map((c) => c.id);

		// If all currently visible are selected -> Deselect All (visible)
		const allVisibleSelected = currentListIds.every((id) => batchClassrooms.includes(id));

		if (allVisibleSelected) {
			batchClassrooms = batchClassrooms.filter((id) => !currentListIds.includes(id));
		} else {
			// Add missing ones
			const newIds = currentListIds.filter((id) => !batchClassrooms.includes(id));
			batchClassrooms = [...batchClassrooms, ...newIds];
		}
	}

	async function handleBatchSubmit() {
		if (batchClassrooms.length === 0) {
			toast.error('กรุณาเลือกห้องเรียนอย่างน้อย 1 ห้อง');
			return;
		}
		if (batchPeriodIds.length === 0) {
			toast.error('กรุณาเลือกคาบเวลาอย่างน้อย 1 คาบ');
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
		if (batchMode === 'SLOT' && !batchSlotId) {
			toast.error('กรุณาเลือก Activity Slot');
			return;
		}

		try {
			submitting = true;

			let titleToSend = batchTitle;
			let entryTypeToSend = batchType;
			let subjectIdToSend = undefined;
			let slotIdToSend: string | undefined = undefined;

			if (batchMode === 'COURSE') {
				const subj = subjectOptions.find((s) => s.id === batchSubjectId);
				titleToSend = subj?.name || '';
				entryTypeToSend = 'ACTIVITY';
				subjectIdToSend = batchSubjectId;
			} else if (batchMode === 'SLOT') {
				const slot = activitySlots.find((s) => s.id === batchSlotId);
				titleToSend = slot?.name || '';
				entryTypeToSend = 'ACTIVITY';
				slotIdToSend = batchSlotId;
			}

			const res = await createBatchTimetableEntries({
				classroom_ids: batchClassrooms,
				day_of_week: batchDay,
				period_ids: batchPeriodIds,
				academic_semester_id: selectedSemesterId,
				entry_type: entryTypeToSend as any,
				title: titleToSend,
				room_id: batchRoomId === 'none' ? undefined : batchRoomId,
				subject_id: subjectIdToSend,
				force: batchForce,
				activity_slot_id: slotIdToSend
			});

			if (res.success === false && res.conflicts) {
				toast.error(res.message || 'พบรายการที่ชนกัน');
				for (const c of res.conflicts) {
					toast.error(c.message);
				}
				submitting = false;
				return;
			}

			toast.success('บันทึกกิจกรรมเรียบร้อย');
			showBatchModal = false;

			// Reset fields
			batchTitle = '';
			batchSubjectId = '';
			batchSlotId = '';

			// Reload if current view is affected
			if (
				viewMode === 'CLASSROOM' &&
				selectedClassroomId &&
				batchClassrooms.includes(selectedClassroomId)
			) {
				loadTimetable();
			}
		} catch (e: any) {
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
	let workspaceRef: HTMLDivElement;
	let wsRect = $state<DOMRect | null>(null);
	function handleMouseMove(e: MouseEvent) {
		const now = Date.now();
		if (now - lastCursorSend > 50 && $authStore.user && workspaceRef) {
			// 20fps cap
			lastCursorSend = now;

			// Send percentage coords (0-1) — works across any screen size
			const rect = workspaceRef.getBoundingClientRect();
			wsRect = rect;
			const x = (e.clientX - rect.left) / rect.width;
			const y = (e.clientY - rect.top) / rect.height;

			const currentViewId = viewMode === 'CLASSROOM' ? selectedClassroomId : selectedInstructorId;

			sendTimetableEvent({
				type: 'CursorMove',
				payload: {
					user_id: $authStore.user.id,
					x,
					y,
					context: {
						view_mode: viewMode,
						view_id: currentViewId
					}
				}
			});
		}
	}

	let lastDragSend = 0;
	let currentDragTarget = $state<{ day: string; periodId: string } | null>(null);

	function handleDragMoveOnGrid(e: DragEvent) {
		// HTML5 drag event fires during drag — use it to send cursor position
		if (!draggedCourse || !$authStore.user || !workspaceRef) return;
		// Chrome sometimes fires drag with clientX=0,clientY=0 — ignore those
		if (e.clientX === 0 && e.clientY === 0) return;

		const now = Date.now();
		if (now - lastDragSend < 50) return; // throttle 20fps
		lastDragSend = now;

		const rect = workspaceRef.getBoundingClientRect();
		wsRect = rect;
		const x = (e.clientX - rect.left) / rect.width;
		const y = (e.clientY - rect.top) / rect.height;

		// Find target cell from element under cursor
		const el = document.elementFromPoint(e.clientX, e.clientY);
		const cell = el?.closest('[data-day][data-period]') as HTMLElement | null;
		const targetDay = cell?.dataset.day;
		const targetPeriod = cell?.dataset.period;

		currentDragTarget = targetDay && targetPeriod ? { day: targetDay, periodId: targetPeriod } : null;

		sendTimetableEvent({
			type: 'DragMove',
			payload: {
				user_id: $authStore.user.id,
				x,
				y,
				target_day: targetDay,
				target_period_id: targetPeriod
			}
		});
	}

	// Auto Refresh Listener (fallback: TableRefresh หรือ gap-reconcile-refetch)
	$effect(() => {
		if ($refreshTrigger > 0) {
			console.log('Auto-refreshing timetable...');
			loadTimetable();
			loadCourses();
		}
	});

	// Patch subscriber — apply realtime patches โดยไม่ต้อง fetch DB
	$effect(() => {
		const patch = $lastPatch;
		if (!patch) return;
		applyPatchToState(patch);
		lastPatch.set(null);
	});

	function applyPatchToState(patch: TimetablePatch) {
		// Helper: อัปเดต entry ใน array ทั้ง timetableEntries + rawTeamEntries
		const updateEntries = (fn: (arr: TimetableEntry[]) => TimetableEntry[]) => {
			timetableEntries = fn(timetableEntries);
			rawTeamEntries = fn(rawTeamEntries);
		};

		switch (patch.type) {
			case 'EntryCreated':
				// create → backend ยังไม่ส่ง full entry (ใช้ TableRefresh) — case นี้ยังไม่เจอในปัจจุบัน
				// ถ้าจะ patch ต้อง re-fetch เพราะ backend entry ไม่มี joined fields
				refreshTrigger.update((n) => n + 1);
				break;
			case 'EntryUpdated': {
				const updated = patch.entry;
				const isRelevant = (e: TimetableEntry) => e.id === updated.id;
				updateEntries((arr) => {
					const found = arr.some(isRelevant);
					if (found) {
						return arr.map((e) => (isRelevant(e) ? { ...e, ...updated } : e));
					}
					// Entry ไม่อยู่ใน state ปัจจุบัน (เช่น view เปลี่ยนไปห้อง/ครูคนอื่น) → ไม่ต้องทำอะไร
					return arr;
				});
				break;
			}
			case 'EntryDeleted':
				updateEntries((arr) => arr.filter((e) => e.id !== patch.entry_id));
				break;
			case 'EntriesSwapped':
				// Swap = 2 entries เปลี่ยน day/period — apply ทั้งคู่
				updateEntries((arr) =>
					arr.map((e) => {
						if (e.id === patch.entry_a.id) return { ...e, ...patch.entry_a };
						if (e.id === patch.entry_b.id) return { ...e, ...patch.entry_b };
						return e;
					})
				);
				break;
			case 'EntryInstructorAdded':
				updateEntries((arr) =>
					arr.map((e) => {
						if (e.id !== patch.entry_id) return e;
						const ids = [...(e.instructor_ids ?? []), patch.instructor_id];
						const names = [...(e.instructor_names ?? []), patch.instructor_name];
						return { ...e, instructor_ids: ids, instructor_names: names };
					})
				);
				break;
			case 'EntryInstructorRemoved':
				if (patch.entry_deleted) {
					updateEntries((arr) => arr.filter((e) => e.id !== patch.entry_id));
				} else {
					updateEntries((arr) =>
						arr.map((e) => {
							if (e.id !== patch.entry_id) return e;
							const idx = e.instructor_ids?.indexOf(patch.instructor_id) ?? -1;
							if (idx < 0) return e;
							return {
								...e,
								instructor_ids: e.instructor_ids!.filter((_, i) => i !== idx),
								instructor_names: e.instructor_names?.filter((_, i) => i !== idx)
							};
						})
					);
				}
				break;
			case 'CourseTeamChanged':
				// Team วิชาเปลี่ยน — entries ของ course นั้นทุก cell ต้อง re-fetch เพราะครู array เปลี่ยน
				// ใช้ TableRefresh fallback
				refreshTrigger.update((n) => n + 1);
				break;
		}
	}

	function getDragOwner(entryId?: string, courseId?: string) {
		if (!entryId && !courseId) return null;
		for (const [userId, drag] of Object.entries($userDrags)) {
			// Strict check: Only lock if entry_id matches (Move)
			// or if dragging NEW course (courseId matches, but only if we want to lock new drags?)

			// Request: Don't lock existing entries if dragging NEW course.
			if (entryId) {
				// Only lock if someone is dragging THIS SPECIFIC entry (Move)
				if (drag.entry_id === entryId) return $activeUsers.find((u) => u.user_id === userId);
			}
			// If this is a course list item
			else if (courseId) {
				// Lock list item if someone is dragging this course
				if (drag.course_id === courseId && !drag.entry_id)
					return $activeUsers.find((u) => u.user_id === userId);
			}
		}
		return null;
	}

	function getRemoteDragHover(day: string, periodId: string) {
		for (const [userId, pos] of Object.entries($dragPositions)) {
			if (pos.target_day === day && pos.target_period_id === periodId) {
				const user = $activeUsers.find((u) => u.user_id === userId);
				const drag = $userDrags[userId];
				if (user && drag) return { user, drag };
			}
		}
		return null;
	}

	onMount(loadInitialData);
</script>

<svelte:head>
	<title>{data.title} - SchoolOrbit</title>
</svelte:head>

<div
	class="h-full flex flex-col space-y-4"
	role="application"
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

			<div class="w-px h-4 bg-border mx-1"></div>

			<!-- Quick Actions -->
			<Button
				variant="ghost"
				size="sm"
				class="h-7 text-xs gap-1.5"
				onclick={() => {
					goto('/staff/academic/timetable/scheduling/auto-schedule');
				}}
			>
				<Zap class="w-3.5 h-3.5 text-orange-500" />
				จัดตารางสอนอัตโนมัติ
			</Button>

			{#if $isConnected && $activeUsers.length > 0}
				<div class="w-px h-4 bg-border mx-1"></div>
				<Tooltip.Provider>
					<div class="flex -space-x-1.5">
						{#each $activeUsers.slice(0, 4) as user (user.user_id + (user.context?.view_id || ''))}
							<!-- Interactive Avatar -->
							<Tooltip.Root>
								<Tooltip.Trigger>
									<button
										class="w-6 h-6 rounded-full border-2 border-white flex items-center justify-center text-[9px] text-white font-bold ring-1 ring-border/10 shadow-sm transition-transform hover:scale-110 hover:z-10 cursor-pointer"
										style="background-color: {user.color}"
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
								</Tooltip.Trigger>
								<Tooltip.Content>
									<p class="font-semibold">{user.name}</p>
									{#if user.context?.view_id}
										{#if user.context.view_mode === 'CLASSROOM'}
											<p class="text-xs text-muted-foreground">
												กำลังดูห้อง {classrooms.find((c) => c.id === user.context?.view_id)?.name ||
													'(ไม่พบข้อมูลห้อง)'}
											</p>
										{:else if user.context.view_mode === 'INSTRUCTOR'}
											<p class="text-xs text-muted-foreground">
												กำลังดูตารางสอน {instructors.find((i) => i.id === user.context?.view_id)
													?.name || '(ไม่พบข้อมูลครู)'}
											</p>
										{/if}
									{/if}
								</Tooltip.Content>
							</Tooltip.Root>
						{/each}
						{#if $activeUsers.length > 4}
							<div
								class="w-6 h-6 rounded-full bg-muted border-2 border-white flex items-center justify-center text-[8px] font-bold shadow-sm"
							>
								+{$activeUsers.length - 4}
							</div>
						{/if}
					</div>
				</Tooltip.Provider>
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

			<div class="flex items-center gap-2">
				<Button variant="outline" size="sm" onclick={() => (showBatchModal = true)}>
					<PlusCircle class="w-4 h-4 mr-2" /> เพิ่มกิจกรรมพิเศษ (Batch)
				</Button>

				<Button variant="outline" size="sm" onclick={handleExportPDF} disabled={isExporting}>
					{#if isExporting}
						<Loader2 class="w-4 h-4 mr-2 animate-spin" />
					{:else}
						<Download class="w-4 h-4 mr-2" />
					{/if}
					ดาวน์โหลด PDF
				</Button>
			</div>
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
						{#if selectedSemesterId && semesters.find((s) => s.id === selectedSemesterId)}
							ภาคเรียนที่ {semesters.find((s) => s.id === selectedSemesterId)?.term}
						{:else}
							เลือกภาคเรียน
						{/if}
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
				{#if selectedInstructorId}
					<label class="flex items-center gap-2 text-xs cursor-pointer select-none px-2 py-1 rounded border bg-muted/30 hover:bg-muted transition-colors">
						<input type="checkbox" bind:checked={showTeamGhosts} class="cursor-pointer" />
						<span>แสดงคาบในทีม (ghost cells)</span>
					</label>
				{/if}
			{/if}
		</div>
	</div>

	<!-- Main Content Grid (Workspace = cursor canvas) -->
	<div
		class="grid grid-cols-12 gap-6 h-[calc(100vh-250px)] min-h-[600px] relative"
		bind:this={workspaceRef}
		onmousemove={handleMouseMove}
		ondrag={handleDragMoveOnGrid}
		role="application"
	>
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
						class="border rounded-lg p-3 shadow-sm cursor-grab active:cursor-grabbing hover:shadow-md hover:brightness-95 transition-all group relative {lockedBy
							? 'opacity-50 pointer-events-none'
							: ''}"
						style="background-color: {getSubjectColor(
							viewMode === 'INSTRUCTOR'
								? course.classroom_name || course.subject_code
								: course.subject_code
						)}; border-color: {getSubjectBorderColor(
							viewMode === 'INSTRUCTOR'
								? course.classroom_name || course.subject_code
								: course.subject_code
						)};"
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

			<!-- Activity Slots Section -->
			{#if unscheduledActivities.length > 0}
				<div class="border-t">
					<div class="py-2 px-4 bg-emerald-50 border-b">
						<span class="text-xs font-medium text-emerald-700 flex items-center gap-1">
							<CalendarDays class="w-3 h-3" /> กิจกรรมพัฒนาผู้เรียน
						</span>
					</div>
					<div class="overflow-y-auto p-3 space-y-2 max-h-[200px]">
						{#each unscheduledActivities as activity}
							{#if activity.is_draggable}
								<!-- Independent: draggable -->
								<div
									class="border rounded-lg p-2.5 shadow-sm cursor-grab active:cursor-grabbing hover:shadow-md transition-all bg-emerald-50 border-emerald-200"
									draggable={true}
									ondragstart={(e) => handleActivityDragStart(e, activity)}
									ondragend={handleDragEnd}
									role="button"
									tabindex="0"
								>
									<div class="flex justify-between items-start mb-1">
										<Badge variant="outline" class="text-[10px] border-emerald-300 text-emerald-700">
											{ACTIVITY_TYPE_LABELS[activity.activity_type] ?? activity.activity_type}
										</Badge>
										<Badge variant="default" class="text-[10px] bg-emerald-600">
											{activity.scheduled_count}/{activity.max_periods} คาบ
										</Badge>
									</div>
									<h4 class="font-medium text-sm line-clamp-1 leading-tight">{activity.name}</h4>
									<div class="text-[10px] text-emerald-600 mt-1">อิสระ — ลากวางได้</div>
									<div class="mt-1.5 h-1 w-full bg-emerald-100 rounded-full overflow-hidden">
										<div
											class="h-full bg-emerald-500 transition-all"
											style="width: {(activity.scheduled_count / activity.max_periods) * 100}%"
										></div>
									</div>
								</div>
							{:else}
								<!-- Synchronized: read-only -->
								<div
									class="border border-dashed rounded-lg p-2.5 opacity-60 bg-gray-50 border-gray-300"
								>
									<div class="flex justify-between items-start mb-1">
										<Badge variant="outline" class="text-[10px]">
											{ACTIVITY_TYPE_LABELS[activity.activity_type] ?? activity.activity_type}
										</Badge>
										<Badge variant="secondary" class="text-[10px]">
											{activity.scheduled_count}/{activity.max_periods} คาบ
										</Badge>
									</div>
									<h4 class="font-medium text-sm line-clamp-1 leading-tight flex items-center gap-1">
										<Lock class="w-3 h-3 shrink-0" /> {activity.name}
									</h4>
									{#if viewMode === 'INSTRUCTOR' && selectedInstructorId}
										<Button
											variant="outline"
											size="sm"
											class="mt-1 h-6 text-xs w-full"
											onclick={async () => {
												try {
													await restoreInstructorToSlot(activity.id, selectedInstructorId);
													toast.success('แสดงในตารางแล้ว');
													loadTimetable();
													loadSidebarActivitySlots();
												} catch (e: any) {
													toast.error(e.message || 'ไม่สำเร็จ');
												}
											}}
										>
											แสดงในตาราง
										</Button>
									{:else}
										<div class="text-[10px] text-muted-foreground mt-1">จัดพร้อมกัน — ใช้ Batch</div>
									{/if}
								</div>
							{/if}
						{/each}
					</div>
				</div>
			{/if}
		</Card.Root>

		<!-- Right Content: Timetable Grid -->
		<Card.Root class="col-span-10 flex flex-col h-full overflow-hidden border-2 shadow-none">
			<div class="overflow-auto flex-1">
				<div class="min-w-[800px] h-full flex flex-col">
					<!-- Header Row (Periods) -->
					<div class="flex sticky top-0 bg-background z-20">
						<div
							class="w-24 shrink-0 p-3 border-r border-b font-medium text-sm text-muted-foreground flex items-center justify-center bg-background sticky left-0 z-30"
						>
							วัน/คาบ
						</div>
						{#each periods as period}
							<div class="flex-1 min-w-[100px] p-2 border-r border-b text-center relative group">
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
								{@const remoteDrag = !entry ? getRemoteDragHover(day.value, period.id) : null}

								<!-- Drop Zone -->
								{@const validity = draggedCourse && dragType === 'MOVE'
									? moveValidityMap.get(`${day.value}|${period.id}`)
									: null}
								{@const validityClass = !validity
									? ''
									: validity.state === 'source'
										? 'opacity-60'
										: validity.state === 'empty' && validity.valid
											? 'bg-green-50/40 ring-1 ring-inset ring-green-400/60'
											: validity.state === 'occupied' && validity.valid
												? 'ring-1 ring-inset ring-blue-400/70 bg-blue-50/30'
												: 'bg-red-50/40 ring-1 ring-inset ring-red-300/60 cursor-not-allowed'}
								<div
									class="flex-1 border-r border-b min-w-[100px] relative transition-colors {isOccupied
										? 'bg-red-50/50 from-red-100/20 bg-gradient-to-br'
										: 'hover:bg-accent/50'} {draggedCourse && !entry && !isOccupied && !validity
										? 'bg-blue-50/30'
										: ''} {validityClass} {draggedCourse && entry && isOccupied && dragType === 'NEW'
										? 'ring-2 ring-inset ring-red-500/70'
										: ''} {remoteDrag ? 'ring-2 ring-inset ring-opacity-50' : ''}"
									style={remoteDrag ? `--tw-ring-color: ${remoteDrag.user.color}40; background-color: ${remoteDrag.user.color}10;` : ''}
									data-day={day.value}
									data-period={period.id}
									title={validity && !validity.valid ? validity.reason : ''}
									ondragover={handleDragOver}
									ondrop={(e) => handleDrop(e, day.value, period.id)}
									role="application"
								>
									{#if remoteDrag}
										<!-- Remote user drag ghost preview -->
										<div class="absolute inset-1 rounded border-2 border-dashed p-1.5 flex flex-col justify-center items-center gap-0.5 animate-in fade-in duration-200 pointer-events-none"
											style="border-color: {remoteDrag.user.color}80; background-color: {remoteDrag.user.color}15;"
										>
											<span class="text-[10px] font-bold truncate max-w-full" style="color: {remoteDrag.user.color}">
												{remoteDrag.drag.info?.code || 'วิชา'}
											</span>
											<span class="text-[9px] text-muted-foreground truncate max-w-full">
												{remoteDrag.drag.info?.title || ''}
											</span>
											<span class="text-[8px] font-medium px-1.5 py-0.5 rounded-full text-white mt-0.5"
												style="background-color: {remoteDrag.user.color};"
											>
												{remoteDrag.user.name}
											</span>
										</div>
									{/if}
									{#if entry && validity && validity.state === 'occupied' && validity.valid}
										<!-- Swap indicator overlay -->
										<div class="absolute top-0.5 right-0.5 z-10 bg-blue-500 text-white text-[9px] px-1 py-0.5 rounded font-bold pointer-events-none">
											⇄ สลับ
										</div>
									{/if}
									{#if entry}
										{@const isGhost =
											viewMode === 'INSTRUCTOR' &&
											selectedInstructorId !== '' &&
											!(entry.instructor_ids ?? []).includes(selectedInstructorId)}
										{@const coTeacherCount =
											viewMode === 'INSTRUCTOR'
												? Math.max(0, (entry.instructor_ids?.length ?? 0) - 1)
												: 0}
										<!-- Timetable Entry Card -->
										<div
											class="absolute inset-1 border rounded p-2 text-xs flex flex-col justify-between shadow-sm hover:shadow-md hover:brightness-95 transition-all group {entry.entry_type !== 'COURSE' || isGhost
												? 'cursor-pointer'
												: 'cursor-grab active:cursor-grabbing'} {lockedBy
												? 'opacity-50 pointer-events-none ring-2 ring-offset-1 ring-' +
													lockedBy.color
												: ''} {isGhost ? 'opacity-50 border-dashed' : ''}"
											style="background-color: {getSubjectColor(
												viewMode === 'INSTRUCTOR'
													? entry.classroom_name || entry.subject_code || ''
													: entry.subject_code || entry.title || '',
												entry.entry_type
											)}; border-color: {getSubjectBorderColor(
												viewMode === 'INSTRUCTOR'
													? entry.classroom_name || entry.subject_code || ''
													: entry.subject_code || entry.title || '',
												entry.entry_type
											)};"
											draggable={!lockedBy && entry.entry_type === 'COURSE' && !isGhost}
											ondragstart={(e) => handleDragStart(e, entry, 'MOVE')}
											ondragend={handleDragEnd}
											onclick={(e) => {
												if ((e.target as HTMLElement).closest('button')) return;
												openEntryPopover(entry);
											}}
											onkeydown={(e) => {
												if (e.key === 'Enter' || e.key === ' ') {
													e.preventDefault();
													openEntryPopover(entry);
												}
											}}
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
												{entry.subject_code
													|| entry.title
													|| (entry.entry_type === 'ACTIVITY' ? 'กิจกรรม' : '')}
											</div>
											<div
												class="line-clamp-1 text-blue-800 text-[10px] mb-auto"
												title={entry.subject_name_th || entry.title || undefined}
											>
												{entry.subject_name_th || entry.title || ''}
											</div>
											<div
												class="mt-1 pt-1 border-t border-blue-200/50 gap-0.5 flex flex-col text-[9px] text-blue-700"
											>
												{#if viewMode === 'CLASSROOM'}
													<div class="flex items-center gap-1 truncate">
														<Users class="w-3 h-3 shrink-0" />
														{(entry.instructor_names && entry.instructor_names.length > 0) ? entry.instructor_names.join(', ') : (entry.instructor_name || '-')}
													</div>
												{:else if entry.entry_type === 'ACTIVITY' && entry.activity_slot_id && entry.activity_scheduling_mode === 'independent'}
													<!-- Independent: แสดงชื่อห้อง -->
													<div class="flex items-center gap-1 truncate">
														<School class="w-3 h-3 shrink-0" />
														{entry.classroom_name || '-'}
													</div>
												{:else if entry.entry_type === 'ACTIVITY' && entry.activity_slot_id}
													<!-- Synchronized: แสดงชื่อกิจกรรมถ้ามี -->
													{@const groupName = instructorGroupsMap[entry.activity_slot_id]}
													<div class="flex items-center gap-1 truncate">
														<BookOpen class="w-3 h-3 shrink-0" />
														{#if groupName}
															{groupName}
														{:else}
															{entry.activity_slot_name || '-'}
														{/if}
													</div>
												{:else}
													<div class="flex items-center gap-1 truncate">
														<School class="w-3 h-3 shrink-0" />
														{entry.classroom_name || '-'}
													</div>
												{/if}

												{#if viewMode === 'INSTRUCTOR' && isGhost}
													<div class="flex items-center gap-1 text-amber-700 text-[9px]">
														<span>👻</span>
														<span>อยู่ในทีม (ยังไม่ได้สอนคาบนี้)</span>
													</div>
												{:else if viewMode === 'INSTRUCTOR' && coTeacherCount > 0}
													<div class="flex items-center gap-1 text-blue-600 text-[9px]" title={entry.instructor_names?.join(', ')}>
														<Users class="w-3 h-3 shrink-0" />
														<span>+{coTeacherCount} ครูร่วม</span>
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

											<!-- Delete Button (ghost cells ไม่แสดง — ยังไม่ใช่คาบของเรา) -->
											{#if !isGhost}
												<button
													class="absolute top-0.5 right-0.5 opacity-0 group-hover:opacity-100 p-0.5 hover:bg-red-100 hover:text-red-500 rounded transition-all z-30"
													onclick={(e) => {
														e.stopPropagation();
														handleDeleteEntry(entry);
													}}
												>
													<Trash2 class="w-3 h-3" />
												</button>
											{/if}
										</div>
									{:else if isOccupied}
										<div
											class="absolute inset-0 flex items-center justify-center p-2 text-center opacity-40 select-none"
										>
											<div class="text-xs text-red-500 font-medium">
											{viewMode === 'INSTRUCTOR' ? 'ห้องนี้ไม่ว่าง' : 'ครูติดสอน'}
										</div>
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

		<!-- GHOST UI OVERLAY (fixed, clipped to workspace via clip-path) -->
		<div
			class="pointer-events-none fixed inset-0 z-[9999]"
			style={wsRect ? `clip-path: inset(${wsRect.top}px ${typeof window !== 'undefined' ? window.innerWidth - wsRect.right : 0}px ${typeof window !== 'undefined' ? window.innerHeight - wsRect.bottom : 0}px ${wsRect.left}px)` : 'display:none'}
		>
			{#each $activeUsers as user (user.user_id)}
				{@const cursor = $remoteCursors[user.user_id]}

				{#if cursor && user.user_id !== $authStore.user?.id}
					{#if cursor.context?.view_mode === viewMode && cursor.context?.view_id === (viewMode === 'CLASSROOM' ? selectedClassroomId : selectedInstructorId)}
						<div
							class="absolute transition-transform duration-100 ease-linear flex flex-col items-start gap-1"
							style="transform: translate({cursor.x * (wsRect?.width ?? 0) + (wsRect?.left ?? 0)}px, {cursor.y * (wsRect?.height ?? 0) + (wsRect?.top ?? 0)}px);"
						>
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

							<div
								class="px-2 py-0.5 rounded text-[10px] text-white font-bold whitespace-nowrap shadow-sm"
								style="background-color: {user.color}"
							>
								{user.name}
							</div>

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
</div>

<!-- Per-cell Instructor Editor Popover -->
<Dialog.Root bind:open={entryPopoverOpen}>
	<Dialog.Content class="sm:max-w-[480px]">
		<Dialog.Header>
			<Dialog.Title>
				{#if entryPopoverTarget}
					{entryPopoverTarget.subject_code ?? ''} {entryPopoverTarget.subject_name_th ?? ''}
				{:else}
					แก้ไขครูในคาบนี้
				{/if}
			</Dialog.Title>
			<Dialog.Description>
				{#if entryPopoverTarget}
					{entryPopoverTarget.classroom_name} · {entryPopoverTarget.day_of_week} · {entryPopoverTarget.period_name ?? ''}
				{/if}
			</Dialog.Description>
		</Dialog.Header>

		<div class="py-2 space-y-3">
			{#if entryPopoverLoading}
				<div class="text-center py-4">
					<Loader2 class="w-5 h-5 animate-spin mx-auto" />
				</div>
			{:else if entryPopoverTarget}
				<div class="space-y-2">
					<div class="text-sm font-medium">ครูสอนในคาบนี้</div>
					{#if popoverInCell.length === 0}
						<p class="text-xs text-muted-foreground italic">ไม่มีครูในคาบนี้</p>
					{:else}
						<div class="flex flex-wrap gap-1.5">
							{#each popoverInCell as uid, idx}
								{@const isSelf = viewMode === 'INSTRUCTOR' && uid === selectedInstructorId}
								<Badge variant={isSelf ? 'default' : 'secondary'} class="gap-1 pr-1">
									<span>
										{isSelf ? '👤 ' : ''}{popoverInCellNames[idx] ?? uid}
										{#if isSelf}<span class="text-[10px] opacity-80">(คุณ)</span>{/if}
									</span>
									<button
										type="button"
										class="ml-1 rounded hover:bg-destructive/20 p-0.5"
										onclick={() => handlePopoverRemoveInstructor(uid)}
										disabled={entryPopoverSaving === uid}
										aria-label="ลบครู"
									>
										{#if entryPopoverSaving === uid}
											<Loader2 class="h-3 w-3 animate-spin" />
										{:else}
											<Trash2 class="h-3 w-3" />
										{/if}
									</button>
								</Badge>
							{/each}
						</div>
					{/if}
				</div>

				<div class="space-y-2 border-t pt-3">
					<div class="text-sm font-medium">เพิ่มครูจากทีมวิชา</div>
					{#if popoverNotInCell.length === 0}
						<p class="text-xs text-muted-foreground italic">ครูในทีมอยู่ในคาบนี้ครบแล้ว</p>
					{:else}
						<div class="flex flex-wrap gap-1.5">
							{#each popoverNotInCell as t}
								<Button
									variant="outline"
									size="sm"
									class="h-7 text-xs"
									onclick={() => handlePopoverAddInstructor(t.instructor_id, t.role)}
									disabled={entryPopoverSaving === t.instructor_id}
								>
									{#if entryPopoverSaving === t.instructor_id}
										<Loader2 class="h-3 w-3 animate-spin mr-1" />
									{:else}
										+
									{/if}
									{t.role === 'primary' ? '⭐ ' : ''}{t.instructor_name ?? t.instructor_id}
								</Button>
							{/each}
						</div>
					{/if}
				</div>
			{/if}
		</div>

		<Dialog.Footer>
			<Button variant="outline" onclick={() => (entryPopoverOpen = false)}>ปิด</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

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

<!-- Delete Activity Dialog (synchronized: single vs batch) -->
<Dialog.Root bind:open={showDeleteActivityDialog}>
	<Dialog.Content class="max-w-sm">
		<Dialog.Header>
			<Dialog.Title>ลบกิจกรรมจากตาราง</Dialog.Title>
			<Dialog.Description>
				{deleteActivityTarget?.activity_slot_name || deleteActivityTarget?.title || 'กิจกรรม'}
				{#if deleteActivityTarget?.classroom_name}
					— {deleteActivityTarget.classroom_name}
				{/if}
			</Dialog.Description>
		</Dialog.Header>
		<div class="flex flex-col gap-2 py-2">
			<Button variant="outline" onclick={() => { if (deleteActivityTarget) doDeleteEntry(deleteActivityTarget.id, false); }}>
				ลบเฉพาะห้องนี้
			</Button>
			<Button variant="destructive" onclick={() => { if (deleteActivityTarget) doDeleteEntry(deleteActivityTarget.id, true); }}>
				ลบทุกห้อง
			</Button>
		</div>
		<Dialog.Footer>
			<Button variant="ghost" onclick={() => { showDeleteActivityDialog = false; }}>ยกเลิก</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<!-- Export Modal -->
<Dialog.Root bind:open={showExportModal}>
	<Dialog.Content class="sm:max-w-[500px]">
		<Dialog.Header>
			<Dialog.Title>Download PDF</Dialog.Title>
			<Dialog.Description>เลือกข้อมูลที่ต้องการดาวน์โหลดตารางเรียน</Dialog.Description>
		</Dialog.Header>

		<div class="grid gap-4 py-4">
			<div class="flex flex-col gap-2">
				<Label.Root>ประเภท</Label.Root>
				<div class="flex gap-2">
					<Button
						variant={exportType === 'CLASSROOM' ? 'default' : 'outline'}
						size="sm"
						onclick={() => {
							exportType = 'CLASSROOM';
							exportTargetIds = [];
						}}
						class="flex-1"
					>
						<Users class="w-4 h-4 mr-2" /> ห้องเรียน
					</Button>
					<Button
						variant={exportType === 'INSTRUCTOR' ? 'default' : 'outline'}
						size="sm"
						onclick={() => {
							exportType = 'INSTRUCTOR';
							exportTargetIds = [];
						}}
						class="flex-1"
					>
						<School class="w-4 h-4 mr-2" /> ครูผู้สอน
					</Button>
				</div>
			</div>

			<div class="flex flex-col gap-2 max-h-[300px] overflow-y-auto border rounded p-2">
				<div class="flex justify-between items-center mb-2 px-1">
					<Label.Root>เลือกรายการ ({exportTargetIds.length})</Label.Root>
					<Button
						variant="ghost"
						size="sm"
						class="h-6 text-xs"
						onclick={() => {
							if (exportType === 'CLASSROOM') {
								if (exportTargetIds.length === classrooms.length) exportTargetIds = [];
								else exportTargetIds = classrooms.map((c) => c.id);
							} else {
								if (exportTargetIds.length === instructors.length) exportTargetIds = [];
								else exportTargetIds = instructors.map((i) => i.id);
							}
						}}
					>
						{exportTargetIds.length > 0 ? 'ล้างการเลือก' : 'เลือกทั้งหมด'}
					</Button>
				</div>

				{#if exportType === 'CLASSROOM'}
					{#each classrooms as room}
						<div class="flex items-center space-x-2 p-1 hover:bg-muted rounded">
							<Checkbox
								id="export-room-{room.id}"
								checked={exportTargetIds.includes(room.id)}
								onCheckedChange={(checked) => {
									if (checked) exportTargetIds = [...exportTargetIds, room.id];
									else exportTargetIds = exportTargetIds.filter((id) => id !== room.id);
								}}
							/>
							<Label.Root for="export-room-{room.id}" class="flex-1 cursor-pointer">
								{room.name}
							</Label.Root>
						</div>
					{/each}
				{:else}
					{#each instructors as teacher}
						<div class="flex items-center space-x-2 p-1 hover:bg-muted rounded">
							<Checkbox
								id="export-teacher-{teacher.id}"
								checked={exportTargetIds.includes(teacher.id)}
								onCheckedChange={(checked) => {
									if (checked) exportTargetIds = [...exportTargetIds, teacher.id];
									else exportTargetIds = exportTargetIds.filter((id) => id !== teacher.id);
								}}
							/>
							<Label.Root for="export-teacher-{teacher.id}" class="flex-1 cursor-pointer">
								{teacher.name}
							</Label.Root>
						</div>
					{/each}
				{/if}
			</div>
		</div>

		<Dialog.Footer>
			<Button variant="outline" onclick={() => (showExportModal = false)}>ยกเลิก</Button>
			<Button onclick={confirmExport} disabled={isExporting || exportTargetIds.length === 0}>
				{#if isExporting}
					<Loader2 class="mr-2 h-4 w-4 animate-spin" />
				{/if}
				ดาวน์โหลด ({exportTargetIds.length})
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
					<Button
						variant={batchMode === 'SLOT' ? 'default' : 'outline'}
						size="sm"
						onclick={() => {
							batchMode = 'SLOT';
							ensureActivitySlotsLoaded();
						}}
					>
						จากกิจกรรมพัฒนาผู้เรียน
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
								{#if batchType === 'ACTIVITY'}
									กิจกรรม
								{:else if batchType === 'BREAK'}
									พักเบรค/พักเที่ยง
								{:else if batchType === 'HOMEROOM'}
									โฮมรูม
								{:else if batchType === 'ACADEMIC'}
									วิชาการ
								{:else}
									{batchType}
								{/if}
							</Select.Trigger>
							<Select.Content>
								<Select.Item value="ACTIVITY">กิจกรรม</Select.Item>
								<Select.Item value="BREAK">พักเบรค/พักเที่ยง</Select.Item>
								<Select.Item value="HOMEROOM">โฮมรูม</Select.Item>
								<Select.Item value="ACADEMIC">วิชาการ</Select.Item>
							</Select.Content>
						</Select.Root>
					</div>
				</div>
			{:else if batchMode === 'SLOT'}
				<div class="grid grid-cols-4 items-center gap-4">
					<Label.Root class="text-right">กิจกรรม</Label.Root>
					<div class="col-span-3">
						{#if loadingSlots}
							<div class="text-sm text-muted-foreground flex items-center gap-2">
								<Loader2 class="w-3 h-3 animate-spin" /> กำลังโหลด...
							</div>
						{:else if activitySlots.length === 0}
							<p class="text-sm text-muted-foreground">ไม่พบ Activity Slot ในภาคเรียนนี้</p>
						{:else}
							<Select.Root type="single" bind:value={batchSlotId}>
								<Select.Trigger class="w-full h-auto py-2">
									<div class="flex flex-col items-start gap-0.5 text-left overflow-hidden">
										<span class="truncate block w-full">
											{activitySlots.find((s) => s.id === batchSlotId)?.name || 'เลือกกิจกรรม'}
										</span>
										{#if batchSlotId}
											{@const slot = activitySlots.find((s) => s.id === batchSlotId)}
											{#if slot}
												<span class="text-xs text-muted-foreground">
													{ACTIVITY_TYPE_LABELS[slot.activity_type] || slot.activity_type}
												</span>
											{/if}
										{/if}
									</div>
								</Select.Trigger>
								<Select.Content class="max-h-[300px] w-[350px] overflow-y-auto">
									{#each activitySlots as slot}
										<Select.Item
											value={slot.id}
											label={slot.name}
											class="flex flex-col items-start py-2 border-b last:border-0"
										>
											<span class="font-medium text-sm">{slot.name}</span>
											<span class="text-xs text-muted-foreground">
												{ACTIVITY_TYPE_LABELS[slot.activity_type] || slot.activity_type}
											</span>
										</Select.Item>
									{/each}
								</Select.Content>
							</Select.Root>
						{/if}
						<p class="text-[10px] text-muted-foreground mt-1.5 leading-relaxed">
							*เลือก Activity Slot จากระบบกิจกรรมพัฒนาผู้เรียน<br />
							นักเรียนจะกดดูกิจกรรมที่ตัวเองลงทะเบียนได้จากตารางเรียน
						</p>
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
				<Label.Root class="text-right">วัน</Label.Root>
				<div class="col-span-3">
					<Select.Root type="single" bind:value={batchDay}>
						<Select.Trigger class="w-full">
							{DAYS.find((d) => d.value === batchDay)?.label}
						</Select.Trigger>
						<Select.Content>
							{#each DAYS as day}
								<Select.Item value={day.value}>{day.label}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>

				</div>
			</div>

			<div class="grid grid-cols-4 items-start gap-4">
				<Label.Root class="text-right mt-1">คาบ ({batchPeriodIds.length})</Label.Root>
				<div class="col-span-3 border rounded-md max-h-[160px] overflow-y-auto p-2 bg-muted/20 grid grid-cols-2 gap-1.5">
					{#each periods as period}
						<label
							class="flex items-center gap-2 p-1.5 rounded border bg-background cursor-pointer hover:bg-muted/50 text-sm {batchPeriodIds.includes(period.id) ? 'border-primary bg-primary/5' : ''}"
						>
							<input
								type="checkbox"
								checked={batchPeriodIds.includes(period.id)}
								onchange={() => {
									if (batchPeriodIds.includes(period.id)) {
										batchPeriodIds = batchPeriodIds.filter((id) => id !== period.id);
									} else {
										batchPeriodIds = [...batchPeriodIds, period.id];
									}
								}}
								class="rounded"
							/>
							<span class="truncate">
								{period.name} ({formatTime(period.start_time)}-{formatTime(period.end_time)})
							</span>
						</label>
					{/each}
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
				{#if batchMode === 'SLOT' && batchSlotId && activitySlots.find((s) => s.id === batchSlotId)?.classroom_ids?.length}
					<div
						class="flex items-center gap-2 mb-3 px-3 py-2 bg-emerald-50/50 rounded border border-emerald-100 text-xs text-emerald-700"
					>
						<span class="font-bold">Info:</span> แสดงเฉพาะห้องเรียนที่เข้าร่วม Activity Slot นี้
					</div>
				{:else if !(batchMode === 'COURSE' && batchSubjectId)}
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
