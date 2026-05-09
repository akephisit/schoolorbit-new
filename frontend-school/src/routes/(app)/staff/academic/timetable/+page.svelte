<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
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
		deleteBatchGroup,
		removeEntryInstructor,
		addEntryInstructor,
		restoreInstructorToSlot,
		hideInstructorFromSlot,
		hideInstructorFromSlotPeriod,
		swapTimetableEntries,
		getTimetableOccupancy,
		type OccupancyEntry,
		type MoveValidityCell
	} from '$lib/api/timetable';
	import {
		listClassrooms,
		listClassroomCourses,
		listCourseInstructors,
		batchListCourseInstructors,
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
		listSlotInstructors,
		type ClassroomCourse
	} from '$lib/api/academic';
	import {
		lookupRooms,
		type RoomLookupItem,
		lookupStaff,
		type StaffLookupItem,
		lookupGradeLevels,
		type GradeLevelLookupItem
	} from '$lib/api/lookup';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Label from '$lib/components/ui/label';

	import * as Card from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import * as Select from '$lib/components/ui/select';
	import * as Popover from '$lib/components/ui/popover';
	import * as Command from '$lib/components/ui/command';
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
		Lock,
		FileStack,
		ChevronsUpDown,
		Check
	} from 'lucide-svelte';
	import { generateTimetablePDF } from '$lib/utils/pdf';
	import { SvelteSet, SvelteMap } from 'svelte/reactivity';

	import { Checkbox } from '$lib/components/ui/checkbox';

	import { authStore } from '$lib/stores/auth';
	import {
		connectTimetableSocket,
		disconnectTimetableSocket,
		sendTimetableEvent,
		sendDropIntent,
		sendEntryIntent,
		activeUsers,
		remoteCursors,
		dragPositions,
		userDrags,
		refreshTrigger,
		isConnected,
		lastPatch,
		setInitialSeq,
		sendUserActivity,
		sendUserActivityEnd,
		remoteActivities,
		type TimetablePatch
	} from '$lib/stores/timetable-socket';
	import type {
		ConflictInfo,
		CreateTimetableEntryRequest,
		UpdateTimetableEntryRequest
	} from '$lib/api/timetable';

	/** Extended course type used for drag-and-drop sidebar items (activity slots, courses, and entries).
	 *  This is a structural "superset" type — includes all properties that any draggable item may have.
	 */
	interface DragCourse {
		id: string;
		_isActivity?: boolean;
		_classroom_id?: string;
		activity_slot_id?: string;
		title_th?: string;
		title?: string;
		title_en?: string;
		classroom_id?: string;
		primary_instructor_id?: string;
		subject_code?: string;
		subject_name_th?: string;
		classroom_course_id?: string;
		room_id?: string;
		instructor_name?: string;
		classroom_name?: string;
		color?: string;
	}

	let { data } = $props();

	// Helper: Generate consistent pastel color from string
	// แสดง label fallback เมื่อ entry ไม่มี subject_code/title
	function getEntryTypeFallbackLabel(entryType?: string): string {
		switch (entryType) {
			case 'ACTIVITY':
				return 'กิจกรรม';
			case 'ACADEMIC':
				return 'วิชาการ';
			case 'BREAK':
				return 'พัก';
			case 'HOMEROOM':
				return 'โฮมรูม';
			default:
				return '';
		}
	}

	function getSubjectColor(code: string, type?: string): string {
		if (type === 'BREAK') return '#fef3c7'; // amber-100
		if (type === 'ACTIVITY' || type === 'HOMEROOM') return '#d1fae5'; // emerald-100
		if (type === 'ACADEMIC') return '#dbeafe'; // blue-100

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
		if (type === 'ACADEMIC') return '#93c5fd'; // blue-300

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
	let timetableEntries = $state<TimetableEntry[]>([]);
	let periods = $state<AcademicPeriod[]>([]);
	let classrooms = $state<Classroom[]>([]);
	let courses = $state<ClassroomCourse[]>([]);
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
	let showDeleteBatchDialog = $state(false);
	let deleteBatchTarget = $state<TimetableEntry | null>(null);

	// View Mode: 'CLASSROOM' or 'INSTRUCTOR'
	let viewMode = $state<'CLASSROOM' | 'INSTRUCTOR'>('CLASSROOM');

	// Per-cell instructor editor (Popover dialog)
	let entryPopoverOpen = $state(false);
	let entryPopoverTarget = $state<TimetableEntry | null>(null);
	let entryPopoverTeam = $state<CourseInstructor[]>([]);
	let entryPopoverLoading = $state(false);
	let entryPopoverSaving = $state('');
	let entryPopoverSavingRoom = $state(false);
	let entryPopoverRoomPickerOpen = $state(false);
	let entryPopoverUnavailableRooms = $state<Set<string>>(new Set());

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

	// ghost = ครูที่เลือกอยู่ในทีมวิชา แต่ไม่ได้สอนคาบนี้ → entry ไม่ใช่ของเขา → ซ่อน "เปลี่ยนห้อง"
	// (กันครูแก้ห้องของ entry คนอื่นโดยไม่ตั้งใจ)
	let entryPopoverIsGhost = $derived(
		viewMode === 'INSTRUCTOR' &&
			selectedInstructorId !== '' &&
			entryPopoverTarget !== null &&
			!(entryPopoverTarget.instructor_ids ?? []).includes(selectedInstructorId)
	);

	// Searchable picker state
	let classroomPickerOpen = $state(false);
	let instructorPickerOpen = $state(false);
	let roomPickerOpen = $state(false);

	// Derived
	let DAYS = $derived(
		getSchoolDays(academicYears.find((y) => y.id === selectedYearId)?.school_days)
	);
	let semesters = $derived(allSemesters.filter((s) => s.academic_year_id === selectedYearId));

	// Drag & Drop state
	let draggedCourse = $state<DragCourse | null>(null);
	let submitting = $state(false);
	let isDropPending = $state(false);
	// Identify what is being dragged: 'NEW' (from list) | 'MOVE' (from grid)
	let dragType = $state<'NEW' | 'MOVE'>('NEW');

	let draggedEntryId = $state<string | null>(null);

	async function loadInitialData() {
		try {
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
			// Phase 2 Fix 3: batch-fetch teams ทุก course → cache สำหรับ buildTempEntry
			void loadCourseTeams(courses.map((c) => c.id));
		} catch (e) {
			console.error(e);
			toast.error('โหลดรายวิชาไม่สำเร็จ');
		}
	}

	async function loadCourseTeams(courseIds: string[]) {
		if (courseIds.length === 0) {
			courseTeamsMap = new Map();
			return;
		}
		try {
			const res = await batchListCourseInstructors(courseIds);
			const map = new Map<string, CourseInstructor[]>();
			for (const [cid, team] of Object.entries(res.data ?? {})) {
				map.set(cid, team);
			}
			courseTeamsMap = map;
		} catch {
			// ถ้า fetch ไม่สำเร็จ → buildTempEntry fallback กลับไปใช้ primary instructor (เหมือนเดิม)
		}
	}

	async function checkClassroomHasInstructor(
		slotId: string,
		classroomIdOverride?: string
	): Promise<boolean> {
		try {
			const res = await listSlotClassroomAssignments(slotId);
			const clsId = classroomIdOverride || selectedClassroomId;
			return (res.data ?? []).some((a) => a.classroom_id === clsId);
		} catch {
			return false;
		}
	}

	// For INSTRUCTOR view: independent slots with per-classroom items
	let instructorActivityItems = $state<
		Array<{
			slot: ActivitySlot;
			classroom_id: string;
			classroom_name: string;
		}>
	>([]);

	// INSTRUCTOR view + sync activity → ปุ่มใน batch delete dialog ทำ "ซ่อน" แทน "ลบจริง"
	let isInstructorSyncDelete = $derived(
		viewMode === 'INSTRUCTOR' &&
			!!deleteBatchTarget?.activity_slot_id &&
			(sidebarActivitySlots.find((s) => s.id === deleteBatchTarget?.activity_slot_id)
				?.scheduling_mode === 'synchronized' ||
				instructorActivityItems.find((i) => i.slot.id === deleteBatchTarget?.activity_slot_id)
					?.slot?.scheduling_mode === 'synchronized')
	);

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
					} catch {
						/* ignore per-slot errors */
					}
				}

				// Independent: slots where instructor is assigned to classrooms
				const indepSlots = allSlots.filter((s) => s.scheduling_mode === 'independent');
				const items: typeof instructorActivityItems = [];
				for (const slot of indepSlots) {
					try {
						const assignRes = await listSlotClassroomAssignments(slot.id);
						for (const a of assignRes.data ?? []) {
							if (a.instructor_id === selectedInstructorId) {
								items.push({
									slot,
									classroom_id: a.classroom_id,
									classroom_name: a.classroom_name ?? ''
								});
							}
						}
					} catch {
						/* ignore per-slot errors */
					}
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
			const res = await listActivityGroups({
				instructor_id: selectedInstructorId,
				semester_id: selectedSemesterId
			});
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
			if (typeof res.current_seq === 'number') {
				setInitialSeq(res.current_seq);
			}
		} catch {
			toast.error('โหลดตารางสอนไม่สำเร็จ');
		}
	}

	// ===== Occupancy map: client-side validation (Phase 1) =====
	async function loadOccupancy() {
		if (!selectedSemesterId) {
			occupancyEntries = [];
			return;
		}
		try {
			const res = await getTimetableOccupancy(selectedSemesterId);
			occupancyEntries = res.data ?? [];
		} catch {
			occupancyEntries = [];
		}
	}

	function entryToOccupancy(e: TimetableEntry): OccupancyEntry {
		return {
			id: e.id,
			classroom_id: e.classroom_id ?? null,
			day_of_week: e.day_of_week,
			period_id: e.period_id,
			room_id: e.room_id ?? null,
			instructor_ids: e.instructor_ids ?? []
		};
	}

	function upsertOccupancy(e: TimetableEntry) {
		const o = entryToOccupancy(e);
		const idx = occupancyEntries.findIndex((x) => x.id === o.id);
		if (idx >= 0) {
			occupancyEntries[idx] = o;
			occupancyEntries = [...occupancyEntries];
		} else {
			occupancyEntries = [...occupancyEntries, o];
		}
	}

	function removeOccupancy(entryId: string) {
		occupancyEntries = occupancyEntries.filter((e) => e.id !== entryId);
	}

	/** Build optimistic TimetableEntry จาก local state lookups — สำหรับ NEW drop (CREATE).
	 *  Joined fields ที่ frontend คำนวณเองได้: subject_code/name, classroom_name, room_code,
	 *  period_name, start/end_time, instructor_id/name (primary). 100ms ภายหลัง backend จะ broadcast
	 *  EntryCreated มาทับด้วยข้อมูลครบ (รวมทีมครูทุกคน). */
	function buildTempEntry(args: {
		tempId: string;
		classroomId: string;
		classroomCourseId?: string;
		activitySlotId?: string;
		day: TimetableEntry['day_of_week'];
		periodId: string;
		roomId?: string;
		title?: string;
		entryType: 'COURSE' | 'ACTIVITY';
	}): TimetableEntry {
		const room = args.roomId ? rooms.find((r) => r.id === args.roomId) : null;
		const classroom = classrooms.find((c) => c.id === args.classroomId);
		const period = periods.find((p) => p.id === args.periodId);
		const courseInfo = args.classroomCourseId
			? courses.find((c) => c.id === args.classroomCourseId)
			: null;
		const activitySlot = args.activitySlotId
			? sidebarActivitySlots.find((s) => s.id === args.activitySlotId)
			: null;
		// instructor lookup — Phase 2 Fix 3: ใช้ทีมเต็มจาก courseTeamsMap ถ้ามี, fallback primary
		let instructorIds: string[] = [];
		let instructorNames: string[] = [];
		const team = args.classroomCourseId ? courseTeamsMap.get(args.classroomCourseId) : undefined;
		if (team && team.length > 0) {
			instructorIds = team.map((m) => m.instructor_id);
			instructorNames = team.map((m) => m.instructor_name ?? '');
		} else if (courseInfo?.primary_instructor_id) {
			instructorIds = [courseInfo.primary_instructor_id];
			instructorNames = courseInfo.instructor_name ? [courseInfo.instructor_name] : [];
		}
		return {
			id: args.tempId,
			classroom_course_id: args.classroomCourseId,
			activity_slot_id: args.activitySlotId,
			classroom_id: args.classroomId,
			academic_semester_id: selectedSemesterId,
			day_of_week: args.day,
			period_id: args.periodId,
			room_id: args.roomId,
			entry_type: args.entryType,
			title: args.title ?? activitySlot?.name,
			is_active: true,
			subject_code: courseInfo?.subject_code,
			subject_name_th: courseInfo?.subject_name_th,
			subject_name_en: courseInfo?.subject_name_en,
			instructor_ids: instructorIds,
			instructor_names: instructorNames,
			classroom_name: classroom?.name,
			room_code: room?.code,
			period_name: period?.name ?? undefined,
			start_time: period?.start_time,
			end_time: period?.end_time,
			activity_scheduling_mode: activitySlot?.scheduling_mode
		};
	}

	/** Compute replacement content fields for REPLACE optimistic — เปลี่ยน entry's content
	 *  ไปเป็น course/activity ใหม่ โดย lookup joined fields จาก local state */
	function computeReplacementFields(args: {
		newCourseId?: string;
		newActivitySlotId?: string;
		newClassroomId?: string;
		newTitle?: string;
	}): Partial<TimetableEntry> {
		if (args.newActivitySlotId) {
			const slot = sidebarActivitySlots.find((s) => s.id === args.newActivitySlotId);
			return {
				classroom_course_id: undefined,
				activity_slot_id: args.newActivitySlotId,
				entry_type: 'ACTIVITY',
				subject_code: undefined,
				subject_name_th: undefined,
				subject_name_en: undefined,
				title: args.newTitle ?? slot?.name,
				activity_scheduling_mode: slot?.scheduling_mode,
				instructor_ids: [],
				instructor_names: []
			};
		} else if (args.newCourseId) {
			const course = courses.find((c) => c.id === args.newCourseId);
			const team = courseTeamsMap.get(args.newCourseId);
			let instructorIds: string[] = [];
			let instructorNames: string[] = [];
			if (team && team.length > 0) {
				instructorIds = team.map((m) => m.instructor_id);
				instructorNames = team.map((m) => m.instructor_name ?? '');
			} else if (course?.primary_instructor_id) {
				instructorIds = [course.primary_instructor_id];
				instructorNames = course.instructor_name ? [course.instructor_name] : [];
			}
			return {
				classroom_course_id: args.newCourseId,
				activity_slot_id: undefined,
				entry_type: 'COURSE',
				subject_code: course?.subject_code,
				subject_name_th: course?.subject_name_th,
				subject_name_en: course?.subject_name_en,
				title: undefined,
				activity_scheduling_mode: undefined,
				classroom_id: args.newClassroomId ?? course?.classroom_id,
				instructor_ids: instructorIds,
				instructor_names: instructorNames
			};
		}
		return {};
	}

	function markPending(entryId: string) {
		pendingEntryIds.add(entryId);
		// Set/refresh timeout — auto rollback ถ้าครบ 15s ยังไม่ confirm
		const existing = pendingTimeouts.get(entryId);
		if (existing) clearTimeout(existing);
		const t = setTimeout(() => {
			if (pendingEntryIds.has(entryId)) {
				autoRollbackPending(entryId);
			}
		}, PENDING_TIMEOUT_MS);
		pendingTimeouts.set(entryId, t);
	}

	function clearPending(entryId: string) {
		pendingEntryIds.delete(entryId);
		optimisticSnapshots.delete(entryId);
		const t = pendingTimeouts.get(entryId);
		if (t) {
			clearTimeout(t);
			pendingTimeouts.delete(entryId);
		}
	}

	/** หมดเวลา pending — ถ้า tempEntry ลบทิ้ง, ถ้า real entry restore จาก snapshot */
	function autoRollbackPending(entryId: string) {
		const isTemp = entryId.startsWith('temp-');
		if (isTemp) {
			// CREATE temp entry ค้าง → ลบทิ้ง
			timetableEntries = timetableEntries.filter((e) => e.id !== entryId);
			rawTeamEntries = rawTeamEntries.filter((e) => e.id !== entryId);
			removeOccupancy(entryId);
		} else {
			// MOVE/SWAP/REPLACE ค้าง → restore จาก snapshot ถ้ามี
			const snap = optimisticSnapshots.get(entryId);
			if (snap) {
				applyEntryMutation(entryId, {
					day_of_week: snap.day_of_week as TimetableEntry['day_of_week'],
					period_id: snap.period_id,
					room_id: snap.room_id ?? undefined
				});
			}
		}
		clearPending(entryId);
		toast.error('การบันทึกไม่ตอบกลับ — ลองอีกครั้ง');
	}

	function snapshotPosition(entryId: string) {
		const e = timetableEntries.find((x) => x.id === entryId)
			?? rawTeamEntries.find((x) => x.id === entryId)
			?? occupancyEntries.find((x) => x.id === entryId);
		if (!e) return null;
		return {
			day_of_week: e.day_of_week,
			period_id: e.period_id,
			room_id: 'room_id' in e ? (e.room_id ?? null) : null
		};
	}

	/** Optimistic mutation: apply partial fields to entry locally across timetableEntries,
	 *  rawTeamEntries, and occupancyEntries. Returns snapshot used to rollback on failure. */
	function applyEntryMutation(
		entryId: string,
		fields: Partial<TimetableEntry>
	): Partial<TimetableEntry> | null {
		const current = timetableEntries.find((e) => e.id === entryId)
			?? rawTeamEntries.find((e) => e.id === entryId);
		if (!current) return null;

		// Build snapshot of fields about to change (so rollback is precise)
		const snapshot: Partial<TimetableEntry> = {};
		for (const k of Object.keys(fields) as (keyof TimetableEntry)[]) {
			(snapshot as Record<string, unknown>)[k] = current[k];
		}

		const merge = (e: TimetableEntry) => (e.id === entryId ? { ...e, ...fields } : e);
		timetableEntries = timetableEntries.map(merge);
		rawTeamEntries = rawTeamEntries.map(merge);

		// Mirror occupancy fields (only the keys that affect conflict checks)
		const occMutation: Partial<OccupancyEntry> = {};
		if ('day_of_week' in fields) occMutation.day_of_week = fields.day_of_week as string;
		if ('period_id' in fields) occMutation.period_id = fields.period_id as string;
		if ('room_id' in fields) occMutation.room_id = (fields.room_id ?? null) as string | null;
		if ('classroom_id' in fields)
			occMutation.classroom_id = (fields.classroom_id ?? null) as string | null;
		if (Object.keys(occMutation).length > 0) {
			occupancyEntries = occupancyEntries.map((e) =>
				e.id === entryId ? { ...e, ...occMutation } : e
			);
		}
		return snapshot;
	}

	/** Compute MoveValidityCell map locally — replaces POST /validate-moves
	 *  Logic mirrors backend handler: pairwise swap validation vs empty-cell direct fit. */
	function computeValidMoves(entryId: string): Map<string, MoveValidityCell> {
		const result = new Map<string, MoveValidityCell>();
		const source = occupancyEntries.find((e) => e.id === entryId);
		if (!source) return result;

		// Index entries by cell key once → O(1) lookup per cell
		const cellIndex = new Map<string, OccupancyEntry[]>();
		for (const e of occupancyEntries) {
			const k = `${e.day_of_week}|${e.period_id}`;
			const arr = cellIndex.get(k);
			if (arr) arr.push(e);
			else cellIndex.set(k, [e]);
		}

		const srcInstructorSet = new Set(source.instructor_ids);
		const srcCellKey = `${source.day_of_week}|${source.period_id}`;

		for (const day of DAYS) {
			for (const period of periods) {
				const key = `${day.value}|${period.id}`;

				if (key === srcCellKey) {
					result.set(key, {
						day_of_week: day.value,
						period_id: period.id,
						state: 'source',
						target_entry_id: null,
						valid: false,
						reason: ''
					});
					continue;
				}

				const occupants = cellIndex.get(key) ?? [];
				const others = occupants.filter((e) => e.id !== entryId);

				if (others.length === 0) {
					// Empty — source moves freely (no entry at this cell to conflict with)
					result.set(key, {
						day_of_week: day.value,
						period_id: period.id,
						state: 'empty',
						target_entry_id: null,
						valid: true,
						reason: ''
					});
					continue;
				}

				// Occupied — try swap with first other entry (matches backend behavior)
				const target = others[0];
				const targetInstructorSet = new Set(target.instructor_ids);

				// Other entries at source's pos (excluding source itself, excluding target if it's there)
				const srcPosOthers = (cellIndex.get(srcCellKey) ?? []).filter(
					(e) => e.id !== entryId && e.id !== target.id
				);
				// Other entries at target's pos (excluding source if it's there, excluding target)
				const tgtPosOthers = others.filter((e) => e.id !== target.id);

				let valid = true;
				let reason = '';

				// Classroom: source's classroom mustn't have a 3rd entry at target's pos
				if (source.classroom_id && tgtPosOthers.some((e) => e.classroom_id === source.classroom_id)) {
					valid = false;
					reason = 'ห้องของต้นทางถูกใช้ที่คาบนี้';
				}
				// Classroom: target's classroom mustn't have a 3rd entry at source's pos
				if (
					valid &&
					target.classroom_id &&
					srcPosOthers.some((e) => e.classroom_id === target.classroom_id)
				) {
					valid = false;
					reason = 'ห้องของปลายทางถูกใช้ที่คาบต้นทาง';
				}

				// Instructor: source's instructors mustn't conflict at target's pos
				if (valid) {
					for (const e of tgtPosOthers) {
						if (e.instructor_ids.some((iid) => srcInstructorSet.has(iid))) {
							valid = false;
							reason = 'ครูต้นทางติดคาบปลายทาง';
							break;
						}
					}
				}
				// Instructor: target's instructors mustn't conflict at source's pos
				if (valid) {
					for (const e of srcPosOthers) {
						if (e.instructor_ids.some((iid) => targetInstructorSet.has(iid))) {
							valid = false;
							reason = 'ครูปลายทางติดคาบต้นทาง';
							break;
						}
					}
				}

				// Room: source's room mustn't conflict at target's pos
				if (valid && source.room_id && tgtPosOthers.some((e) => e.room_id === source.room_id)) {
					valid = false;
					reason = 'ห้องต้นทางถูกใช้ที่คาบปลายทาง';
				}
				// Room: target's room mustn't conflict at source's pos
				if (valid && target.room_id && srcPosOthers.some((e) => e.room_id === target.room_id)) {
					valid = false;
					reason = 'ห้องปลายทางถูกใช้ที่คาบต้นทาง';
				}

				result.set(key, {
					day_of_week: day.value,
					period_id: period.id,
					state: 'occupied',
					target_entry_id: target.id,
					valid,
					reason
				});
			}
		}

		return result;
	}

	// ===== Per-cell instructor popover =====
	async function openEntryPopover(entry: TimetableEntry) {
		// Only support COURSE entries (activity entries have different instructor logic)
		if (entry.entry_type !== 'COURSE' || !entry.classroom_course_id) return;
		// Lock: block ถ้ามี user อื่นเปิด dialog ที่ entry นี้อยู่
		const locked = getRemoteActivityForEntry(entry.id);
		if (locked) {
			toast.error(
				`${locked.user.name} กำลัง${activityLabel(locked.activity)}ที่คาบนี้ — ลองอีกครั้ง`
			);
			return;
		}
		entryPopoverTarget = entry;
		entryPopoverTeam = [];
		entryPopoverUnavailableRooms = new Set();
		entryPopoverOpen = true;
		entryPopoverLoading = true;
		try {
			const tasks: Promise<unknown>[] = [listCourseInstructors(entry.classroom_course_id)];
			// โหลด busy rooms เฉพาะกรณีที่จะแสดง section "เปลี่ยนห้อง" (INSTRUCTOR view + ไม่ใช่ ghost)
			const isGhostForThisEntry =
				viewMode === 'INSTRUCTOR' &&
				selectedInstructorId !== '' &&
				!(entry.instructor_ids ?? []).includes(selectedInstructorId);
			if (viewMode === 'INSTRUCTOR' && !isGhostForThisEntry) {
				tasks.push(loadUnavailableRoomsForEntry(entry));
			}
			const [teamRes] = (await Promise.all(tasks)) as [
				Awaited<ReturnType<typeof listCourseInstructors>>,
				...unknown[]
			];
			entryPopoverTeam = teamRes.data ?? [];
		} catch {
			toast.error('โหลดรายชื่อครูไม่สำเร็จ');
		} finally {
			entryPopoverLoading = false;
		}
	}

	async function loadUnavailableRoomsForEntry(entry: TimetableEntry) {
		try {
			const res = await listTimetableEntries({
				day_of_week: entry.day_of_week,
				academic_semester_id: selectedSemesterId
			});
			const busy = new Set<string>();
			res.data.forEach((e) => {
				if (e.period_id === entry.period_id && e.room_id && e.id !== entry.id) {
					busy.add(e.room_id);
				}
			});
			entryPopoverUnavailableRooms = busy;
		} catch {
			entryPopoverUnavailableRooms = new Set();
		}
	}

	async function handlePopoverChangeRoom(roomId: string) {
		if (!entryPopoverTarget) return;
		const entry = entryPopoverTarget;
		if (entry.room_id === roomId) {
			entryPopoverRoomPickerOpen = false;
			return;
		}
		entryPopoverSavingRoom = true;
		try {
			const res = await updateTimetableEntry(entry.id, { room_id: roomId });
			if (res?.success === false) {
				toast.error(res.message || 'เปลี่ยนห้องไม่สำเร็จ');
				return;
			}
			entry.room_id = roomId;
			rawTeamEntries = [...rawTeamEntries];
			timetableEntries = [...timetableEntries];
			toast.success('เปลี่ยนห้องแล้ว');
		} catch (e: unknown) {
			toast.error(e instanceof Error ? e.message : 'เปลี่ยนห้องไม่สำเร็จ');
		} finally {
			entryPopoverSavingRoom = false;
			entryPopoverRoomPickerOpen = false;
		}
	}

	async function handlePopoverRemoveInstructor(userId: string) {
		if (!entryPopoverTarget) return;
		const entry = entryPopoverTarget;
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

	// Dialog presence — broadcast activity ให้ user อื่นเห็น
	$effect(() => {
		if (entryPopoverOpen && entryPopoverTarget) {
			sendUserActivity('instructor_edit', { entry_id: entryPopoverTarget.id });
		} else if (showRoomModal && pendingDropContext) {
			sendUserActivity('room_picker', {
				day: pendingDropContext.day,
				period_id: pendingDropContext.periodId,
				classroom_id: selectedClassroomId || null
			});
		} else {
			sendUserActivityEnd();
		}
	});

	/** หา remote activity ที่ target = slot (day, periodId, classroom) ของ view ปัจจุบัน */
	function getRemoteActivityForSlot(day: string, periodId: string) {
		for (const [userId, act] of Object.entries($remoteActivities)) {
			const t = (act.target ?? {}) as { day?: string; period_id?: string; classroom_id?: string };
			if (act.activity_type !== 'room_picker') continue;
			if (t.day !== day || t.period_id !== periodId) continue;
			// filter classroom: ถ้า activity ระบุ classroom → ต้องตรงกับ classroom view
			// (ไม่งั้น user ใน ห้อง 1/2 เห็น lock ของ 1/1)
			if (t.classroom_id && viewMode === 'CLASSROOM' && t.classroom_id !== selectedClassroomId)
				continue;
			const user = $activeUsers.find((u) => u.user_id === userId);
			if (!user) continue;
			return { user, activity: act };
		}
		return null;
	}

	/** หา remote activity ที่ target = entry_id (instructor_edit)
	 *  entry_id เป็น global unique — ไม่ต้อง filter context (ถ้าคุณเห็น entry นี้ = ล็อค)
	 */
	function getRemoteActivityForEntry(entryId: string) {
		for (const [userId, act] of Object.entries($remoteActivities)) {
			const t = (act.target ?? {}) as { entry_id?: string };
			const matches = act.activity_type === 'instructor_edit' && t.entry_id === entryId;
			if (!matches) continue;
			const user = $activeUsers.find((u) => u.user_id === userId);
			if (!user) continue;
			return { user, activity: act };
		}
		return null;
	}

	function activityLabel(act: { activity_type: string }): string {
		switch (act.activity_type) {
			case 'room_picker':
				return 'กำลังเลือกห้อง';
			case 'instructor_edit':
				return 'กำลังแก้ครู';
			case 'replace_confirm':
				return 'ยืนยันแทน';
			default:
				return 'กำลังแก้ไข';
		}
	}

	async function handleDeleteEntry(entry: TimetableEntry) {
		// Guard: ถ้ามี user อื่นเปิด dialog ที่ entry นี้ → block
		const remoteLock = getRemoteActivityForEntry(entry.id);
		if (remoteLock) {
			toast.error(
				`${remoteLock.user.name} กำลัง${activityLabel(remoteLock.activity)}ที่คาบนี้ — ลบไม่ได้`
			);
			return;
		}

		// batch_id ทุกชนิด (TEXT/SLOT-sync) เปิด dialog ถามก่อน
		// — doDeleteBatch* จัดการ routing เอง: INSTRUCTOR + sync ใช้ hide endpoint,
		//   ส่วน CLASSROOM หรือ TEXT batch ลบ entry/group จริง
		if (entry.batch_id) {
			deleteBatchTarget = entry;
			showDeleteBatchDialog = true;
			return;
		}

		// INSTRUCTOR view: non-batch entries (independent activity, course)
		if (viewMode === 'INSTRUCTOR') {
			if (entry.activity_slot_id) {
				const slot =
					sidebarActivitySlots.find((s) => s.id === entry.activity_slot_id) ||
					instructorActivityItems.find((i) => i.slot.id === entry.activity_slot_id)?.slot;
				if (slot?.scheduling_mode === 'synchronized') {
					// Legacy sync ที่ไม่มี batch_id — ซ่อนทั้ง slot
					if (!selectedInstructorId) return;
					try {
						await hideInstructorFromSlot(entry.activity_slot_id, selectedInstructorId);
						toast.success('ลบครูออกจากกิจกรรมนี้แล้ว (ทุกห้อง)');
					} catch (err: unknown) {
						toast.error((err instanceof Error ? err.message : String(err)) || 'ลบไม่สำเร็จ');
						return;
					}
				} else {
					// Independent: one entry = one classroom; delete entry
					try {
						await deleteTimetableEntry(entry.id);
						toast.success('ลบกิจกรรมออกจากตารางสำเร็จ');
					} catch (e: unknown) {
						toast.error((e instanceof Error ? e.message : String(e)) || 'ลบไม่สำเร็จ');
						return;
					}
				}
			} else {
				// Regular course: ลบ entry ทั้งอัน (กระทบครูทุกคน) — ถ้าจะลบแค่ตัวเอง ใช้ × ใน popover
				try {
					await deleteTimetableEntry(entry.id);
					toast.success('ลบคาบแล้ว');
				} catch (e: unknown) {
					toast.error((e instanceof Error ? e.message : String(e)) || 'ลบไม่สำเร็จ');
					return;
				}
			}
			// Backend broadcasts patch event ให้แล้ว — ไม่ต้องส่ง TableRefresh ซ้ำ
			loadTimetable();
			loadSidebarActivitySlots();
			return;
		}

		if (entry.activity_slot_id) {
			const slot =
				sidebarActivitySlots.find((s) => s.id === entry.activity_slot_id) ||
				instructorActivityItems.find((i) => i.slot.id === entry.activity_slot_id)?.slot;
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

	async function doDeleteBatchGroup() {
		const target = deleteBatchTarget;
		if (!target?.batch_id) return;
		try {
			// INSTRUCTOR view + sync activity → ซ่อนครูคนนี้จากทั้ง slot (ไม่ลบ entry จริง)
			if (
				viewMode === 'INSTRUCTOR' &&
				target.activity_slot_id &&
				selectedInstructorId
			) {
				const slot =
					sidebarActivitySlots.find((s) => s.id === target.activity_slot_id) ||
					instructorActivityItems.find((i) => i.slot.id === target.activity_slot_id)?.slot;
				if (slot?.scheduling_mode === 'synchronized') {
					await hideInstructorFromSlot(target.activity_slot_id, selectedInstructorId);
					toast.success('ลบครูออกจากกิจกรรมนี้แล้ว (ทุกคาบ ทุกห้อง)');
					showDeleteBatchDialog = false;
					deleteBatchTarget = null;
					loadTimetable();
					loadSidebarActivitySlots();
					return;
				}
			}
			// CLASSROOM view (หรือ INSTRUCTOR + non-sync): ลบ batch group จริง
			const res = await deleteBatchGroup(target.batch_id);
			toast.success(`ลบทั้งกลุ่มสำเร็จ (${res.deleted_count} คาบ)`);
			showDeleteBatchDialog = false;
			deleteBatchTarget = null;
			loadTimetable();
		} catch (e: unknown) {
			toast.error((e instanceof Error ? e.message : String(e)) || 'ลบไม่สำเร็จ');
		}
	}

	async function doDeleteBatchSingle() {
		if (!deleteBatchTarget) return;
		const target = deleteBatchTarget;
		try {
			// INSTRUCTOR view + sync activity → ซ่อนครูคนนี้จาก slot+day+period (ทุกห้อง)
			// ไม่ใช่ลบ entry เพราะจะลบแค่ห้องเดียว — INSTRUCTOR view ทำ dedupe → cell ยังโผล่
			if (
				viewMode === 'INSTRUCTOR' &&
				target.activity_slot_id &&
				selectedInstructorId
			) {
				const slot =
					sidebarActivitySlots.find((s) => s.id === target.activity_slot_id) ||
					instructorActivityItems.find((i) => i.slot.id === target.activity_slot_id)?.slot;
				if (slot?.scheduling_mode === 'synchronized') {
					await hideInstructorFromSlotPeriod(
						target.activity_slot_id,
						selectedInstructorId,
						target.day_of_week,
						target.period_id
					);
					toast.success('ลบครูออกจากคาบนี้แล้ว (ทุกห้อง)');
					showDeleteBatchDialog = false;
					deleteBatchTarget = null;
					loadTimetable();
					loadSidebarActivitySlots();
					return;
				}
			}
			// CLASSROOM view (หรือ INSTRUCTOR + non-sync): ลบ entry เดียว
			await deleteTimetableEntry(target.id);
			toast.success('ลบเฉพาะคาบนี้แล้ว');
			showDeleteBatchDialog = false;
			deleteBatchTarget = null;
			loadTimetable();
		} catch (e: unknown) {
			toast.error((e instanceof Error ? e.message : String(e)) || 'ลบไม่สำเร็จ');
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
		} catch (e: unknown) {
			toast.error((e instanceof Error ? e.message : String(e)) || 'ลบไม่สำเร็จ');
		}
	}

	function getEntryForSlot(day: string, periodId: string): TimetableEntry | undefined {
		return displayEntries.find((e) => e.day_of_week === day && e.period_id === periodId);
	}

	// In INSTRUCTOR view, deduplicate ACTIVITY entries that share the same slot+day+period
	let displayEntries = $derived.by(() => {
		if (viewMode !== 'INSTRUCTOR') return timetableEntries;
		const seen = new SvelteSet<string>();
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

	// Dialog ปิดโดยคลิกนอก/กด ESC → bind:open จะ set false เองแต่ cleanup ไม่ทำงาน
	// watch: ถ้า modal ปิดตอน drop ยัง pending → treat as cancel (ลบ highlights/conflicts)
	$effect(() => {
		if (!showRoomModal && isDropPending) {
			isDropPending = false;
			pendingDropContext = null;
			handleDragEnd();
		}
	});
	let pendingDropContext = $state<{
		day: string;
		periodId: string;
		dragType: 'NEW' | 'MOVE';
		course: DragCourse;
		entryId: string | null;
	} | null>(null);

	let selectedRoomId = $state<string>(''); // empty string = no room (default)

	// Availability State
	interface SlotConflict {
		kind: 'classroom' | 'teacher';
		teacher_name?: string;
		subject_code: string;
		subject_name: string;
		classroom_name: string;
		room_code?: string;
	}
	let slotConflicts = new SvelteMap<string, SlotConflict[]>();

	// Floating conflict popup ตอน drag hover (native title tooltip ใช้ไม่ได้ระหว่าง drag)
	let hoverDragCell = $state<{ day: string; periodId: string; x: number; y: number } | null>(null);

	// Drag validity map: key "DAY|PERIODID" → cell state (computed locally from occupancyEntries)
	// Populated on drag start for MOVE type; cleared on drag end.
	let moveValidityMap = $state<Map<string, MoveValidityCell>>(new Map());

	// Lightweight semester-wide entry summary for client-side conflict checks.
	// โหลดครั้งเดียวต่อ semester + sync ตาม WS events → drag validity คำนวณใน JS (0ms lag)
	let occupancyEntries = $state<OccupancyEntry[]>([]);

	// Phase 2 Fix 3: pre-fetched course teams (full instructor list per course)
	// → buildTempEntry render ครูครบทีมตอน CREATE optimistic (ไม่ใช่แค่ primary)
	let courseTeamsMap = $state<Map<string, CourseInstructor[]>>(new Map());

	// Phase 2: entry IDs ที่อยู่ในสถานะ pending (รอ DB confirm) — UI แสดง spinner
	let pendingEntryIds = $state<SvelteSet<string>>(new SvelteSet());

	// Phase 2: snapshot table — สำหรับ rollback ตอน DropRejected (ทั้ง self + others)
	// เฉพาะ entry ที่มี optimistic mutation ที่ยังไม่ confirm
	let optimisticSnapshots = new Map<
		string,
		{ day_of_week: string; period_id: string; room_id: string | null }
	>();

	// Phase 2 Fix 1: timeout per pending entry — ถ้าครบ 15s ยังไม่ confirm → auto rollback
	// ป้องกัน UI ค้างถาวรเมื่อ server crash หรือ WS drop ระหว่าง pending
	let pendingTimeouts = new Map<string, ReturnType<typeof setTimeout>>();
	const PENDING_TIMEOUT_MS = 15000;

	function getSlotKey(day: string, periodId: string) {
		return `${day}_${periodId}`;
	}

	function isSlotOccupiedByInstructor(day: string, periodId: string) {
		return slotConflicts.has(getSlotKey(day, periodId));
	}

	async function fetchInstructorConflicts(course: DragCourse) {
		// 1. Detect MOVE entry (source-of-truth สำหรับทีม via tei)
		let moveEntry: TimetableEntry | undefined;
		if (dragType === 'MOVE' && draggedEntryId) {
			moveEntry =
				timetableEntries.find((e) => e.id === draggedEntryId) ??
				rawTeamEntries.find((e) => e.id === draggedEntryId);
		}

		// 2. Target classroom (ที่จะวาง entry ลง)
		let targetClassroomId: string | undefined;
		if (viewMode === 'INSTRUCTOR') {
			targetClassroomId = moveEntry?.classroom_id ?? course._classroom_id ?? course.classroom_id;
		} else {
			targetClassroomId = selectedClassroomId;
		}
		if (!targetClassroomId) {
			slotConflicts.clear();
			return;
		}

		// 3. หาทีมครู
		let teamIds: string[] = [];
		if (moveEntry) {
			// MOVE: ใช้ tei ปัจจุบันของ entry (source-of-truth ว่าใครสอน cell นี้จริง)
			teamIds = moveEntry.instructor_ids ?? [];
		} else if (course._isActivity) {
			// ACTIVITY NEW — Independent เท่านั้น (synchronized ลากไม่ได้): 1 ครู/ห้อง
			if (course.activity_slot_id) {
				try {
					const res = await listSlotClassroomAssignments(course.activity_slot_id);
					const a = (res.data ?? []).find((x) => x.classroom_id === targetClassroomId);
					if (a) teamIds = [a.instructor_id];
				} catch {
					/* ignore */
				}
			}
		} else if (course.id) {
			// COURSE NEW: ทีมจาก classroom_course_instructors
			try {
				const res = await listCourseInstructors(course.id);
				teamIds = (res.data ?? []).map((m) => m.instructor_id);
			} catch {
				/* ignore */
			}
		}

		// 4. Fetch parallel: entries ของห้องปลายทาง + entries ของครูแต่ละคนในทีม
		try {
			const results = await Promise.all([
				listTimetableEntries({
					classroom_id: targetClassroomId,
					academic_semester_id: selectedSemesterId
				}),
				...teamIds.map((id) =>
					listTimetableEntries({
						instructor_id: id,
						academic_semester_id: selectedSemesterId
					})
				)
			]);
			const classroomEntries = results[0].data;
			const teamEntriesList = results.slice(1).map((r) => r.data);

			// Build ใน SvelteMap ใหม่ก่อน แล้วค่อย clear + copy ไปที่ slotConflicts (reactive)
			// เพื่อไม่ให้ UI "กระพริบ" ตอน populate (clear ทันทีจะเห็นเปล่าชั่วขณะก่อน set ชุดใหม่)
			const newConflicts = new SvelteMap<string, SlotConflict[]>();
			const addConflict = (key: string, c: SlotConflict) => {
				if (!newConflicts.has(key)) newConflicts.set(key, []);
				newConflicts.get(key)!.push(c);
			};

			// 5a. Classroom-busy: ห้องปลายทางติดอยู่แล้ว และผู้ drop ไม่ได้อยู่ใน tei
			//     pivotInstructor = ใครเป็นผู้ drop ของ cell นี้
			//     - INSTRUCTOR view: selectedInstructor (grid viewer)
			//     - CLASSROOM view: ไม่เช็ก pivot — ถ้าห้องติด = conflict เสมอ (ยังไม่ map ผู้ drop ได้ชัด)
			const pivotInstructor = viewMode === 'INSTRUCTOR' ? selectedInstructorId : undefined;
			classroomEntries.forEach((entry) => {
				if (dragType === 'MOVE' && entry.id === draggedEntryId) return;
				if (pivotInstructor && (entry.instructor_ids ?? []).includes(pivotInstructor)) return;

				const key = getSlotKey(entry.day_of_week, entry.period_id);
				addConflict(key, {
					kind: 'classroom',
					subject_code: entry.subject_code || getEntryTypeFallbackLabel(entry.entry_type),
					subject_name: entry.subject_name_th || entry.title || '',
					classroom_name: entry.classroom_name ?? '',
					room_code: entry.room_code
				});
			});

			// 5b. Team-busy: ครูแต่ละคนในทีมติดคาบที่ห้องอื่นในเวลาเดียวกัน (double-book)
			teamIds.forEach((tid, idx) => {
				const entries = teamEntriesList[idx];
				const teacherInfo = instructors.find((i) => i.id === tid);
				const teacherName = teacherInfo?.name ?? '?';

				entries.forEach((entry) => {
					if (dragType === 'MOVE' && entry.id === draggedEntryId) return;
					if (entry.classroom_id === targetClassroomId) return; // ห้องเดียวกัน = ไม่ใช่ double-book

					const key = getSlotKey(entry.day_of_week, entry.period_id);
					addConflict(key, {
						kind: 'teacher',
						teacher_name: teacherName,
						subject_code: entry.subject_code || getEntryTypeFallbackLabel(entry.entry_type),
						subject_name: entry.subject_name_th || entry.title || '',
						classroom_name: entry.classroom_name ?? '',
						room_code: entry.room_code
					});
				});
			});

			slotConflicts.clear();
			newConflicts.forEach((v, k) => slotConflicts.set(k, v));
		} catch (e) {
			console.error('Failed to check conflicts', e);
		}
	}

	function createDragFrame(code: string, bgColor: string, borderColor: string) {
		const div = document.createElement('div');
		div.style.cssText = `
			position: fixed;
			top: -1000px;
			left: -1000px;
			width: 120px;
			height: 70px;
			border: 2px solid ${borderColor};
			border-radius: 6px;
			background: transparent;
			box-sizing: border-box;
			padding: 4px 6px;
			display: flex;
			align-items: flex-start;
			justify-content: flex-start;
			font-family: system-ui, -apple-system, sans-serif;
			transform: rotate(-2deg);
		`;
		if (code) {
			const pill = document.createElement('div');
			pill.style.cssText = `
				background: ${bgColor};
				border: 1px solid ${borderColor};
				color: #1e293b;
				font-size: 11px;
				font-weight: 700;
				padding: 1px 6px;
				border-radius: 4px;
				white-space: nowrap;
				overflow: hidden;
				text-overflow: ellipsis;
				max-width: 100%;
			`;
			pill.textContent = code;
			div.appendChild(pill);
		}
		document.body.appendChild(div);
		return div;
	}

	function handleActivityDragStart(
		event: DragEvent,
		activity: (typeof unscheduledActivities)[number]
	) {
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

		// conflict highlights: ใช้ draggedCourse ที่มี _isActivity + _classroom_id
		// fetchInstructorConflicts จัดการ target classroom + team lookup เองตาม viewMode
		fetchInstructorConflicts(draggedCourse);

		if (event.dataTransfer) {
			event.dataTransfer.effectAllowed = 'copy';
			event.dataTransfer.setData('text/plain', JSON.stringify({ type: 'NEW', id: activity.id }));
			// Frame-only drag ghost (โปร่งตรงกลาง → popup ทะลุเห็นได้)
			const code = ACTIVITY_TYPE_LABELS[activity.activity_type] ?? activity.activity_type;
			const bgColor = getSubjectColor(code, 'ACTIVITY');
			const borderColor = getSubjectBorderColor(code, 'ACTIVITY');
			const dragElement = createDragFrame(code, bgColor, borderColor);
			event.dataTransfer.setDragImage(dragElement, 60, 35);
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

	function handleDragStart(
		event: DragEvent,
		item: DragCourse | TimetableEntry,
		type: 'NEW' | 'MOVE'
	) {
		dragType = type;

		let courseToCheck: DragCourse | null = null;

		if (type === 'NEW') {
			draggedCourse = item;
			draggedEntryId = null;
			courseToCheck = item;
		} else {
			draggedCourse = item;
			draggedEntryId = item.id;

			const entry = item as TimetableEntry;
			const isActivityEntry = entry.entry_type === 'ACTIVITY';
			const originalCourse = isActivityEntry
				? null
				: courses.find((c) => c.id === entry.classroom_course_id);
			courseToCheck = originalCourse || {
				...entry,
				id: entry.classroom_course_id ?? entry.activity_slot_id ?? entry.id,
				subject_code: entry.subject_code ?? (isActivityEntry ? entry.title : undefined),
				title: entry.subject_name_th ?? entry.title,
				title_th: entry.subject_name_th ?? entry.title,
				_isActivity: isActivityEntry,
				activity_slot_id: entry.activity_slot_id
			};
		}

		if (courseToCheck) {
			fetchInstructorConflicts(courseToCheck);
		}

		// Precompute drop validity for MOVE drags (colorize cells 🟢🔵🔴)
		// Local computation — no API call, 0ms lag
		if (type === 'MOVE' && draggedEntryId) {
			moveValidityMap = computeValidMoves(draggedEntryId);
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

			// Frame-only drag ghost — สี/ขนาดตาม subject theme, กลางโปร่ง → popup ทะลุได้
			const code = courseToCheck!.subject_code || courseToCheck!.title || 'วิชา';
			const entryType = (courseToCheck as unknown as { entry_type?: string }).entry_type;
			const bgColor = getSubjectColor(code, entryType);
			const borderColor = getSubjectBorderColor(code, entryType);
			const dragElement = createDragFrame(code, bgColor, borderColor);
			event.dataTransfer.setDragImage(dragElement, 60, 35);
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
						code: courseToCheck!.subject_code || '??',
						title:
							courseToCheck!.title_th ||
							courseToCheck!.title_en ||
							courseToCheck!.title ||
							'รายวิชา',
						color: courseToCheck!.color as string | undefined
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
		slotConflicts.clear();
		hoverDragCell = null;
		moveValidityMap = new Map();
	}

	function handleDragOver(event: DragEvent, day?: string, periodId?: string) {
		event.preventDefault();
		if (event.dataTransfer) {
			event.dataTransfer.dropEffect = dragType === 'NEW' ? 'copy' : 'move';
		}
		// Track cell ที่ cursor hover ระหว่าง drag เพื่อแสดง floating popup
		if (day && periodId && draggedCourse && slotConflicts.has(getSlotKey(day, periodId))) {
			hoverDragCell = { day, periodId, x: event.clientX, y: event.clientY };
		} else {
			hoverDragCell = null;
		}
	}

	async function handleDrop(event: DragEvent, day: string, periodId: string) {
		event.preventDefault();

		if (!draggedCourse) return;

		// Lock: block ถ้ามี user อื่นเปิด dialog อยู่ที่ slot/entry นี้
		const actSlot = getRemoteActivityForSlot(day, periodId);
		const existingEntry = getEntryForSlot(day, periodId);
		const actEntry = existingEntry ? getRemoteActivityForEntry(existingEntry.id) : null;
		const lockedAct = actSlot || actEntry;
		if (lockedAct) {
			toast.error(
				`${lockedAct.user.name} กำลัง${activityLabel(lockedAct.activity)}ที่ช่องนี้ — ลองอีกครั้ง`
			);
			handleDragEnd();
			return;
		}

		// Block: ห้ามวาง/สลับทับ entry ที่สร้างจาก batch (TEXT/SLOT-sync — pinned)
		if (existingEntry?.batch_id) {
			toast.error('คาบนี้สร้างจาก Batch — ย้าย/สลับไม่ได้ (ลบแล้ว batch ใหม่แทน)');
			handleDragEnd();
			return;
		}

		// Phase 2 Fix 2: block drop ทับ entry ที่ยัง pending (กัน phantom race)
		if (existingEntry && pendingEntryIds.has(existingEntry.id)) {
			toast.error('คาบนี้กำลังบันทึก — รอสักครู่แล้วลองใหม่');
			handleDragEnd();
			return;
		}
		if (existingEntry && existingEntry.id.startsWith('temp-')) {
			toast.error('คาบนี้ยังไม่ confirm — รอสักครู่');
			handleDragEnd();
			return;
		}

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

		// Case A: MOVE drag (from table) onto occupied → SWAP (optimistic + WS intent broadcast)
		if (
			existingEntry &&
			dragType === 'MOVE' &&
			draggedEntryId &&
			existingEntry.id !== draggedEntryId
		) {
			const validity = moveValidityMap.get(`${day}|${periodId}`);
			if (validity && !validity.valid) {
				toast.error(validity.reason || 'สลับไม่ได้');
				handleDragEnd();
				return;
			}
			const aId = draggedEntryId;
			const bId = existingEntry.id;
			const aOriginal = snapshotPosition(aId);
			const bOriginal = snapshotPosition(bId);
			if (!aOriginal || !bOriginal) {
				handleDragEnd();
				return;
			}
			// Optimistic swap: ขยับ UI ทันที, รอ API ใน background
			optimisticSnapshots.set(aId, aOriginal);
			optimisticSnapshots.set(bId, bOriginal);
			markPending(aId);
			markPending(bId);
			applyEntryMutation(aId, {
				day_of_week: bOriginal.day_of_week as TimetableEntry['day_of_week'],
				period_id: bOriginal.period_id
			});
			applyEntryMutation(bId, {
				day_of_week: aOriginal.day_of_week as TimetableEntry['day_of_week'],
				period_id: aOriginal.period_id
			});
			// Broadcast intent → คนอื่นเห็น optimistic ทันที
			sendDropIntent({
				kind: 'swap',
				entry_id: aId,
				day_of_week: bOriginal.day_of_week,
				period_id: bOriginal.period_id,
				swap_partner_id: bId,
				swap_partner_day: aOriginal.day_of_week,
				swap_partner_period_id: aOriginal.period_id
			});
			handleDragEnd();
			try {
				await swapTimetableEntries(aId, bId);
				// API success = confirm — clear pending ทันที (ไม่รอ WS เพราะ broadcast
				// อาจ skip ถ้ามี subscriber คนเดียว). WS ยังมาถึงคนอื่น (idempotent)
				clearPending(aId);
				clearPending(bId);
				toast.success('สลับคาบเรียบร้อย');
			} catch (e: unknown) {
				// Rollback locally; DropRejected broadcast (จาก server) จะมาแยก
				applyEntryMutation(aId, {
					day_of_week: aOriginal.day_of_week as TimetableEntry['day_of_week'],
					period_id: aOriginal.period_id
				});
				applyEntryMutation(bId, {
					day_of_week: bOriginal.day_of_week as TimetableEntry['day_of_week'],
					period_id: bOriginal.period_id
				});
				clearPending(aId);
				clearPending(bId);
				toast.error((e instanceof Error ? e.message : String(e)) || 'สลับไม่สำเร็จ');
			}
			return;
		}

		// Case B: NEW drag (from sidebar) onto occupied → REPLACE (optimistic + WS intent)
		if (existingEntry && dragType === 'NEW') {
			const dc = draggedCourse;
			if (!dc) {
				handleDragEnd();
				return;
			}
			const payload: UpdateTimetableEntryRequest = {};
			let newClassroomId: string | undefined;
			if (dc._isActivity) {
				payload.activity_slot_id = dc.activity_slot_id;
				payload.classroom_course_id = null;
			} else {
				payload.classroom_course_id = dc.id;
				payload.activity_slot_id = null;
				if (dc.classroom_id && dc.classroom_id !== existingEntry.classroom_id) {
					payload.classroom_id = dc.classroom_id;
					newClassroomId = dc.classroom_id;
				}
			}

			// Compute new content + apply optimistic
			const newFields = computeReplacementFields({
				newCourseId: dc._isActivity ? undefined : dc.id,
				newActivitySlotId: dc._isActivity ? dc.activity_slot_id : undefined,
				newClassroomId,
				newTitle: dc._isActivity ? dc.title_th : undefined
			});
			const snapshot = applyEntryMutation(existingEntry.id, newFields);
			// Mirror occupancy ครบ (instructor_ids เปลี่ยน → ต้อง upsert)
			const updatedEntry = timetableEntries.find((e) => e.id === existingEntry.id)
				?? rawTeamEntries.find((e) => e.id === existingEntry.id);
			if (updatedEntry) upsertOccupancy(updatedEntry);
			if (snapshot) optimisticSnapshots.set(existingEntry.id, {
				day_of_week: existingEntry.day_of_week,
				period_id: existingEntry.period_id,
				room_id: existingEntry.room_id ?? null
			});
			markPending(existingEntry.id);

			// Broadcast intent → คนอื่น lookup local courses → overwrite
			sendDropIntent({
				kind: 'replace',
				entry_id: existingEntry.id,
				day_of_week: existingEntry.day_of_week,
				period_id: existingEntry.period_id,
				room_id: existingEntry.room_id ?? null,
				new_classroom_course_id: dc._isActivity ? null : dc.id,
				new_activity_slot_id: dc._isActivity ? dc.activity_slot_id : null,
				new_classroom_id: newClassroomId ?? null
			});

			handleDragEnd();
			try {
				const result = (await updateTimetableEntry(existingEntry.id, payload)) as {
					success?: boolean;
					conflicts?: ConflictInfo[];
				};
				if (result?.success === false) {
					// Rollback content
					if (snapshot) applyEntryMutation(existingEntry.id, snapshot);
					const restored = timetableEntries.find((e) => e.id === existingEntry.id)
						?? rawTeamEntries.find((e) => e.id === existingEntry.id);
					if (restored) upsertOccupancy(restored);
					clearPending(existingEntry.id);
					const msgs = (result.conflicts ?? []).map((c) => c.message).filter(Boolean);
					toast.error(msgs.length > 0 ? msgs.join(' · ') : 'แทนที่ไม่สำเร็จ');
				} else {
					// API success = confirm
					clearPending(existingEntry.id);
					toast.success('แทนที่รายการเดิมแล้ว');
				}
			} catch (e: unknown) {
				if (snapshot) applyEntryMutation(existingEntry.id, snapshot);
				const restored = timetableEntries.find((x) => x.id === existingEntry.id)
					?? rawTeamEntries.find((x) => x.id === existingEntry.id);
				if (restored) upsertOccupancy(restored);
				clearPending(existingEntry.id);
				toast.error((e instanceof Error ? e.message : String(e)) || 'แทนที่ไม่สำเร็จ');
			}
			return;
		}

		// Case C: dropping onto own source — no-op
		if (existingEntry && existingEntry.id === draggedEntryId) {
			handleDragEnd();
			return;
		}

		if (isSlotOccupiedByInstructor(day, periodId)) {
			toast.error(
				viewMode === 'INSTRUCTOR' ? 'ห้องนี้มีวิชาอื่นในคาบนี้แล้ว' : 'ครูติดสอนในคาบนี้แล้ว'
			);
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

	async function updateUnavailableRooms(day: string, periodId: string) {
		unavailableRooms = new Set();
		try {
			const res = await listTimetableEntries({
				day_of_week: day,
				academic_semester_id: selectedSemesterId
			});

			const busyRooms = new SvelteSet<string>();
			res.data.forEach((entry) => {
				if (entry.period_id === periodId && entry.room_id) {
					if (dragType === 'MOVE' && entry.id === draggedEntryId) return;
					busyRooms.add(entry.room_id);
				}
			});
			unavailableRooms = busyRooms;
		} catch (e) {
			console.error(e);
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
				// CREATE NEW — optimistic + WS intent broadcast
				const courseCode = course.subject_code || course.title || 'รายการ';
				const ac = course;
				const tempId = `temp-${crypto.randomUUID()}`;
				let payload: CreateTimetableEntryRequest;
				let entryType: 'COURSE' | 'ACTIVITY' = 'COURSE';
				let dropClassroomId = selectedClassroomId;

				if (ac._isActivity) {
					entryType = 'ACTIVITY';
					dropClassroomId = ac._classroom_id || selectedClassroomId;
					// Check if independent slot has instructor assigned for this classroom
					const slot =
						sidebarActivitySlots.find((s) => s.id === ac.activity_slot_id) ||
						instructorActivityItems.find((i) => i.slot.id === ac.activity_slot_id)?.slot;
					if (slot?.scheduling_mode === 'independent') {
						const hasInstructor = await checkClassroomHasInstructor(
							ac.activity_slot_id!,
							dropClassroomId
						);
						if (!hasInstructor) {
							toast.error('กรุณากำหนดครูประจำห้องนี้ก่อนในหน้ากิจกรรม');
							submitting = false;
							pendingDropContext = null;
							isDropPending = false;
							handleDragEnd();
							return;
						}
					}
					payload = {
						activity_slot_id: ac.activity_slot_id,
						classroom_id: dropClassroomId,
						academic_semester_id: selectedSemesterId,
						day_of_week: day,
						period_id: periodId,
						room_id: roomId,
						entry_type: 'ACTIVITY',
						title: ac.title_th,
						client_temp_id: tempId
					};
				} else {
					payload = {
						classroom_course_id: course.id,
						day_of_week: day,
						period_id: periodId,
						room_id: roomId,
						client_temp_id: tempId
					};
				}

				// Optimistic: build tempEntry + push + broadcast intent
				const tempEntry = buildTempEntry({
					tempId,
					classroomId: dropClassroomId,
					classroomCourseId: ac._isActivity ? undefined : course.id,
					activitySlotId: ac._isActivity ? ac.activity_slot_id : undefined,
					day: day as TimetableEntry['day_of_week'],
					periodId,
					roomId,
					title: ac._isActivity ? ac.title_th : undefined,
					entryType
				});
				timetableEntries = [...timetableEntries, tempEntry];
				upsertOccupancy(tempEntry);
				markPending(tempId);

				sendEntryIntent({
					temp_id: tempId,
					classroom_id: dropClassroomId,
					classroom_course_id: ac._isActivity ? null : course.id,
					activity_slot_id: ac._isActivity ? ac.activity_slot_id : null,
					day_of_week: day,
					period_id: periodId,
					room_id: roomId ?? null,
					title: ac._isActivity ? ac.title_th : null,
					entry_type: entryType
				});

				submitting = false;
				pendingDropContext = null;
				isDropPending = false;
				handleDragEnd();
				try {
					const res = (await createTimetableEntry(payload)) as {
						success?: boolean;
						conflicts?: ConflictInfo[];
						data?: TimetableEntry;
					};
					if (res?.success === false) {
						// Remove temp + show conflicts
						timetableEntries = timetableEntries.filter((e) => e.id !== tempId);
						removeOccupancy(tempId);
						clearPending(tempId);
						const msgs = (res.conflicts ?? []).map((c) => c.message).filter(Boolean);
						toast.error(msgs.length > 0 ? msgs.join(' · ') : 'ลงตารางไม่สำเร็จ');
					} else {
						// API success = confirm. Swap temp → real (id เปลี่ยน, joined fields เก็บที่คำนวณไว้)
						const realId = res?.data?.id;
						if (realId) {
							const swap = (e: TimetableEntry) =>
								e.id === tempId ? { ...e, id: realId } : e;
							timetableEntries = timetableEntries.map(swap);
							rawTeamEntries = rawTeamEntries.map(swap);
							removeOccupancy(tempId);
							const finalEntry = timetableEntries.find((e) => e.id === realId);
							if (finalEntry) upsertOccupancy(finalEntry);
						}
						clearPending(tempId);
						toast.success(`ลงตาราง ${courseCode} สำเร็จ`);
					}
				} catch (e: unknown) {
					timetableEntries = timetableEntries.filter((x) => x.id !== tempId);
					removeOccupancy(tempId);
					clearPending(tempId);
					toast.error((e instanceof Error ? e.message : String(e)) || 'ลงตารางไม่สำเร็จ');
				}
				return;
			} else if (dragType === 'MOVE' && entryId) {
				// UPDATE EXISTING — optimistic move + WS intent broadcast
				const courseName = course.subject_code || course.title || 'รายการ';

				const payload = {
					day_of_week: day,
					period_id: periodId,
					room_id: roomId
				};

				const original = snapshotPosition(entryId);
				if (original) optimisticSnapshots.set(entryId, original);
				markPending(entryId);
				// Optimistic mutation
				applyEntryMutation(entryId, {
					day_of_week: day as TimetableEntry['day_of_week'],
					period_id: periodId,
					room_id: roomId
				});
				// Broadcast intent → คนอื่นเห็น optimistic ทันที
				sendDropIntent({
					kind: 'move',
					entry_id: entryId,
					day_of_week: day,
					period_id: periodId,
					room_id: roomId ?? null
				});
				submitting = false;
				pendingDropContext = null;
				isDropPending = false;
				handleDragEnd();
				try {
					const res = (await updateTimetableEntry(entryId, payload)) as {
						success?: boolean;
						conflicts?: ConflictInfo[];
					};
					if (res?.success === false) {
						if (original) {
							applyEntryMutation(entryId, {
								day_of_week: original.day_of_week as TimetableEntry['day_of_week'],
								period_id: original.period_id,
								room_id: original.room_id
							});
						}
						clearPending(entryId);
						const msgs = (res.conflicts ?? []).map((c) => c.message).filter(Boolean);
						toast.error(msgs.length > 0 ? msgs.join(' · ') : 'ย้ายไม่สำเร็จ');
					} else {
						// API success = confirm
						clearPending(entryId);
						toast.success(`ย้าย ${courseName} สำเร็จ`);
					}
				} catch (e: unknown) {
					if (original) {
						applyEntryMutation(entryId, {
							day_of_week: original.day_of_week as TimetableEntry['day_of_week'],
							period_id: original.period_id,
							room_id: original.room_id
						});
					}
					clearPending(entryId);
					toast.error((e instanceof Error ? e.message : String(e)) || 'ย้ายไม่สำเร็จ');
				}
				return;
			}
		} catch (e: unknown) {
			toast.error((e instanceof Error ? e.message : String(e)) || 'บันทึกไม่สำเร็จ');
		} finally {
			// Notify others
			// Backend broadcasts patch event ให้แล้ว — ไม่ต้องส่ง TableRefresh ซ้ำ

			submitting = false;
			pendingDropContext = null;
			isDropPending = false;
			handleDragEnd();
		}
	}

	async function handleResponse(
		res: { success?: boolean; conflicts?: ConflictInfo[] } | unknown,
		successMessage: string
	) {
		if ((res as { success?: boolean }).success === false) {
			// รวบ conflict messages เป็นไทยเดียว ไม่โชว์ "Conflict detected"
			const r = res as { success?: boolean; conflicts?: ConflictInfo[] };
			const msgs: string[] = (r.conflicts ?? [])
				.map((c: ConflictInfo) => c.message)
				.filter(Boolean);
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

			const semesterName = semesters.find((s) => s.id === selectedSemesterId)?.term || '';
			const yearObj = academicYears.find((y) => y.id === selectedYearId);
			const yearName = (yearObj?.name || '').replace('ปีการศึกษา', '').trim();
			const subTitle = `ภาคเรียนที่ ${semesterName} ปีการศึกษา ${yearName}`;

			// fetch entries ของทุก target พร้อมกัน แล้วรวมเป็น pages เดียวสำหรับ PDF
			const pagePromises = exportTargetIds.map(async (id) => {
				let entries: TimetableEntry[] = [];
				let title = '';

				if (exportType === 'CLASSROOM') {
					const room = classrooms.find((c) => c.id === id);
					if (!room) return null;
					let roomName = room.name;
					if (roomName.startsWith('ม.')) roomName = roomName.replace('ม.', 'มัธยมศึกษาปีที่ ');
					else if (roomName.startsWith('ป.')) roomName = roomName.replace('ป.', 'ประถมศึกษาปีที่ ');
					else if (roomName.startsWith('อ.')) roomName = roomName.replace('อ.', 'อนุบาลปีที่ ');
					else if (/^\d/.test(roomName)) roomName = `มัธยมศึกษาปีที่ ${roomName}`;
					title = `ตารางเรียน ชั้น${roomName}`;
					const res = await listTimetableEntries({
						classroom_id: id,
						academic_semester_id: selectedSemesterId
					});
					entries = res.data;
				} else {
					const teacher = instructors.find((inst) => inst.id === id);
					if (!teacher) return null;
					const targetName = teacher.name.startsWith('ครู') ? teacher.name : `ครู${teacher.name}`;
					title = `ตารางสอน ${targetName}`;
					const res = await listTimetableEntries({
						instructor_id: id,
						academic_semester_id: selectedSemesterId
					});
					entries = res.data;
				}

				return {
					title,
					subTitle,
					periods,
					timetableEntries: entries,
					viewMode: exportType
				};
			});

			const results = await Promise.all(pagePromises);
			const pages = results.filter((p): p is NonNullable<typeof p> => p !== null);

			if (pages.length === 0) {
				toast.error('ไม่พบข้อมูลสำหรับรายการที่เลือก');
				return;
			}

			// ตั้งชื่อไฟล์: ถ้าหน้าเดียวใช้ title, หลายหน้าใช้ชื่อรวม
			const fileName =
				pages.length === 1
					? pages[0].title
					: `${exportType === 'CLASSROOM' ? 'ตารางเรียน' : 'ตารางสอน'}_รวม_${pages.length}รายการ_${yearName}_ภาค${semesterName}`;

			await generateTimetablePDF(pages, fileName);

			toast.success(
				pages.length === 1
					? 'ดาวน์โหลดเสร็จสิ้น'
					: `ดาวน์โหลดไฟล์รวม ${pages.length} รายการเสร็จสิ้น`
			);
			showExportModal = false;
		} catch (e: unknown) {
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
		const courseCounts = new SvelteMap<string, number>();
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
		if (viewMode === 'INSTRUCTOR') {
			const items: UnscheduledActivity[] = [];

			// Synchronized: count unique (day, period) ต่อ slot
			// (1 คาบ logical = N entries ของ N ห้อง — นับเป็น 1)
			for (const slot of sidebarActivitySlots) {
				const seen = new SvelteSet<string>();
				for (const entry of timetableEntries) {
					if (entry.activity_slot_id === slot.id) {
						seen.add(`${entry.day_of_week}:${entry.period_id}`);
					}
				}
				const scheduled = seen.size;
				if (scheduled < slot.periods_per_week) {
					items.push({
						...slot,
						scheduled_count: scheduled,
						max_periods: slot.periods_per_week,
						is_completed: false,
						is_draggable: false,
						_classroom_id: undefined,
						_classroom_name: undefined
					});
				}
			}

			// Independent items per classroom (draggable) — นับ entries ตรง ๆ ต่อ slot+classroom
			const indepCounts = new SvelteMap<string, number>();
			for (const entry of timetableEntries) {
				if (entry.activity_slot_id && entry.classroom_id) {
					const key = `${entry.activity_slot_id}:${entry.classroom_id}`;
					indepCounts.set(key, (indepCounts.get(key) || 0) + 1);
				}
			}
			for (const item of instructorActivityItems) {
				const key = `${item.slot.id}:${item.classroom_id}`;
				const scheduled = indepCounts.get(key) || 0;
				if (scheduled < item.slot.periods_per_week) {
					items.push({
						...item.slot,
						name: `${item.slot.name} — ${item.classroom_name}`,
						scheduled_count: scheduled,
						max_periods: item.slot.periods_per_week,
						is_completed: false,
						is_draggable: true,
						_classroom_id: item.classroom_id,
						_classroom_name: item.classroom_name
					});
				}
			}

			return items;
		}

		// CLASSROOM view: 1 entry = 1 คาบ (entries ถูก filter เหลือห้องเดียวอยู่แล้ว)
		const slotCounts = new SvelteMap<string, number>();
		timetableEntries.forEach((entry) => {
			if (entry.activity_slot_id) {
				slotCounts.set(
					entry.activity_slot_id,
					(slotCounts.get(entry.activity_slot_id) || 0) + 1
				);
			}
		});

		return sidebarActivitySlots
			.map((slot) => {
				const scheduled = slotCounts.get(slot.id) || 0;
				const maxPeriods = slot.periods_per_week;
				return {
					...slot,
					scheduled_count: scheduled,
					max_periods: maxPeriods,
					is_completed: scheduled >= maxPeriods,
					is_draggable: slot.scheduling_mode === 'independent',
					_classroom_id: undefined as string | undefined,
					_classroom_name: undefined as string | undefined
				};
			})
			.filter((s) => !s.is_completed);
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

	// Occupancy = semester-wide (ไม่ขึ้นกับ view) → load แยก ตอน semester เปลี่ยน
	$effect(() => {
		if (selectedSemesterId) {
			loadOccupancy();
		} else {
			occupancyEntries = [];
		}
	});

	// Batch Assign State
	let showBatchModal = $state(false);
	let batchClassrooms = $state<string[]>([]);
	let batchInstructors = $state<string[]>([]);
	let batchDays = $state<string[]>(['MON']);
	let batchPeriodIds = $state<string[]>([]);
	let batchType = $state('ACTIVITY');
	let batchTitle = $state('');
	let batchRoomId = $state('none');

	// Batch Mode State
	let batchMode = $state<'TEXT' | 'SLOT'>('TEXT');

	// Activity Slot mode
	let activitySlots = $state<ActivitySlot[]>([]);
	let batchSlotId = $state('');
	let loadingSlots = $state(false);
	// SLOT mode: auto-populate ห้อง + ครู จาก slot metadata เมื่อเลือก slot
	// (user สามารถ uncheck ได้ถ้าอยากเอาออก)
	$effect(() => {
		const sid = batchSlotId;
		if (batchMode === 'SLOT' && sid && showBatchModal) {
			const slot = activitySlots.find((s) => s.id === sid);
			if (slot?.classroom_ids) {
				batchClassrooms = [...slot.classroom_ids];
			}
			listSlotInstructors(sid)
				.then((res) => {
					if (batchSlotId === sid) {
						batchInstructors = (res.data ?? []).map((i) => i.user_id);
					}
				})
				.catch(() => {});
		}
	});

	async function ensureActivitySlotsLoaded() {
		if (activitySlots.length > 0 || !selectedSemesterId) return;
		loadingSlots = true;
		try {
			const res = await listActivitySlots({ semester_id: selectedSemesterId });
			// Batch รองรับเฉพาะ synchronized (ต้องตรงกันทุกห้อง)
			// Independent → ลากทีละห้องจาก sidebar ไม่ใช่ batch
			activitySlots = res.data.filter((s) => s.scheduling_mode === 'synchronized');
		} catch (e) {
			console.error(e);
			toast.error('โหลดข้อมูล Activity Slot ไม่สำเร็จ');
		} finally {
			loadingSlots = false;
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

	let filteredBatchClassroomsList = $derived.by(() => {
		let list = classrooms;

		// SLOT mode: filter เฉพาะห้องที่เข้าร่วม slot จริง (junction) ไม่ใช่ catalog template
		if (batchMode === 'SLOT' && batchSlotId) {
			const slot = activitySlots.find((s) => s.id === batchSlotId);
			if (slot?.classroom_ids) {
				list = list.filter((c) => slot.classroom_ids!.includes(c.id));
			}
		}

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
		// SLOT mode: ต้องมีห้อง (ครูมาอัตโนมัติจาก slot); TEXT mode: ห้องหรือครู อย่างน้อย 1
		if (batchMode === 'SLOT') {
			if (batchClassrooms.length === 0) {
				toast.error('กรุณาเลือกห้องเรียนอย่างน้อย 1 ห้อง');
				return;
			}
		} else if (batchClassrooms.length === 0 && batchInstructors.length === 0) {
			toast.error('กรุณาเลือกห้องเรียน หรือ ครู อย่างน้อย 1 อย่าง');
			return;
		}
		if (batchPeriodIds.length === 0) {
			toast.error('กรุณาเลือกคาบเวลาอย่างน้อย 1 คาบ');
			return;
		}
		if (batchDays.length === 0) {
			toast.error('กรุณาเลือกวันอย่างน้อย 1 วัน');
			return;
		}

		// Validate based on mode
		if (batchMode === 'TEXT' && !batchTitle) {
			toast.error('กรุณาระบุชื่อกิจกรรม');
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
			let slotIdToSend: string | undefined = undefined;

			if (batchMode === 'SLOT') {
				const slot = activitySlots.find((s) => s.id === batchSlotId);
				titleToSend = slot?.name || '';
				entryTypeToSend = 'ACTIVITY';
				slotIdToSend = batchSlotId;
			}

			// Backend รับ days_of_week: string[] → call เดียวจบ ทุก entry อยู่ใน batch เดียวกัน
			// SLOT mode: tei attach ครูใน batchInstructors เข้า classroom entries (ไม่สร้าง
			// teacher-only entry ซ้ำ — backend ข้ามให้เมื่อ activity_slot_id present)
			const res = await createBatchTimetableEntries({
				classroom_ids: batchClassrooms,
				instructor_ids: batchInstructors,
				days_of_week: batchDays,
				period_ids: batchPeriodIds,
				academic_semester_id: selectedSemesterId,
				entry_type: entryTypeToSend as 'ACTIVITY' | 'BREAK' | 'HOMEROOM' | 'ACADEMIC',
				title: titleToSend,
				room_id: batchRoomId === 'none' ? undefined : batchRoomId,
				force: batchForce,
				activity_slot_id: slotIdToSend
			});

			if (res.success === false && res.conflicts) {
				toast.error('พบรายการที่ชนกัน');
				for (const c of res.conflicts as ConflictInfo[]) toast.error(c.message);
				submitting = false;
				return;
			}

			toast.success('บันทึกกิจกรรมเรียบร้อย');
			showBatchModal = false;

			// Reset fields
			batchTitle = '';
			batchSlotId = '';
			batchInstructors = [];

			// Reload เสมอ — batch สามารถกระทบ view ปัจจุบันได้ทางอ้อม
			// (force=true ลบคาบเก่า, teacher entries ใน INSTRUCTOR view แม้ไม่ได้ tick ตัวเอง,
			//  tei ที่ derive จาก slot/course)
			loadTimetable();
			loadSidebarActivitySlots();
		} catch (e: unknown) {
			toast.error((e instanceof Error ? e.message : String(e)) || 'บันทึกไม่สำเร็จ');
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
			case 'EntryCreated': {
				// Backend ส่ง entry พร้อม joined fields ครบ
				const created = patch.entry as TimetableEntry;
				clearPending(created.id);

				// ถ้ามี client_temp_id → เป็น CREATE optimistic ที่เรา push tempEntry ไว้แล้ว → swap
				if (patch.client_temp_id) {
					const tempId = patch.client_temp_id;
					clearPending(tempId);
					const swapTempToReal = (e: TimetableEntry) =>
						e.id === tempId ? created : e;
					if (timetableEntries.some((e) => e.id === tempId)) {
						timetableEntries = timetableEntries.map(swapTempToReal);
					}
					if (rawTeamEntries.some((e) => e.id === tempId)) {
						rawTeamEntries = rawTeamEntries.map(swapTempToReal);
					}
					removeOccupancy(tempId);
					upsertOccupancy(created);
					break;
				}

				upsertOccupancy(created);
				// เฉพาะ entry ที่เกี่ยวกับ view ปัจจุบันถึงจะ push
				const relevantForClassroom =
					viewMode === 'CLASSROOM' && created.classroom_id === selectedClassroomId;
				const relevantForInstructor =
					viewMode === 'INSTRUCTOR' &&
					(created.instructor_ids ?? []).includes(selectedInstructorId);
				if (relevantForClassroom || relevantForInstructor) {
					timetableEntries = [...timetableEntries, created];
				}
				if (viewMode === 'INSTRUCTOR' && (created.instructor_ids ?? []).length > 0) {
					// Also keep rawTeamEntries in sync (superset includes ghost cells)
					const teamMembers = courses.some((c) => c.id === created.classroom_course_id);
					if (teamMembers) {
						rawTeamEntries = [...rawTeamEntries, created];
					}
				}
				break;
			}
			case 'EntryUpdated': {
				const updated = patch.entry;
				clearPending(updated.id);
				upsertOccupancy(updated);
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
				clearPending(patch.entry_id);
				removeOccupancy(patch.entry_id);
				updateEntries((arr) => arr.filter((e) => e.id !== patch.entry_id));
				break;
			case 'EntriesSwapped':
				// Swap = 2 entries เปลี่ยน day/period — apply ทั้งคู่
				clearPending(patch.entry_a.id);
				clearPending(patch.entry_b.id);
				upsertOccupancy(patch.entry_a);
				upsertOccupancy(patch.entry_b);
				updateEntries((arr) =>
					arr.map((e) => {
						if (e.id === patch.entry_a.id) return { ...e, ...patch.entry_a };
						if (e.id === patch.entry_b.id) return { ...e, ...patch.entry_b };
						return e;
					})
				);
				break;
			case 'EntryInstructorAdded': {
				const targetId = patch.entry_id;
				const newIid = patch.instructor_id;
				occupancyEntries = occupancyEntries.map((e) =>
					e.id === targetId
						? { ...e, instructor_ids: [...e.instructor_ids, newIid] }
						: e
				);
				updateEntries((arr) =>
					arr.map((e) => {
						if (e.id !== targetId) return e;
						const ids = [...(e.instructor_ids ?? []), newIid];
						const names = [...(e.instructor_names ?? []), patch.instructor_name];
						return { ...e, instructor_ids: ids, instructor_names: names };
					})
				);
				break;
			}
			case 'EntryInstructorRemoved':
				if (patch.entry_deleted) {
					removeOccupancy(patch.entry_id);
					updateEntries((arr) => arr.filter((e) => e.id !== patch.entry_id));
				} else {
					const targetId = patch.entry_id;
					const removeIid = patch.instructor_id;
					occupancyEntries = occupancyEntries.map((e) =>
						e.id === targetId
							? { ...e, instructor_ids: e.instructor_ids.filter((iid) => iid !== removeIid) }
							: e
					);
					updateEntries((arr) =>
						arr.map((e) => {
							if (e.id !== targetId) return e;
							const idx = e.instructor_ids?.indexOf(removeIid) ?? -1;
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
			case 'DropIntent': {
				// Phase 2: คนอื่น drop เสร็จแล้ว → apply optimistic (รอ EntryUpdated/Swapped/DropRejected)
				if (patch.user_id === $authStore.user?.id) break; // ตัวเองทำเอง — apply ไปแล้ว
				const original = snapshotPosition(patch.entry_id);
				if (original && !optimisticSnapshots.has(patch.entry_id)) {
					optimisticSnapshots.set(patch.entry_id, original);
				}
				markPending(patch.entry_id);

				if (patch.kind === 'replace') {
					// REPLACE: lookup new course/activity locally → overwrite content fields
					const newFields = computeReplacementFields({
						newCourseId: patch.new_classroom_course_id ?? undefined,
						newActivitySlotId: patch.new_activity_slot_id ?? undefined,
						newClassroomId: patch.new_classroom_id ?? undefined
					});
					applyEntryMutation(patch.entry_id, newFields);
					const updated = timetableEntries.find((e) => e.id === patch.entry_id)
						?? rawTeamEntries.find((e) => e.id === patch.entry_id);
					if (updated) upsertOccupancy(updated);
				} else {
					// move/swap: เปลี่ยน day/period/room
					applyEntryMutation(patch.entry_id, {
						day_of_week: patch.day_of_week as TimetableEntry['day_of_week'],
						period_id: patch.period_id,
						room_id: patch.room_id ?? undefined
					});
					if (patch.kind === 'swap' && patch.swap_partner_id) {
						const partnerOriginal = snapshotPosition(patch.swap_partner_id);
						if (partnerOriginal && !optimisticSnapshots.has(patch.swap_partner_id)) {
							optimisticSnapshots.set(patch.swap_partner_id, partnerOriginal);
						}
						markPending(patch.swap_partner_id);
						if (patch.swap_partner_day && patch.swap_partner_period_id) {
							applyEntryMutation(patch.swap_partner_id, {
								day_of_week: patch.swap_partner_day as TimetableEntry['day_of_week'],
								period_id: patch.swap_partner_period_id
							});
						}
					}
				}
				break;
			}
			case 'EntryIntent': {
				// คนอื่น drop NEW → render tempEntry จาก local state lookups
				if (patch.user_id === $authStore.user?.id) break; // ตัวเอง — push ไปแล้ว
				const tempEntry = buildTempEntry({
					tempId: patch.temp_id,
					classroomId: patch.classroom_id,
					classroomCourseId: patch.classroom_course_id ?? undefined,
					activitySlotId: patch.activity_slot_id ?? undefined,
					day: patch.day_of_week as TimetableEntry['day_of_week'],
					periodId: patch.period_id,
					roomId: patch.room_id ?? undefined,
					title: patch.title ?? undefined,
					entryType: patch.entry_type === 'ACTIVITY' ? 'ACTIVITY' : 'COURSE'
				});
				upsertOccupancy(tempEntry);
				const relevantClassroom =
					viewMode === 'CLASSROOM' && tempEntry.classroom_id === selectedClassroomId;
				const relevantInstructor =
					viewMode === 'INSTRUCTOR' &&
					(tempEntry.instructor_ids ?? []).includes(selectedInstructorId);
				if (relevantClassroom || relevantInstructor) {
					timetableEntries = [...timetableEntries, tempEntry];
				}
				markPending(patch.temp_id);
				break;
			}
			case 'EntryRejected': {
				// CREATE 409 → ลบ tempEntry ทุก client
				const tempId = patch.temp_id;
				timetableEntries = timetableEntries.filter((e) => e.id !== tempId);
				rawTeamEntries = rawTeamEntries.filter((e) => e.id !== tempId);
				removeOccupancy(tempId);
				clearPending(tempId);
				if (patch.user_id === $authStore.user?.id) {
					toast.error(patch.reason ? `ขัดแย้ง — ${patch.reason}` : 'ลงตารางไม่สำเร็จ');
				}
				break;
			}
			case 'DropRejected': {
				// Phase 2: rollback optimistic state (ทุกคนที่เคยรับ DropIntent)
				const snap = optimisticSnapshots.get(patch.entry_id);
				if (snap) {
					applyEntryMutation(patch.entry_id, {
						day_of_week: snap.day_of_week as TimetableEntry['day_of_week'],
						period_id: snap.period_id,
						room_id: snap.room_id
					});
				} else if (patch.original_day && patch.original_period_id) {
					// fallback: ใช้ original จาก server (ถ้าไม่มี local snapshot)
					applyEntryMutation(patch.entry_id, {
						day_of_week: patch.original_day as TimetableEntry['day_of_week'],
						period_id: patch.original_period_id,
						room_id: patch.original_room_id ?? undefined
					});
				}
				clearPending(patch.entry_id);
				if (patch.partner_id) {
					const psnap = optimisticSnapshots.get(patch.partner_id);
					if (psnap) {
						applyEntryMutation(patch.partner_id, {
							day_of_week: psnap.day_of_week as TimetableEntry['day_of_week'],
							period_id: psnap.period_id,
							room_id: psnap.room_id
						});
					} else if (patch.partner_original_day && patch.partner_original_period_id) {
						applyEntryMutation(patch.partner_id, {
							day_of_week: patch.partner_original_day as TimetableEntry['day_of_week'],
							period_id: patch.partner_original_period_id
						});
					}
					clearPending(patch.partner_id);
				}
				// Toast: เฉพาะคนที่ drop (ตอบโจทย์ feedback option b)
				if (patch.user_id === $authStore.user?.id) {
					toast.error(patch.reason ? `ขัดแย้ง — ${patch.reason}` : 'การลากถูก reject — ย้อนกลับ');
				}
				break;
			}
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
		const myViewId = viewMode === 'CLASSROOM' ? selectedClassroomId : selectedInstructorId;
		for (const [userId, pos] of Object.entries($dragPositions)) {
			if (pos.target_day === day && pos.target_period_id === periodId) {
				const user = $activeUsers.find((u) => u.user_id === userId);
				const drag = $userDrags[userId];
				if (!user || !drag) continue;
				// แสดง drag hover เฉพาะถ้าอยู่ view เดียวกัน
				// (คนละห้อง/คนละครู/คนละ mode → ไม่ควรเห็น cursor ของกัน)
				if (user.context?.view_mode !== viewMode || user.context?.view_id !== myViewId) {
					continue;
				}
				return { user, drag };
			}
		}
		return null;
	}

	onMount(loadInitialData);
</script>

<svelte:head>
	<title>{data.title} - SchoolOrbit</title>
</svelte:head>

<div class="h-full flex flex-col space-y-2" role="application">
	<!-- Row 1: Title + View Mode + Status + Avatars + Actions -->
	<div class="flex items-center justify-between gap-3 flex-wrap">
		<div class="flex items-center gap-3">
			<h2 class="text-3xl font-bold flex items-center gap-2">
				<CalendarDays class="h-8 w-8" />
				จัดตารางสอน
			</h2>

			<div class="flex bg-muted p-1 rounded-lg transition-colors">
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
		</div>

		<div class="flex items-center gap-2 flex-wrap">
			<Tooltip.Provider>
				<Tooltip.Root>
					<Tooltip.Trigger>
						<div
							class="flex items-center gap-1.5 px-2.5 py-1.5 rounded-full border bg-white/50 backdrop-blur shadow-sm"
						>
							<div
								class="w-2 h-2 rounded-full {$isConnected
									? 'bg-green-500 shadow-[0_0_6px_rgba(34,197,94,0.6)]'
									: 'bg-red-500'}"
							></div>
							<span class="text-xs font-semibold text-muted-foreground">
								{$isConnected ? `Online (${$activeUsers.length})` : 'Offline'}
							</span>
						</div>
					</Tooltip.Trigger>
					<Tooltip.Content>
						{$isConnected ? `ออนไลน์ ${$activeUsers.length} คน` : 'ไม่ได้เชื่อมต่อ'}
					</Tooltip.Content>
				</Tooltip.Root>
			</Tooltip.Provider>

			{#if $isConnected && $activeUsers.length > 0}
				<Tooltip.Provider>
					<div class="flex -space-x-1.5">
						{#each $activeUsers.slice(0, 4) as user (user.user_id + (user.context?.view_id || ''))}
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

			<div class="w-px h-5 bg-border mx-1"></div>

			<Button
				variant="outline"
				onclick={() => goto(resolve('/staff/academic/timetable/scheduling-config'))}
			>
				<Zap class="w-4 h-4 mr-2 text-orange-500" />
				จัดอัตโนมัติ
			</Button>

			<Button
				variant="outline"
				onclick={() => goto(resolve('/staff/academic/timetable/templates'))}
			>
				<FileStack class="w-4 h-4 mr-2" />
				Templates
			</Button>

			<Button variant="outline" onclick={() => (showBatchModal = true)}>
				<PlusCircle class="w-4 h-4 mr-2" /> กิจกรรมพิเศษ
			</Button>

			<Button variant="outline" onclick={handleExportPDF} disabled={isExporting}>
				{#if isExporting}
					<Loader2 class="w-4 h-4 mr-2 animate-spin" />
				{:else}
					<Download class="w-4 h-4 mr-2" />
				{/if}
				ดาวน์โหลด PDF
			</Button>
		</div>
	</div>

	<!-- Row 2: Filters -->
	<div class="flex items-center gap-2 flex-wrap">
		<div class="w-[180px]">
			<Select.Root type="single" bind:value={selectedYearId}>
				<Select.Trigger class="w-full h-9">
					{academicYears.find((y) => y.id === selectedYearId)?.name || 'เลือกปีการศึกษา'}
				</Select.Trigger>
				<Select.Content>
					{#each academicYears as year (year.id)}
						<Select.Item value={year.id}>{year.name}</Select.Item>
					{/each}
				</Select.Content>
			</Select.Root>
		</div>

		<div class="w-[180px]">
			<Select.Root type="single" bind:value={selectedSemesterId}>
				<Select.Trigger class="w-full h-9">
					{#if selectedSemesterId && semesters.find((s) => s.id === selectedSemesterId)}
						ภาคเรียนที่ {semesters.find((s) => s.id === selectedSemesterId)?.term}
					{:else}
						เลือกภาคเรียน
					{/if}
				</Select.Trigger>
				<Select.Content>
					{#each semesters as term (term.id)}
						<Select.Item value={term.id}>ภาคเรียนที่ {term.term}</Select.Item>
					{/each}
				</Select.Content>
			</Select.Root>
		</div>

		{#if viewMode === 'CLASSROOM'}
			<div class="w-[220px]">
				<Popover.Root bind:open={classroomPickerOpen}>
					<Popover.Trigger class="w-full">
						<Button
							variant="outline"
							role="combobox"
							aria-expanded={classroomPickerOpen}
							class="w-full justify-between font-normal h-9"
						>
							<span class="truncate">
								{classrooms.find((c) => c.id === selectedClassroomId)?.name || 'เลือกห้องเรียน'}
							</span>
							<ChevronsUpDown class="ml-2 h-4 w-4 shrink-0 opacity-50" />
						</Button>
					</Popover.Trigger>
					<Popover.Content class="w-[--bits-popover-trigger-width] p-0">
						<Command.Root>
							<Command.Input placeholder="ค้นหาห้อง..." />
							<Command.Empty>ไม่พบห้องเรียน</Command.Empty>
							<Command.Group class="max-h-[280px] overflow-y-auto">
								{#each classrooms as classroom (classroom.id)}
									<Command.Item
										value={classroom.name}
										onSelect={() => {
											selectedClassroomId = classroom.id;
											classroomPickerOpen = false;
										}}
									>
										<Check
											class="mr-2 h-4 w-4 {selectedClassroomId === classroom.id
												? 'opacity-100'
												: 'opacity-0'}"
										/>
										{classroom.name}
									</Command.Item>
								{/each}
							</Command.Group>
						</Command.Root>
					</Popover.Content>
				</Popover.Root>
			</div>
		{:else}
			<div class="w-[220px]">
				<Popover.Root bind:open={instructorPickerOpen}>
					<Popover.Trigger class="w-full">
						<Button
							variant="outline"
							role="combobox"
							aria-expanded={instructorPickerOpen}
							class="w-full justify-between font-normal h-9"
						>
							<span class="truncate">
								{instructors.find((i) => i.id === selectedInstructorId)?.name || 'เลือกครูผู้สอน'}
							</span>
							<ChevronsUpDown class="ml-2 h-4 w-4 shrink-0 opacity-50" />
						</Button>
					</Popover.Trigger>
					<Popover.Content class="w-[--bits-popover-trigger-width] p-0">
						<Command.Root>
							<Command.Input placeholder="ค้นหาครู..." />
							<Command.Empty>ไม่พบครู</Command.Empty>
							<Command.Group class="max-h-[280px] overflow-y-auto">
								{#each instructors as instructor (instructor.id)}
									<Command.Item
										value={instructor.name}
										onSelect={() => {
											selectedInstructorId = instructor.id;
											instructorPickerOpen = false;
										}}
									>
										<Check
											class="mr-2 h-4 w-4 {selectedInstructorId === instructor.id
												? 'opacity-100'
												: 'opacity-0'}"
										/>
										{instructor.name}
									</Command.Item>
								{/each}
							</Command.Group>
						</Command.Root>
					</Popover.Content>
				</Popover.Root>
			</div>
			{#if selectedInstructorId}
				<label
					class="flex items-center gap-2 text-xs cursor-pointer select-none px-2 py-1 rounded border bg-muted/30 hover:bg-muted transition-colors"
				>
					<input type="checkbox" bind:checked={showTeamGhosts} class="cursor-pointer" />
					<span>แสดงคาบในทีม (ghost cells)</span>
				</label>
			{/if}
		{/if}
	</div>

	<!-- Main Content Grid (Workspace = cursor canvas) -->
	<div
		class="grid grid-cols-12 gap-3 flex-1 min-h-0 relative"
		bind:this={workspaceRef}
		onmousemove={handleMouseMove}
		ondrag={handleDragMoveOnGrid}
		role="application"
	>
		<!-- Left Sidebar: Courses -->
		<Card.Root class="col-span-2 flex flex-col h-full overflow-hidden gap-0 py-0">
			<div class="py-2 px-3 border-b shrink-0">
				<div class="text-sm font-semibold flex items-center gap-2">
					<BookOpen class="w-4 h-4" /> รายวิชา
					<span class="text-[10px] font-normal text-muted-foreground ml-auto"> ลากไปวาง </span>
				</div>
			</div>
			<div class="flex-1 overflow-y-auto p-2 space-y-2 bg-muted/20">
				{#each unscheduledCourses as course (course.id)}
					{@const lockedBy = getDragOwner(undefined, course.id)}
					<div
						class="border rounded-md p-2 shadow-sm cursor-grab active:cursor-grabbing hover:shadow-md hover:brightness-95 transition-all group relative {lockedBy
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

						<div class="flex justify-between items-start mb-0.5 gap-1">
							<Badge variant="outline" class="text-[10px] px-1 py-0 leading-tight"
								>{course.subject_code}</Badge
							>
							<Badge
								variant={course.is_completed ? 'secondary' : 'default'}
								class="text-[10px] px-1 py-0 leading-tight"
							>
								{course.scheduled_count}/{course.max_periods}
							</Badge>
						</div>
						<h4 class="font-medium text-xs line-clamp-2 leading-snug mb-1">
							{course.subject_name_th || 'ไม่มีชื่อวิชา'}
						</h4>
						<div class="flex flex-col gap-0.5 text-[10px] text-muted-foreground">
							{#if viewMode === 'CLASSROOM'}
								<div class="flex items-center gap-1 truncate">
									<Users class="w-3 h-3 shrink-0" />
									<span class="truncate">{course.instructor_name || 'ไม่ระบุครู'}</span>
								</div>
							{:else}
								<div class="flex items-center gap-1 truncate">
									<School class="w-3 h-3 shrink-0" />
									<span class="truncate">{course.classroom_name || 'ไม่ระบุห้อง'}</span>
								</div>
							{/if}
							<div>{course.subject_credit} นก.</div>
						</div>

						<!-- Progress Bar -->
						<div class="mt-1.5 h-1 w-full bg-secondary rounded-full overflow-hidden">
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
						{#each unscheduledActivities as activity (activity.id + ':' + (activity._classroom_id ?? ''))}
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
										<Badge
											variant="outline"
											class="text-[10px] border-emerald-300 text-emerald-700"
										>
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
									<h4
										class="font-medium text-sm line-clamp-1 leading-tight flex items-center gap-1"
									>
										<Lock class="w-3 h-3 shrink-0" />
										{activity.name}
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
												} catch (e: unknown) {
													toast.error((e instanceof Error ? e.message : String(e)) || 'ไม่สำเร็จ');
												}
											}}
										>
											แสดงในตาราง
										</Button>
									{:else}
										<div class="text-[10px] text-muted-foreground mt-1">
											จัดพร้อมกัน — ใช้ Batch
										</div>
									{/if}
								</div>
							{/if}
						{/each}
					</div>
				</div>
			{/if}
		</Card.Root>

		<!-- Right Content: Timetable Grid -->
		<Card.Root
			class="col-span-10 flex flex-col h-full overflow-hidden border-2 shadow-none gap-0 py-0"
		>
			<div class="overflow-auto flex-1">
				<div class="min-w-[800px] h-full flex flex-col">
					<!-- Header Row (Periods) -->
					<div class="flex sticky top-0 bg-background z-20">
						<div
							class="w-20 shrink-0 p-3 border-r border-b font-medium text-sm text-muted-foreground flex items-center justify-center bg-background sticky left-0 z-30"
						>
							วัน/คาบ
						</div>
						{#each periods as period (period.id)}
							<div class="flex-1 min-w-[100px] p-2 border-r border-b text-center relative group">
								<div class="text-sm font-bold">{period.name || ' '}</div>
								<div class="text-xs text-muted-foreground">
									{formatTime(period.start_time)}-{formatTime(period.end_time)}
								</div>
							</div>
						{/each}
					</div>

					<!-- Days Rows -->
					{#each DAYS as day (day.value)}
						<div class="flex flex-1 min-h-[70px]">
							<!-- Day Header -->
							<div
								class="w-20 shrink-0 border-r border-b bg-background font-medium flex items-center justify-center relative sticky left-0 z-10"
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
							{#each periods as period (period.id)}
								{@const entry = getEntryForSlot(day.value, period.id)}
								{@const isOccupied = isSlotOccupiedByInstructor(day.value, period.id)}
								{@const lockedBy = entry ? getDragOwner(entry.id) : null}
								{@const remoteDrag = !entry ? getRemoteDragHover(day.value, period.id) : null}
								{@const remoteActivitySlot = getRemoteActivityForSlot(day.value, period.id)}
								{@const remoteActivityEntry = entry ? getRemoteActivityForEntry(entry.id) : null}
								{@const remoteActivity = remoteActivitySlot || remoteActivityEntry}

								<!-- Drop Zone -->
								{@const validity =
									draggedCourse && dragType === 'MOVE'
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
										? 'bg-green-50/50 ring-1 ring-inset ring-green-400/60'
										: ''} {validityClass} {draggedCourse &&
									entry &&
									isOccupied &&
									dragType === 'NEW'
										? 'ring-2 ring-inset ring-red-500/70'
										: ''} {remoteDrag ? 'ring-2 ring-inset ring-opacity-50' : ''}"
									style={remoteDrag
										? `--tw-ring-color: ${remoteDrag.user.color}40; background-color: ${remoteDrag.user.color}10;`
										: ''}
									data-day={day.value}
									data-period={period.id}
									title={validity && !validity.valid ? validity.reason : ''}
									ondragover={(e) => handleDragOver(e, day.value, period.id)}
									ondrop={(e) => handleDrop(e, day.value, period.id)}
									role="application"
								>
									{#if remoteDrag}
										<!-- Remote user drag ghost preview -->
										<div
											class="absolute inset-1 rounded border-2 border-dashed p-1.5 flex flex-col justify-center items-center gap-0.5 animate-in fade-in duration-200 pointer-events-none"
											style="border-color: {remoteDrag.user.color}80; background-color: {remoteDrag
												.user.color}15;"
										>
											<span
												class="text-[10px] font-bold truncate max-w-full"
												style="color: {remoteDrag.user.color}"
											>
												{remoteDrag.drag.info?.code || 'วิชา'}
											</span>
											<span class="text-[9px] text-muted-foreground truncate max-w-full">
												{remoteDrag.drag.info?.title || ''}
											</span>
											<span
												class="text-[8px] font-medium px-1.5 py-0.5 rounded-full text-white mt-0.5"
												style="background-color: {remoteDrag.user.color};"
											>
												{remoteDrag.user.name}
											</span>
										</div>
									{/if}
									{#if entry && validity && validity.state === 'occupied' && validity.valid}
										<!-- Swap indicator overlay -->
										<div
											class="absolute top-0.5 right-0.5 z-10 bg-blue-500 text-white text-[9px] px-1 py-0.5 rounded font-bold pointer-events-none"
										>
											⇄ สลับ
										</div>
									{/if}
									{#if remoteActivity}
										<!-- Remote user dialog activity — ring lock + badge (ช่วยเห็นเมื่อ cursor ไม่อยู่ใน view) -->
										<div
											class="absolute inset-0 ring-2 ring-inset pointer-events-none z-[5] rounded"
											style="--tw-ring-color: {remoteActivity.user
												.color}; background-color: {remoteActivity.user.color}1a;"
										></div>
										<div
											class="absolute top-0.5 left-0.5 right-0.5 z-20 flex items-center gap-1 px-1 py-0.5 rounded text-[9px] font-medium shadow-sm pointer-events-none"
											style="background-color: {remoteActivity.user.color}; color: white;"
										>
											<span>⚡</span>
											<span class="truncate"
												>{remoteActivity.user.name}: {activityLabel(remoteActivity.activity)}</span
											>
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
										{@const isRemoteLocked = !!remoteActivityEntry}
										{@const teacherText =
											entry.instructor_names && entry.instructor_names.length > 0
												? entry.instructor_names.join(', ')
												: entry.instructor_name && entry.instructor_name !== '-'
													? entry.instructor_name
													: ''}
										{@const hasMetaRow =
											viewMode === 'CLASSROOM'
												? !!teacherText || !!entry.room_id
												: !!entry.classroom_name ||
													!!entry.activity_slot_id ||
													isGhost ||
													coTeacherCount > 0 ||
													!!entry.room_id}
										<!-- Timetable Entry Card -->
										<div
											class="absolute inset-0.5 border rounded px-1.5 py-1 text-xs flex flex-col justify-between shadow-sm hover:shadow-md hover:brightness-95 transition-all group {(entry.entry_type !==
												'COURSE' &&
												!(
													entry.entry_type === 'ACTIVITY' &&
													entry.activity_scheduling_mode === 'independent'
												)) ||
											isGhost ||
											isRemoteLocked
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
											draggable={!lockedBy &&
												!isRemoteLocked &&
												!isGhost &&
												!pendingEntryIds.has(entry.id) &&
												!entry.id.startsWith('temp-') &&
												(entry.entry_type === 'COURSE' ||
													(entry.entry_type === 'ACTIVITY' &&
														entry.activity_scheduling_mode === 'independent' &&
														!entry.batch_id))}
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

											{#if entry.subject_code}
												<div class="font-bold text-foreground/90 truncate text-sm leading-tight">
													{entry.subject_code}
												</div>
												<div
													class="line-clamp-1 text-foreground/70 text-[11px] leading-tight mb-auto"
													title={entry.subject_name_th || undefined}
												>
													{entry.subject_name_th || ''}
												</div>
											{:else}
												<!-- TEXT-batch / activity: full title (รองรับหลายบรรทัดจาก textarea) -->
												<div
													class="font-bold text-foreground/90 text-sm leading-tight whitespace-pre-line line-clamp-3 mb-auto"
													title={entry.title || undefined}
												>
													{entry.title || getEntryTypeFallbackLabel(entry.entry_type)}
												</div>
											{/if}
											{#if hasMetaRow}
												<div
													class="mt-1 pt-1 border-t border-foreground/15 gap-0.5 flex flex-col text-[10px] text-muted-foreground"
												>
													{#if viewMode === 'CLASSROOM'}
														{#if teacherText}
															<div class="flex items-center gap-1 truncate">
																<Users class="w-3 h-3 shrink-0" />
																{teacherText}
															</div>
														{/if}
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
													{:else if entry.classroom_name}
														<div class="flex items-center gap-1 truncate">
															<School class="w-3 h-3 shrink-0" />
															{entry.classroom_name}
														</div>
													{/if}

													{#if viewMode === 'INSTRUCTOR' && isGhost}
														<div class="flex items-center gap-1 text-amber-700 text-[10px]">
															<span>👻</span>
															<span>อยู่ในทีม (ยังไม่ได้สอนคาบนี้)</span>
														</div>
													{:else if viewMode === 'INSTRUCTOR' && coTeacherCount > 0}
														<div
															class="flex items-center gap-1 text-foreground/60 text-[10px]"
															title={entry.instructor_names?.join(', ')}
														>
															<Users class="w-3 h-3 shrink-0" />
															<span>+{coTeacherCount} ครูร่วม</span>
														</div>
													{/if}

													{#if entry.room_id}
														<div
															class="flex items-center gap-1 truncate text-foreground/60"
															title={rooms.find((r) => r.id === entry.room_id)?.name_th}
														>
															<MapPin class="w-3 h-3 shrink-0" />
															{rooms.find((r) => r.id === entry.room_id)?.name_th || '?'}
														</div>
													{/if}
												</div>
											{/if}

											<!-- Delete Button (ghost cells + remote-locked ไม่แสดง — ไม่ใช่คาบที่แก้ได้) -->
											{#if !isGhost && !isRemoteLocked}
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
											<!-- Phase 2: pending spinner — drop ยังไม่ confirm จาก DB -->
											{#if pendingEntryIds.has(entry.id)}
												<div
													class="absolute top-0.5 left-0.5 z-30 p-0.5 rounded bg-amber-50/80"
													title="กำลังบันทึก..."
												>
													<Loader2 class="w-3 h-3 animate-spin text-amber-600" />
												</div>
											{/if}
										</div>
									{:else if isOccupied}
										{@const conflicts = slotConflicts.get(getSlotKey(day.value, period.id)) ?? []}
										{@const primary = conflicts[0]}
										<div
											class="absolute inset-0 flex flex-col items-center justify-center px-1 py-0.5 text-center select-none gap-0.5"
											title={conflicts
												.map((c) => {
													const subj = [c.subject_code, c.subject_name].filter(Boolean).join(' · ');
													const loc = [c.classroom_name, c.room_code ? `ห้อง ${c.room_code}` : '']
														.filter(Boolean)
														.join(' ');
													return c.kind === 'classroom'
														? `ห้องติด: ${subj}${loc ? ' (' + loc + ')' : ''}`
														: `${c.teacher_name} ติด: ${subj}${loc ? ' (' + loc + ')' : ''}`;
												})
												.join('\n')}
										>
											{#if primary}
												<div
													class="flex items-center gap-1 text-[11px] text-red-600 font-semibold truncate max-w-full leading-tight"
												>
													{#if primary.kind === 'classroom'}
														<BookOpen class="w-3 h-3 shrink-0" />
														<span class="truncate">{primary.subject_code || 'ไม่ว่าง'}</span>
													{:else}
														<Users class="w-3 h-3 shrink-0" />
														<span class="truncate">{primary.teacher_name}</span>
													{/if}
												</div>
												{#if primary.subject_name}
													<div
														class="text-[10px] text-red-500/80 truncate max-w-full leading-tight"
													>
														{primary.subject_name}
													</div>
												{/if}
												{#if primary.kind === 'teacher' || primary.room_code}
													<div class="text-[9px] text-red-500/70 truncate max-w-full leading-tight">
														{#if primary.kind === 'teacher'}
															{primary.classroom_name}{#if primary.room_code}
																· ห้อง {primary.room_code}{/if}
														{:else}
															ห้อง {primary.room_code}
														{/if}
													</div>
												{/if}
												{#if conflicts.length > 1}
													<div class="text-[9px] text-red-400 leading-none">
														+{conflicts.length - 1} ติดเพิ่ม
													</div>
												{/if}
											{:else}
												<div class="text-xs text-red-500 font-medium">
													{viewMode === 'INSTRUCTOR' ? 'ห้องนี้ไม่ว่าง' : 'ครูติดสอน'}
												</div>
											{/if}
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

	<!-- Floating Conflict Popup (ระหว่าง drag — native title ใช้ไม่ได้ใน drag state) -->
	{#if hoverDragCell && draggedCourse}
		{@const hoverConflicts =
			slotConflicts.get(getSlotKey(hoverDragCell.day, hoverDragCell.periodId)) ?? []}
		{#if hoverConflicts.length > 0}
			<div
				class="fixed z-[10000] pointer-events-none bg-white border border-red-300 rounded-md shadow-lg p-2 text-xs max-w-xs space-y-1"
				style="top: {hoverDragCell.y + 45}px; left: {hoverDragCell.x + 68}px;"
			>
				{#each hoverConflicts as c, i (i)}
					<div class="flex items-start gap-1.5 text-red-700">
						{#if c.kind === 'classroom'}
							<BookOpen class="w-3.5 h-3.5 shrink-0 mt-0.5" />
							<div class="flex-1 leading-tight">
								<div class="font-semibold">ห้องติด: {c.subject_code}</div>
								{#if c.subject_name}
									<div class="text-red-600/80">{c.subject_name}</div>
								{/if}
								{#if c.classroom_name || c.room_code}
									<div class="text-red-500/70 text-[10px]">
										{c.classroom_name}{#if c.room_code}
											· ห้อง {c.room_code}{/if}
									</div>
								{/if}
							</div>
						{:else}
							<Users class="w-3.5 h-3.5 shrink-0 mt-0.5" />
							<div class="flex-1 leading-tight">
								<div class="font-semibold">{c.teacher_name} ติด: {c.subject_code}</div>
								{#if c.subject_name}
									<div class="text-red-600/80">{c.subject_name}</div>
								{/if}
								{#if c.classroom_name || c.room_code}
									<div class="text-red-500/70 text-[10px]">
										{c.classroom_name}{#if c.room_code}
											· ห้อง {c.room_code}{/if}
									</div>
								{/if}
							</div>
						{/if}
					</div>
				{/each}
			</div>
		{/if}
	{/if}

	<!-- GHOST UI OVERLAY (fixed, clipped to workspace via clip-path) -->
	<div
		class="pointer-events-none fixed inset-0 z-[9999]"
		style={wsRect
			? `clip-path: inset(${wsRect.top}px ${typeof window !== 'undefined' ? window.innerWidth - wsRect.right : 0}px ${typeof window !== 'undefined' ? window.innerHeight - wsRect.bottom : 0}px ${wsRect.left}px)`
			: 'display:none'}
	>
		{#each $activeUsers as user (user.user_id)}
			{@const cursor = $remoteCursors[user.user_id]}

			{#if cursor && user.user_id !== $authStore.user?.id}
				{#if cursor.context?.view_mode === viewMode && cursor.context?.view_id === (viewMode === 'CLASSROOM' ? selectedClassroomId : selectedInstructorId)}
					<div
						class="absolute transition-transform duration-100 ease-linear flex flex-col items-start gap-1"
						style="transform: translate({cursor.x * (wsRect?.width ?? 0) +
							(wsRect?.left ?? 0)}px, {cursor.y * (wsRect?.height ?? 0) + (wsRect?.top ?? 0)}px);"
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

						{#if $remoteActivities[user.user_id]}
							{@const act = $remoteActivities[user.user_id]}
							<div
								class="px-2 py-0.5 rounded text-[10px] text-white whitespace-nowrap shadow-sm flex items-center gap-1 mt-0.5"
								style="background-color: {user.color}dd"
							>
								<span>⚡</span>
								<span>{activityLabel(act)}</span>
							</div>
						{/if}

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
					แก้ไขคาบนี้
				{/if}
			</Dialog.Title>
			<Dialog.Description>
				{#if entryPopoverTarget}
					{entryPopoverTarget.classroom_name} · {entryPopoverTarget.day_of_week} · {entryPopoverTarget.period_name ??
						''}
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
							{#each popoverInCell as uid, idx (uid)}
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
							{#each popoverNotInCell as t (t.instructor_id)}
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

				<!-- ห้องเรียน — ครูร่วมที่ใช้ห้องต่างจากครูหลัก: เปลี่ยนเฉพาะคาบนี้ได้
			      เฉพาะมุมมองครู (CLASSROOM view ไม่ใช่ use case) + ไม่ใช่ ghost (ไม่ใช่ entry ของเรา) -->
				{#if viewMode === 'INSTRUCTOR' && !entryPopoverIsGhost}
				<div class="space-y-2 border-t pt-3">
					<div class="text-sm font-medium">ห้องเรียน</div>
					<Popover.Root bind:open={entryPopoverRoomPickerOpen}>
						<Popover.Trigger class="w-full">
							<Button
								variant="outline"
								role="combobox"
								aria-expanded={entryPopoverRoomPickerOpen}
								class="w-full justify-between font-normal"
								disabled={entryPopoverSavingRoom}
							>
								<span class="truncate flex items-center gap-1.5">
									{#if entryPopoverSavingRoom}
										<Loader2 class="h-3.5 w-3.5 animate-spin" />
									{:else}
										<MapPin class="h-3.5 w-3.5 shrink-0 opacity-70" />
									{/if}
									{#if entryPopoverTarget?.room_id}
										{@const r = rooms.find((x) => x.id === entryPopoverTarget?.room_id)}
										{r ? `${r.name_th}${r.building_name ? ` (${r.building_name})` : ''}` : 'ห้องไม่พบ'}
									{:else}
										ไม่ระบุห้อง
									{/if}
								</span>
								<ChevronsUpDown class="ml-2 h-4 w-4 shrink-0 opacity-50" />
							</Button>
						</Popover.Trigger>
						<Popover.Content class="w-[--bits-popover-trigger-width] p-0">
							<Command.Root>
								<Command.Input placeholder="ค้นหาห้อง..." />
								<Command.Empty>ไม่พบห้อง</Command.Empty>
								<Command.Group class="max-h-[280px] overflow-y-auto">
									{#each rooms as room (room.id)}
										{@const isBusy = entryPopoverUnavailableRooms.has(room.id)}
										{@const isSelected = entryPopoverTarget?.room_id === room.id}
										{#if !isBusy || isSelected}
											<Command.Item
												value={`${room.name_th} ${room.building_name ?? ''}`}
												onSelect={() => handlePopoverChangeRoom(room.id)}
											>
												<Check
													class="mr-2 h-4 w-4 {isSelected ? 'opacity-100' : 'opacity-0'}"
												/>
												{room.name_th}{room.building_name ? ` (${room.building_name})` : ''}
											</Command.Item>
										{/if}
									{/each}
								</Command.Group>
							</Command.Root>
						</Popover.Content>
					</Popover.Root>
				</div>
				{/if}
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
				<Popover.Root bind:open={roomPickerOpen}>
					<Popover.Trigger class="w-full">
						<Button
							variant="outline"
							role="combobox"
							aria-expanded={roomPickerOpen}
							class="w-full justify-between font-normal"
						>
							<span class="truncate">
								{#if selectedRoomId === 'none'}
									ไม่ระบุห้อง
								{:else if selectedRoomId}
									{@const r = rooms.find((x) => x.id === selectedRoomId)}
									{r ? `${r.name_th} (${r.building_name})` : 'เลือกห้อง'}
								{:else}
									เลือกห้อง
								{/if}
							</span>
							<ChevronsUpDown class="ml-2 h-4 w-4 shrink-0 opacity-50" />
						</Button>
					</Popover.Trigger>
					<Popover.Content class="w-[--bits-popover-trigger-width] p-0">
						<Command.Root>
							<Command.Input placeholder="ค้นหาห้อง..." />
							<Command.Empty>ไม่พบห้อง</Command.Empty>
							<Command.Group class="max-h-[280px] overflow-y-auto">
								<Command.Item
									value="ไม่ระบุห้อง"
									onSelect={() => {
										selectedRoomId = 'none';
										roomPickerOpen = false;
									}}
								>
									<Check
										class="mr-2 h-4 w-4 {selectedRoomId === 'none' ? 'opacity-100' : 'opacity-0'}"
									/>
									<span class="text-muted-foreground">ไม่ระบุห้อง</span>
								</Command.Item>
								{#each rooms as room (room.id)}
									{@const isBusy = unavailableRooms.has(room.id)}
									{@const displaySelected = selectedRoomId === room.id}
									{#if !isBusy || displaySelected}
										<Command.Item
											value={`${room.name_th} ${room.building_name ?? ''}`}
											onSelect={() => {
												selectedRoomId = room.id;
												roomPickerOpen = false;
											}}
										>
											<Check
												class="mr-2 h-4 w-4 {selectedRoomId === room.id
													? 'opacity-100'
													: 'opacity-0'}"
											/>
											{room.name_th} ({room.building_name})
										</Command.Item>
									{/if}
								{/each}
							</Command.Group>
						</Command.Root>
					</Popover.Content>
				</Popover.Root>
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
<!-- Delete Batch Group Dialog (entry ที่มาจาก /timetable/batch) -->
<Dialog.Root bind:open={showDeleteBatchDialog}>
	<Dialog.Content class="max-w-sm">
		<Dialog.Header>
			<Dialog.Title>
				{isInstructorSyncDelete ? 'ซ่อนตัวเองจากกิจกรรม' : 'ลบคาบที่สร้างจาก Batch'}
			</Dialog.Title>
			<Dialog.Description>
				<span class="font-medium text-foreground">{deleteBatchTarget?.title || 'กิจกรรม'}</span>
				<br />
				{#if isInstructorSyncDelete}
					กิจกรรมนี้สอนพร้อมกันหลายห้อง — ซ่อนครูแบบไหนดี? (ห้องอื่นยังเห็นกิจกรรมเหมือนเดิม)
				{:else}
					คาบนี้ถูกสร้างพร้อมกับคาบอื่นจาก Batch เดียวกัน — ลบแบบไหนดี?
				{/if}
			</Dialog.Description>
		</Dialog.Header>
		<div class="flex flex-col gap-2 py-2">
			<Button variant="outline" onclick={doDeleteBatchSingle}>
				{isInstructorSyncDelete ? 'ซ่อนเฉพาะคาบนี้' : 'ลบแค่คาบนี้'}
			</Button>
			<Button variant="destructive" onclick={doDeleteBatchGroup}>
				{isInstructorSyncDelete ? 'ซ่อนทั้งกิจกรรม' : 'ลบทั้งหมดที่สร้างพร้อมกัน'}
			</Button>
		</div>
		<Dialog.Footer>
			<Button
				variant="ghost"
				onclick={() => {
					showDeleteBatchDialog = false;
					deleteBatchTarget = null;
				}}>ยกเลิก</Button
			>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

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
			<Button
				variant="outline"
				onclick={() => {
					if (deleteActivityTarget) doDeleteEntry(deleteActivityTarget.id, false);
				}}
			>
				ลบเฉพาะห้องนี้
			</Button>
			<Button
				variant="destructive"
				onclick={() => {
					if (deleteActivityTarget) doDeleteEntry(deleteActivityTarget.id, true);
				}}
			>
				ลบทุกห้อง
			</Button>
		</div>
		<Dialog.Footer>
			<Button
				variant="ghost"
				onclick={() => {
					showDeleteActivityDialog = false;
				}}>ยกเลิก</Button
			>
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
					{#each classrooms as room (room.id)}
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
					{#each instructors as teacher (teacher.id)}
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
	<Dialog.Content class="sm:max-w-[600px] max-h-[90vh] flex flex-col overflow-hidden">
		<Dialog.Header>
			<Dialog.Title>เพิ่มกิจกรรมพิเศษ (Batch)</Dialog.Title>
			<Dialog.Description>
				เพิ่มกิจกรรมให้หลายห้องเรียนพร้อมกัน (เช่น กิจกรรมหน้าเสาธง, ประชุมระดับ)
			</Dialog.Description>
		</Dialog.Header>

		<div class="grid gap-3 py-3 overflow-y-auto flex-1 pr-2">
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
				<div class="grid grid-cols-4 items-start gap-4">
					<Label.Root class="text-right mt-2">ชื่อกิจกรรม</Label.Root>
					<div class="col-span-3">
						<textarea
							rows="2"
							class="flex w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50 resize-y min-h-[60px]"
							bind:value={batchTitle}
							placeholder="เช่น ประชุมระดับ, กิจกรรมพัฒนาผู้เรียน&#10;ขึ้นบรรทัดใหม่ได้ (Enter)"
						></textarea>
						<p class="text-[10px] text-muted-foreground mt-1">
							พิมพ์หลายบรรทัดได้ (เช่น "พักกลางวัน\nรับประทานอาหาร")
						</p>
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
									{#each activitySlots as slot (slot.id)}
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
							*Batch รองรับเฉพาะ <b>Synchronized</b> (ทุกห้องตรงกัน) — Independent ให้ลากทีละห้องจาก
							sidebar<br />
							นักเรียนกดดูกิจกรรมที่ตัวเองลงทะเบียนได้จากตารางเรียน
						</p>
					</div>
				</div>
			{/if}

			<div class="grid grid-cols-4 items-start gap-4">
				<Label.Root class="text-right mt-1">วัน ({batchDays.length})</Label.Root>
				<div class="col-span-3 flex flex-wrap gap-1.5">
					{#each DAYS.slice(0, 5) as day (day.value)}
						<label
							class="flex items-center gap-1.5 px-2.5 py-1 rounded border cursor-pointer text-sm transition-colors {batchDays.includes(
								day.value
							)
								? 'border-primary bg-primary/10 text-primary font-medium'
								: 'bg-background hover:bg-muted/50'}"
						>
							<input
								type="checkbox"
								checked={batchDays.includes(day.value)}
								onchange={() => {
									if (batchDays.includes(day.value)) {
										batchDays = batchDays.filter((d) => d !== day.value);
									} else {
										batchDays = [...batchDays, day.value];
									}
								}}
								class="rounded"
							/>
							<span>{day.label}</span>
						</label>
					{/each}
				</div>
			</div>

			<div class="grid grid-cols-4 items-start gap-4">
				<Label.Root class="text-right mt-1">คาบ ({batchPeriodIds.length})</Label.Root>
				<div
					class="col-span-3 border rounded-md max-h-[160px] overflow-y-auto p-2 bg-muted/20 grid grid-cols-2 gap-1.5"
				>
					{#each periods as period (period.id)}
						<label
							class="flex items-center gap-2 p-1.5 rounded border bg-background cursor-pointer hover:bg-muted/50 text-sm {batchPeriodIds.includes(
								period.id
							)
								? 'border-primary bg-primary/5'
								: ''}"
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
								{period.name ? `${period.name} ` : ''}({formatTime(period.start_time)}-{formatTime(
									period.end_time
								)})
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
							{#each rooms as room (room.id)}
								<Select.Item value={room.id}>
									{room.name_th}
								</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
			</div>

			<!-- Targets Selection -->
			<div class="border-t pt-3 mt-1 space-y-3">
				{#if batchMode === 'SLOT' && batchSlotId}
					<!-- SLOT mode: ใช้ข้อมูลจาก slot ทั้งห้องและครู (read-only summary) -->
					<div class="rounded-md border border-emerald-200 bg-emerald-50/50 p-3 space-y-2">
						<p class="text-[11px] text-emerald-800">
							ใช้ข้อมูลจาก slot ตรงๆ — แก้จำนวนห้อง/ครูในหน้า
							<a
								href={resolve('/staff/academic/activities')}
								target="_blank"
								class="underline font-medium">Activities</a
							>
						</p>
						<div class="text-xs">
							<span class="font-semibold text-emerald-700">
								<School class="inline w-3 h-3 mb-0.5" /> ห้องเรียน ({batchClassrooms.length})
							</span>
							<div class="flex flex-wrap gap-1 mt-1">
								{#if batchClassrooms.length === 0}
									<span class="text-muted-foreground italic">
										slot นี้ยังไม่มีห้อง — เพิ่มในหน้า Activities ก่อน
									</span>
								{:else}
									{#each batchClassrooms as cid (cid)}
										{@const cr = classrooms.find((c) => c.id === cid)}
										<span class="bg-white border border-emerald-300 rounded px-1.5 py-0.5">
											{cr?.name || cid.slice(0, 8)}
										</span>
									{/each}
								{/if}
							</div>
						</div>
						<div class="text-xs">
							<span class="font-semibold text-emerald-700">
								<Users class="inline w-3 h-3 mb-0.5" /> ครู ({batchInstructors.length})
							</span>
							<div class="flex flex-wrap gap-1 mt-1">
								{#if batchInstructors.length === 0}
									<span class="text-muted-foreground italic">
										slot นี้ยังไม่มีครู — เพิ่มในหน้า Activities ก่อน
									</span>
								{:else}
									{#each batchInstructors as iid (iid)}
										{@const ins = instructors.find((i) => i.id === iid)}
										<span class="bg-white border border-emerald-300 rounded px-1.5 py-0.5">
											{ins?.name || iid.slice(0, 8)}
										</span>
									{/each}
								{/if}
							</div>
						</div>
					</div>
				{:else}
					<!-- TEXT mode: เลือก ห้อง / ครู อิสระ -->
					<p class="text-[11px] text-muted-foreground -mb-1">
						เลือก <b>ห้องเรียน</b> ให้กิจกรรมขึ้นในตารางนักเรียน / เลือก <b>ครู</b>
						ให้ขึ้นในตารางครู (เลือกทั้งคู่ได้ — event จะผูกครูทุกคนที่เลือกในทุกห้อง)
					</p>

					<!-- ห้องเรียน -->
					<div>
						<div class="flex items-center gap-2 mb-2">
							<Label.Root class="text-xs">กรองชั้น:</Label.Root>
							<select
								class="flex h-7 w-full items-center justify-between rounded-md border border-input bg-background px-2 py-0.5 text-xs ring-offset-background focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2"
								value={batchGradeFilterId}
								onchange={(e) => (batchGradeFilterId = e.currentTarget.value)}
								onmouseenter={loadBatchGradeLevels}
							>
								<option value="all">ทุกระดับ ({classrooms.length})</option>
								{#each batchGradeLevels as gl (gl.id)}
									<option value={gl.id}>{gl.name}</option>
								{/each}
							</select>
						</div>

						<div class="flex justify-between items-center mb-1">
							<Label.Root class="text-xs">ห้องเรียน ({batchClassrooms.length})</Label.Root>
							<Button
								variant="ghost"
								size="sm"
								class="h-6 text-xs"
								onclick={selectAllBatchClassrooms}
							>
								เลือกทั้งหมด
							</Button>
						</div>
						<div
							class="border rounded-md max-h-[140px] min-h-[60px] overflow-y-auto p-1.5 bg-muted/20 grid grid-cols-3 gap-1"
						>
							{#each filteredBatchClassroomsList as classroom (classroom.id)}
								<label
									class="flex items-center gap-1.5 bg-background px-1.5 py-1 rounded border shadow-sm text-xs cursor-pointer hover:bg-muted/50"
								>
									<Checkbox
										checked={batchClassrooms.includes(classroom.id)}
										onCheckedChange={() => toggleBatchClassroom(classroom.id)}
									/>
									<span class="truncate">{classroom.name}</span>
								</label>
							{:else}
								<div
									class="col-span-3 flex flex-col items-center justify-center text-muted-foreground py-2 opacity-70"
								>
									<School class="w-6 h-6 mb-1 opacity-20" />
									<span class="text-xs">ไม่พบห้องเรียน</span>
								</div>
							{/each}
						</div>
					</div>

					<!-- ครู -->
					<div>
						<div class="flex justify-between items-center mb-1">
							<Label.Root class="text-xs">ครู ({batchInstructors.length})</Label.Root>
							<Button
								variant="ghost"
								size="sm"
								class="h-6 text-xs"
								onclick={() => {
									if (batchInstructors.length === instructors.length) {
										batchInstructors = [];
									} else {
										batchInstructors = instructors.map((i) => i.id);
									}
								}}
							>
								{batchInstructors.length === instructors.length ? 'ล้าง' : 'เลือกทั้งหมด'}
							</Button>
						</div>
						<div
							class="border rounded-md max-h-[140px] min-h-[60px] overflow-y-auto p-1.5 bg-muted/20 grid grid-cols-2 gap-1"
						>
							{#each instructors as instructor (instructor.id)}
								<label
									class="flex items-center gap-1.5 bg-background px-1.5 py-1 rounded border shadow-sm text-xs cursor-pointer hover:bg-muted/50"
								>
									<Checkbox
										checked={batchInstructors.includes(instructor.id)}
										onCheckedChange={() => {
											if (batchInstructors.includes(instructor.id)) {
												batchInstructors = batchInstructors.filter((i) => i !== instructor.id);
											} else {
												batchInstructors = [...batchInstructors, instructor.id];
											}
										}}
									/>
									<span class="truncate">{instructor.name}</span>
								</label>
							{:else}
								<div
									class="col-span-2 flex flex-col items-center justify-center text-muted-foreground py-2 opacity-70"
								>
									<Users class="w-6 h-6 mb-1 opacity-20" />
									<span class="text-xs">ไม่พบครู</span>
								</div>
							{/each}
						</div>
					</div>
				{/if}
			</div>
			<div class="border-t pt-3 mt-1">
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
