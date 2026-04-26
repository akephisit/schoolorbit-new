<script lang="ts">
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';
	import {
		listInstructorConstraints,
		updateInstructorConstraints,
		reorderInstructorPriority,
		getSchoolSettings,
		updateSchoolSettings,
		listPeriods,
		listClassroomCourseConstraints,
		updateClassroomCourseConstraints,
		listCcPreferredRooms,
		setCcPreferredRooms,
		listAllRooms,
		autoScheduleTimetable,
		getSchedulingJob,
		undoSchedulingJob,
		type InstructorConstraintView,
		type ClassroomCourseConstraintView,
		type CcPreferredRoom,
		type RoomView,
		type Period,
		type TimeSlot,
		type SchedulingJobResponse,
		type FailedCourse
	} from '$lib/api/scheduling';
	import { getAcademicStructure, getSchoolDays, type AcademicYear, listClassrooms, type Semester } from '$lib/api/academic';
	import * as Card from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Badge } from '$lib/components/ui/badge';
	import { GripVertical, ChevronDown, ChevronRight, Sparkles, Save, LoaderCircle, Zap, AlertCircle, Undo2 } from 'lucide-svelte';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Select from '$lib/components/ui/select';

	let { data } = $props();

	let loading = $state(true);
	let saving = $state(false);
	let instructors = $state<InstructorConstraintView[]>([]);
	let periods = $state<Period[]>([]);
	let schoolDays = $state<{ value: string; label: string; shortLabel: string }[]>([]);
	let defaultMaxConsecutive = $state(4);
	let activeYear = $state<AcademicYear | null>(null);

	// Per-row UI state
	let expandedIds = $state(new Set<string>());
	let expandedCcIds = $state(new Set<string>()); // เปิดดู cc แต่ละตัว
	// Local edits — keyed by instructor_id, only flushed on Save
	let unavailableEdits = $state(new Map<string, TimeSlot[]>());
	// Per-instructor room (assigned_room_id) — server snapshot + local edit
	// '' = not assigned (clear), uuid = set
	let instructorRoomEdits = $state(new Map<string, string>());

	// Phase B: cc constraints — load lazily ต่อครู
	let ccByInstructor = $state(new Map<string, ClassroomCourseConstraintView[]>());
	let ccLoadingIds = $state(new Set<string>());
	// Local edits ของ cc — keyed by cc.id
	let ccUnavailableEdits = $state(new Map<string, TimeSlot[]>());
	let ccPatternEdits = $state(new Map<string, number[] | null>());
	let ccSameDayUniqueEdits = $state(new Map<string, boolean>());

	// Phase D: cc rooms — server state + local edits
	let allRooms = $state<RoomView[]>([]);
	let ccRoomsServer = $state(new Map<string, CcPreferredRoom[]>()); // server snapshot
	let ccRoomsEdits = $state(new Map<string, CcPreferredRoom[]>()); // local edits

	// Phase E: auto-schedule
	let semesters = $state<Semester[]>([]);
	let selectedSemesterId = $state('');
	let autoScheduling = $state(false);
	let currentJob = $state<SchedulingJobResponse | null>(null);
	let showResultDialog = $state(false);
	let pollAbort: ReturnType<typeof setTimeout> | null = null;

	// DnD state
	let draggedId = $state<string | null>(null);
	let priorityDirty = $state(false);

	function slotKey(day: string, periodId: string): string {
		return `${day}__${periodId}`;
	}

	function isUnavailable(instructorId: string, day: string, periodId: string): boolean {
		const slots = unavailableEdits.get(instructorId);
		if (!slots) return false;
		return slots.some((s) => s.day === day && s.period_id === periodId);
	}

	function toggleUnavailable(instructorId: string, day: string, periodId: string) {
		const current = unavailableEdits.get(instructorId) ?? [];
		const idx = current.findIndex((s) => s.day === day && s.period_id === periodId);
		const next = idx >= 0
			? current.filter((_, i) => i !== idx)
			: [...current, { day, period_id: periodId }];
		const newMap = new Map(unavailableEdits);
		newMap.set(instructorId, next);
		unavailableEdits = newMap;
	}

	async function loadAll() {
		loading = true;
		try {
			const struct = await getAcademicStructure();
			const yrs = struct.data.years;
			activeYear = yrs.find((y) => y.is_active) ?? yrs[0] ?? null;
			if (!activeYear) {
				toast.error('ไม่พบปีการศึกษาที่ใช้งานอยู่');
				return;
			}
			schoolDays = getSchoolDays(activeYear.school_days);

			// Phase E: load semesters เพื่อให้เลือก scope auto-schedule
			semesters = (struct.data.semesters ?? []).filter((s) => s.academic_year_id === activeYear!.id);
			const activeSem = semesters.find((s) => s.is_active) ?? semesters[0];
			if (activeSem) selectedSemesterId = activeSem.id;

			const [instrRes, periodsRes, settingsRes, roomsRes] = await Promise.all([
				listInstructorConstraints(),
				listPeriods(activeYear.id),
				getSchoolSettings(),
				listAllRooms()
			]);
			instructors = (instrRes.data ?? []).filter((i) => i.primary_course_count > 0);
			periods = (periodsRes.data ?? []).sort((a, b) => a.order_index - b.order_index);
			defaultMaxConsecutive = settingsRes.data?.default_max_consecutive ?? 4;
			allRooms = roomsRes.data ?? [];

			// Initialize edits from server state
			const init = new Map<string, TimeSlot[]>();
			const roomInit = new Map<string, string>();
			for (const i of instructors) {
				init.set(i.id, (i.hard_unavailable_slots ?? []) as TimeSlot[]);
				roomInit.set(i.id, i.assigned_room_id ?? '');
			}
			unavailableEdits = init;
			instructorRoomEdits = roomInit;
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'โหลดข้อมูลไม่สำเร็จ');
		} finally {
			loading = false;
		}
	}

	// =========================================
	// Drag & Drop priority
	// =========================================

	function onDragStart(e: DragEvent, id: string) {
		e.dataTransfer!.effectAllowed = 'move';
		draggedId = id;
	}

	function onDragOver(e: DragEvent) {
		e.preventDefault();
		e.dataTransfer!.dropEffect = 'move';
	}

	function onDragEnter(_e: DragEvent, targetId: string) {
		if (!draggedId || draggedId === targetId) return;
		const src = instructors.findIndex((i) => i.id === draggedId);
		const dst = instructors.findIndex((i) => i.id === targetId);
		if (src < 0 || dst < 0) return;
		const next = [...instructors];
		const [moved] = next.splice(src, 1);
		next.splice(dst, 0, moved);
		instructors = next;
		priorityDirty = true;
	}

	function onDragEnd() {
		draggedId = null;
	}

	// =========================================
	// Save
	// =========================================

	async function saveAll() {
		if (saving) return;
		saving = true;
		try {
			const ops: Promise<unknown>[] = [];

			// 1. Priority order — bulk endpoint (1 query batch)
			if (priorityDirty) {
				ops.push(reorderInstructorPriority(instructors.map((i) => i.id)));
			}

			// 2. Global settings
			ops.push(updateSchoolSettings({ default_max_consecutive: defaultMaxConsecutive }));

			// 3. Per-instructor unavailable + room — only ที่เปลี่ยนจริง
			for (const i of instructors) {
				const localUnavail = unavailableEdits.get(i.id) ?? [];
				const remoteUnavail = (i.hard_unavailable_slots ?? []) as TimeSlot[];
				const localRoom = instructorRoomEdits.get(i.id) ?? '';
				const remoteRoom = i.assigned_room_id ?? '';

				const unavailChanged = !slotsEqual(localUnavail, remoteUnavail);
				const roomChanged = localRoom !== remoteRoom;

				if (!unavailChanged && !roomChanged) continue;

				const req: Parameters<typeof updateInstructorConstraints>[1] = {};
				if (unavailChanged) req.hard_unavailable_slots = localUnavail;
				if (roomChanged) {
					if (localRoom === '') {
						req.clear_assigned_room = true;
					} else {
						req.assigned_room_id = localRoom;
					}
				}
				ops.push(updateInstructorConstraints(i.id, req));
			}

			// 4. Per-cc constraints — only ที่เปลี่ยนจริง
			for (const [_, ccList] of ccByInstructor.entries()) {
				for (const cc of ccList) {
					const localUnavail = ccUnavailableEdits.get(cc.id) ?? [];
					const localPattern = ccPatternEdits.get(cc.id) ?? null;
					const localSdu = ccSameDayUniqueEdits.get(cc.id);

					const unavailChanged = !slotsEqual(localUnavail, cc.hard_unavailable_slots ?? []);
					const patternChanged = !patternEquals(localPattern, cc.consecutive_pattern);
					const sduChanged = localSdu !== undefined && localSdu !== cc.same_day_unique;

					if (!unavailChanged && !patternChanged && !sduChanged) continue;

					ops.push(updateClassroomCourseConstraints(cc.id, {
						hard_unavailable_slots: unavailChanged ? localUnavail : undefined,
						consecutive_pattern: patternChanged ? localPattern : undefined,
						same_day_unique: sduChanged ? localSdu : undefined
					}));
				}
			}

			// 5. Phase D: cc rooms — only ที่เปลี่ยนจริง
			for (const [ccId, _] of ccRoomsEdits.entries()) {
				if (!ccRoomsChanged(ccId)) continue;
				const local = ccRoomsEdits.get(ccId) ?? [];
				ops.push(setCcPreferredRooms(ccId, {
					rooms: local.map((r) => ({
						room_id: r.room_id,
						rank: r.rank,
						is_required: r.is_required
					}))
				}));
			}

			await Promise.all(ops);
			toast.success('บันทึกการตั้งค่าสำเร็จ');
			priorityDirty = false;
			await loadAll();
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'บันทึกไม่สำเร็จ');
		} finally {
			saving = false;
		}
	}

	function slotsEqual(a: TimeSlot[], b: TimeSlot[]): boolean {
		if (a.length !== b.length) return false;
		const setA = new Set(a.map((s) => slotKey(s.day, s.period_id)));
		for (const s of b) if (!setA.has(slotKey(s.day, s.period_id))) return false;
		return true;
	}

	function toggleExpand(id: string) {
		const next = new Set(expandedIds);
		if (next.has(id)) {
			next.delete(id);
		} else {
			next.add(id);
			// Load cc list lazily ครั้งแรก
			if (!ccByInstructor.has(id)) {
				loadCcForInstructor(id);
			}
		}
		expandedIds = next;
	}

	async function loadCcForInstructor(instructorId: string) {
		const loading = new Set(ccLoadingIds);
		loading.add(instructorId);
		ccLoadingIds = loading;
		try {
			const res = await listClassroomCourseConstraints(instructorId);
			const list = res.data ?? [];
			const newMap = new Map(ccByInstructor);
			newMap.set(instructorId, list);
			ccByInstructor = newMap;

			// Init local edits จาก server state
			const unavail = new Map(ccUnavailableEdits);
			const pattern = new Map(ccPatternEdits);
			const sdu = new Map(ccSameDayUniqueEdits);
			for (const cc of list) {
				unavail.set(cc.id, cc.hard_unavailable_slots ?? []);
				pattern.set(cc.id, cc.consecutive_pattern ?? null);
				sdu.set(cc.id, cc.same_day_unique);
			}
			ccUnavailableEdits = unavail;
			ccPatternEdits = pattern;
			ccSameDayUniqueEdits = sdu;
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'โหลด classroom courses ไม่สำเร็จ');
		} finally {
			const stop = new Set(ccLoadingIds);
			stop.delete(instructorId);
			ccLoadingIds = stop;
		}
	}

	function toggleCcExpand(ccId: string) {
		const next = new Set(expandedCcIds);
		if (next.has(ccId)) {
			next.delete(ccId);
		} else {
			next.add(ccId);
			// Lazy load rooms ครั้งแรก
			if (!ccRoomsServer.has(ccId)) {
				loadCcRooms(ccId);
			}
		}
		expandedCcIds = next;
	}

	async function loadCcRooms(ccId: string) {
		try {
			const res = await listCcPreferredRooms(ccId);
			const list = res.data ?? [];
			const newServer = new Map(ccRoomsServer);
			const newEdits = new Map(ccRoomsEdits);
			newServer.set(ccId, list);
			newEdits.set(ccId, [...list]); // copy → editable
			ccRoomsServer = newServer;
			ccRoomsEdits = newEdits;
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'โหลดห้องไม่สำเร็จ');
		}
	}

	function ccRooms(ccId: string): CcPreferredRoom[] {
		return ccRoomsEdits.get(ccId) ?? [];
	}

	function addCcRoom(ccId: string, roomId: string) {
		const current = ccRooms(ccId);
		if (current.some((r) => r.room_id === roomId)) return;
		const room = allRooms.find((r) => r.id === roomId);
		if (!room) return;
		const next: CcPreferredRoom[] = [
			...current,
			{
				id: '',
				classroom_course_id: ccId,
				room_id: roomId,
				room_code: room.code,
				room_name: room.name_th,
				rank: current.length + 1,
				is_required: false
			}
		];
		const map = new Map(ccRoomsEdits);
		map.set(ccId, next);
		ccRoomsEdits = map;
	}

	function removeCcRoom(ccId: string, roomId: string) {
		const current = ccRooms(ccId).filter((r) => r.room_id !== roomId);
		// Recompute ranks
		current.forEach((r, i) => (r.rank = i + 1));
		const map = new Map(ccRoomsEdits);
		map.set(ccId, current);
		ccRoomsEdits = map;
	}

	function moveCcRoom(ccId: string, roomId: string, direction: -1 | 1) {
		const current = ccRooms(ccId);
		const idx = current.findIndex((r) => r.room_id === roomId);
		const newIdx = idx + direction;
		if (idx < 0 || newIdx < 0 || newIdx >= current.length) return;
		const next = [...current];
		const [moved] = next.splice(idx, 1);
		next.splice(newIdx, 0, moved);
		next.forEach((r, i) => (r.rank = i + 1));
		const map = new Map(ccRoomsEdits);
		map.set(ccId, next);
		ccRoomsEdits = map;
	}

	function toggleCcRoomRequired(ccId: string, roomId: string) {
		const current = ccRooms(ccId);
		const next = current.map((r) =>
			r.room_id === roomId ? { ...r, is_required: !r.is_required } : r
		);
		const map = new Map(ccRoomsEdits);
		map.set(ccId, next);
		ccRoomsEdits = map;
	}

	function ccRoomsChanged(ccId: string): boolean {
		const server = ccRoomsServer.get(ccId) ?? [];
		const local = ccRoomsEdits.get(ccId) ?? [];
		if (server.length !== local.length) return true;
		for (let i = 0; i < local.length; i++) {
			const s = server[i];
			const l = local[i];
			if (s.room_id !== l.room_id || s.rank !== l.rank || s.is_required !== l.is_required) {
				return true;
			}
		}
		return false;
	}

	// CC unavailable helpers — รวม union ของ team + local edits
	function ccIsUnavailable(cc: ClassroomCourseConstraintView, day: string, periodId: string): boolean {
		// Inherited จากครูใน team → readonly true
		if ((cc.team_unavailable_slots ?? []).some((s) => s.day === day && s.period_id === periodId)) {
			return true;
		}
		const local = ccUnavailableEdits.get(cc.id) ?? [];
		return local.some((s) => s.day === day && s.period_id === periodId);
	}

	function ccIsInheritedUnavailable(
		cc: ClassroomCourseConstraintView,
		day: string,
		periodId: string
	): boolean {
		return (cc.team_unavailable_slots ?? []).some(
			(s) => s.day === day && s.period_id === periodId
		);
	}

	function toggleCcUnavailable(cc: ClassroomCourseConstraintView, day: string, periodId: string) {
		// Inherited → ห้าม toggle
		if (ccIsInheritedUnavailable(cc, day, periodId)) return;
		const current = ccUnavailableEdits.get(cc.id) ?? [];
		const idx = current.findIndex((s) => s.day === day && s.period_id === periodId);
		const next = idx >= 0
			? current.filter((_, i) => i !== idx)
			: [...current, { day, period_id: periodId }];
		const newMap = new Map(ccUnavailableEdits);
		newMap.set(cc.id, next);
		ccUnavailableEdits = newMap;
	}

	function setCcPattern(ccId: string, pattern: number[] | null) {
		const next = new Map(ccPatternEdits);
		next.set(ccId, pattern);
		ccPatternEdits = next;
	}

	function setCcSameDayUnique(ccId: string, value: boolean) {
		const next = new Map(ccSameDayUniqueEdits);
		next.set(ccId, value);
		ccSameDayUniqueEdits = next;
	}

	// Generate pattern options ให้ periods_per_week
	// 3 → [[1,1,1], [2,1], [1,2], [3]]
	// 4 → [[1,1,1,1], [2,1,1], [1,2,1], [1,1,2], [2,2], [3,1], [1,3], [4]]
	// ใช้ recursive composition
	function patternOptions(periods: number): number[][] {
		if (periods <= 0) return [[]];
		if (periods > 6) return [[periods]]; // edge case — too many — only "all in one"
		const result: number[][] = [];
		const compose = (remaining: number, acc: number[]) => {
			if (remaining === 0) {
				result.push([...acc]);
				return;
			}
			for (let chunk = 1; chunk <= remaining; chunk++) {
				acc.push(chunk);
				compose(remaining - chunk, acc);
				acc.pop();
			}
		};
		compose(periods, []);
		return result;
	}

	function patternLabel(pattern: number[]): string {
		return pattern.join('+');
	}

	function patternEquals(a: number[] | null | undefined, b: number[] | null | undefined): boolean {
		if (!a && !b) return true;
		if (!a || !b) return false;
		if (a.length !== b.length) return false;
		return a.every((v, i) => v === b[i]);
	}

	function unavailableCount(id: string): number {
		return unavailableEdits.get(id)?.length ?? 0;
	}

	// =========================================
	// Phase E: Auto-schedule + result polling
	// =========================================

	async function runAutoSchedule() {
		if (!selectedSemesterId) {
			toast.error('กรุณาเลือกภาคเรียน');
			return;
		}
		// บันทึก config ก่อน → จัด
		await saveAll();
		if (saving) return; // saveAll ยัง running

		// Load classrooms ของปีการศึกษานี้
		try {
			autoScheduling = true;
			if (!activeYear) throw new Error('No active year');
			const crRes = await listClassrooms({ year_id: activeYear.id });
			const classroom_ids = (crRes.data ?? []).map((c) => c.id);
			if (classroom_ids.length === 0) {
				toast.error('ไม่มีห้องเรียนในภาคเรียนนี้');
				autoScheduling = false;
				return;
			}

			const jobRes = await autoScheduleTimetable({
				academic_semester_id: selectedSemesterId,
				classroom_ids,
				algorithm: 'BACKTRACKING',
				config: {
					force_overwrite: true,
					allow_partial: true,
					min_quality_score: 60,
					timeout_seconds: 120
				}
			});

			if (!jobRes.data?.job_id) {
				throw new Error('Backend ไม่ส่ง job_id กลับ');
			}
			toast.success('เริ่มจัดตารางอัตโนมัติแล้ว');
			pollJob(jobRes.data.job_id);
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'จัดอัตโนมัติไม่สำเร็จ');
			autoScheduling = false;
		}
	}

	let undoing = $state(false);

	async function handleUndo() {
		if (!currentJob || undoing) return;
		if (!window.confirm('Undo การจัดอัตโนมัติครั้งนี้? — จะลบ entries ที่ scheduler สร้าง')) return;
		undoing = true;
		try {
			const res = await undoSchedulingJob(currentJob.id);
			toast.success(`Undo สำเร็จ — ลบ ${res.data?.deleted ?? 0} entries`);
			showResultDialog = false;
			currentJob = null;
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'Undo ไม่สำเร็จ');
		} finally {
			undoing = false;
		}
	}

	function pollJob(jobId: string) {
		const check = async () => {
			try {
				const res = await getSchedulingJob(jobId);
				const job = res.data;
				if (!job) {
					autoScheduling = false;
					return;
				}
				currentJob = job;
				if (job.status === 'COMPLETED' || job.status === 'FAILED' || job.status === 'CANCELLED') {
					autoScheduling = false;
					showResultDialog = true;
					if (job.status === 'COMPLETED') {
						toast.success(`จัดสำเร็จ ${job.scheduled_courses}/${job.total_courses} วิชา`);
					} else {
						toast.error(job.error_message || `Status: ${job.status}`);
					}
				} else {
					// running — poll again
					pollAbort = setTimeout(check, 2000);
				}
			} catch (e) {
				toast.error(e instanceof Error ? e.message : 'ติดตามสถานะไม่สำเร็จ');
				autoScheduling = false;
			}
		};
		check();
	}

	onMount(loadAll);

	// Cleanup polling on unmount
	import { onDestroy } from 'svelte';
	onDestroy(() => {
		if (pollAbort) clearTimeout(pollAbort);
	});
</script>

<svelte:head>
	<title>{data.title}</title>
</svelte:head>

<div class="container mx-auto p-4 space-y-4">
	<div class="flex items-center justify-between">
		<div class="flex items-center gap-2">
			<Sparkles class="w-6 h-6 text-primary" />
			<h1 class="text-2xl font-bold">ตั้งค่าจัดตารางอัตโนมัติ</h1>
		</div>
		<div class="flex items-center gap-2">
			<Button variant="outline" onclick={saveAll} disabled={saving || loading || autoScheduling}>
				{#if saving}
					<LoaderCircle class="w-4 h-4 animate-spin mr-2" />
				{:else}
					<Save class="w-4 h-4 mr-2" />
				{/if}
				บันทึก
			</Button>
			<Button onclick={runAutoSchedule} disabled={saving || loading || autoScheduling || !selectedSemesterId}>
				{#if autoScheduling}
					<LoaderCircle class="w-4 h-4 animate-spin mr-2" />
				{:else}
					<Zap class="w-4 h-4 mr-2" />
				{/if}
				บันทึกและจัดอัตโนมัติ
			</Button>
		</div>
	</div>

	{#if loading}
		<div class="flex items-center justify-center py-20">
			<LoaderCircle class="w-8 h-8 animate-spin text-muted-foreground" />
		</div>
	{:else}
		<!-- Global settings -->
		<Card.Root class="p-4">
			<h2 class="font-semibold mb-3">ตั้งค่ารวม</h2>
			<div class="grid gap-3 md:grid-cols-2">
				<div class="flex items-center gap-3">
					<Label for="max-consec" class="shrink-0">ครูสอนติดสูงสุด:</Label>
					<Input
						id="max-consec"
						type="number"
						min="1"
						max="20"
						bind:value={defaultMaxConsecutive}
						class="w-24"
					/>
					<span class="text-sm text-muted-foreground">คาบติด</span>
				</div>
				<div class="flex items-center gap-3">
					<Label class="shrink-0">ภาคเรียน:</Label>
					<Select.Root type="single" bind:value={selectedSemesterId}>
						<Select.Trigger class="flex-1">
							{semesters.find((s) => s.id === selectedSemesterId)?.name || 'เลือกภาคเรียน'}
						</Select.Trigger>
						<Select.Content>
							{#each semesters as sem (sem.id)}
								<Select.Item value={sem.id}>{sem.name}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
			</div>
		</Card.Root>

		<!-- Instructor priority + constraints -->
		<Card.Root class="p-4">
			<div class="mb-3">
				<h2 class="font-semibold">ลำดับครู (ลากเพื่อจัดเรียง)</h2>
				<p class="text-sm text-muted-foreground">
					ครูที่อยู่บนสุด จะถูกจัดตารางก่อน — แสดงเฉพาะครูที่เป็น primary ของวิชา
					({instructors.length} คน)
				</p>
			</div>

			{#if instructors.length === 0}
				<p class="text-muted-foreground text-center py-8">
					ยังไม่มีครูที่เป็น primary instructor — เพิ่มได้ที่หน้า Course Planning
				</p>
			{:else}
				<div class="space-y-2">
					{#each instructors as instr, idx (instr.id)}
						<div
							draggable="true"
							ondragstart={(e) => onDragStart(e, instr.id)}
							ondragover={onDragOver}
							ondragenter={(e) => onDragEnter(e, instr.id)}
							ondragend={onDragEnd}
							role="listitem"
							class="border rounded-md bg-card transition-shadow {draggedId === instr.id ? 'opacity-40' : ''}"
						>
							<!-- Header row -->
							<div class="flex items-center gap-2 p-2">
								<GripVertical class="w-4 h-4 text-muted-foreground cursor-move shrink-0" />
								<Badge variant="secondary" class="shrink-0 w-10 justify-center">
									{idx + 1}
								</Badge>
								<button
									onclick={() => toggleExpand(instr.id)}
									class="flex items-center gap-2 flex-1 text-left hover:bg-accent rounded px-2 py-1"
								>
									{#if expandedIds.has(instr.id)}
										<ChevronDown class="w-4 h-4" />
									{:else}
										<ChevronRight class="w-4 h-4" />
									{/if}
									<span class="font-medium">{instr.first_name} {instr.last_name}</span>
									<span class="text-xs text-muted-foreground">
										({instr.primary_course_count} วิชา)
									</span>
									{#if unavailableCount(instr.id) > 0}
										<Badge variant="outline" class="ml-auto text-xs">
											ไม่ว่าง {unavailableCount(instr.id)} คาบ
										</Badge>
									{/if}
								</button>
							</div>

							<!-- Expanded content -->
							{#if expandedIds.has(instr.id)}
								<div class="border-t p-3 bg-muted/30 space-y-3">
									<!-- ห้องประจำของครู (instructor_room_assignments) — fallback room ของ scheduler -->
									<div>
										<Label class="text-sm font-medium">ห้องประจำของครู</Label>
										<select
											class="text-sm border rounded px-2 py-1 bg-background w-full mt-1"
											value={instructorRoomEdits.get(instr.id) ?? ''}
											onchange={(e) => {
												const next = new Map(instructorRoomEdits);
												next.set(instr.id, e.currentTarget.value);
												instructorRoomEdits = next;
											}}
										>
											<option value="">— ไม่กำหนด —</option>
											{#each allRooms as r (r.id)}
												<option value={r.id}>{r.code} — {r.name_th}</option>
											{/each}
										</select>
										<p class="text-xs text-muted-foreground mt-1">
											ห้องที่ครูคนนี้มักใช้สอน — scheduler จะใช้เป็น fallback
											ถ้าวิชาไม่ได้กำหนดห้องเฉพาะ
										</p>
									</div>

									<div>
										<h4 class="text-sm font-medium mb-2">คาบที่ไม่ว่าง</h4>
										<div class="overflow-x-auto">
											<table class="text-xs border-collapse">
												<thead>
													<tr>
														<th class="border p-1 bg-card sticky left-0 z-10">วัน</th>
														{#each periods as p (p.id)}
															<th class="border p-1 bg-card min-w-[60px]">
																{p.name || `คาบ ${p.order_index}`}
															</th>
														{/each}
													</tr>
												</thead>
												<tbody>
													{#each schoolDays as day (day.value)}
														<tr>
															<td class="border p-1 bg-card font-medium sticky left-0 z-10">
																{day.shortLabel}
															</td>
															{#each periods as p (p.id)}
																<td class="border p-0 text-center">
																	<button
																		onclick={() => toggleUnavailable(instr.id, day.value, p.id)}
																		class="w-full h-7 hover:bg-accent transition-colors {isUnavailable(
																			instr.id,
																			day.value,
																			p.id
																		)
																			? 'bg-destructive/80 hover:bg-destructive text-destructive-foreground'
																			: ''}"
																		aria-label={isUnavailable(instr.id, day.value, p.id)
																			? 'คลิกเพื่อตั้งเป็นว่าง'
																			: 'คลิกเพื่อตั้งเป็นไม่ว่าง'}
																	>
																		{isUnavailable(instr.id, day.value, p.id) ? '✕' : ''}
																	</button>
																</td>
															{/each}
														</tr>
													{/each}
												</tbody>
											</table>
										</div>
										<p class="text-xs text-muted-foreground mt-1">
											คลิกเพื่อ toggle "ไม่ว่าง" — คาบสีแดง = ครูจะไม่ถูกจัดในคาบนั้น
										</p>
									</div>

									<!-- Phase B: classroom_courses ที่ครูเป็น primary -->
									<div>
										<h4 class="text-sm font-medium mb-2">
											รายวิชาที่ครูคนนี้เป็นครูหลัก ({ccByInstructor.get(instr.id)?.length ?? 0})
										</h4>
										{#if ccLoadingIds.has(instr.id)}
											<div class="flex items-center gap-2 text-sm text-muted-foreground">
												<LoaderCircle class="w-4 h-4 animate-spin" /> กำลังโหลด...
											</div>
										{:else if (ccByInstructor.get(instr.id)?.length ?? 0) === 0}
											<p class="text-sm text-muted-foreground">ไม่มีวิชาที่เป็นครูหลัก</p>
										{:else}
											<div class="space-y-2">
												{#each ccByInstructor.get(instr.id) ?? [] as cc (cc.id)}
													{@const ccPpw = cc.periods_per_week ?? 1}
													{@const opts = patternOptions(ccPpw)}
													{@const currentPattern = ccPatternEdits.get(cc.id) ?? null}
													{@const currentSdu = ccSameDayUniqueEdits.get(cc.id) ?? cc.same_day_unique}
													<div class="border rounded-md bg-card">
														<button
															onclick={() => toggleCcExpand(cc.id)}
															class="w-full flex items-center gap-2 p-2 hover:bg-accent text-left text-sm"
														>
															{#if expandedCcIds.has(cc.id)}
																<ChevronDown class="w-3 h-3" />
															{:else}
																<ChevronRight class="w-3 h-3" />
															{/if}
															<span class="font-medium">{cc.subject_code}</span>
															<span class="text-muted-foreground">
																{cc.subject_name} — {cc.classroom_name}
															</span>
															<span class="ml-auto text-xs text-muted-foreground">
																{ccPpw} คาบ/สัปดาห์
															</span>
														</button>
														{#if expandedCcIds.has(cc.id)}
															<div class="border-t p-3 space-y-3">
																<!-- Pattern picker -->
																<div>
																	<Label class="text-xs">รูปแบบการจัดคาบ</Label>
																	<div class="flex flex-wrap gap-1 mt-1">
																		<button
																			onclick={() => setCcPattern(cc.id, null)}
																			class="text-xs px-2 py-1 rounded border {currentPattern === null
																				? 'bg-primary text-primary-foreground'
																				: 'bg-background hover:bg-accent'}"
																		>
																			Auto (default)
																		</button>
																		{#each opts as opt (patternLabel(opt))}
																			<button
																				onclick={() => setCcPattern(cc.id, opt)}
																				class="text-xs px-2 py-1 rounded border {patternEquals(currentPattern, opt)
																					? 'bg-primary text-primary-foreground'
																					: 'bg-background hover:bg-accent'}"
																			>
																				{patternLabel(opt)}
																			</button>
																		{/each}
																	</div>
																</div>

																<!-- Same day unique -->
																<label class="flex items-center gap-2 text-xs cursor-pointer">
																	<input
																		type="checkbox"
																		checked={currentSdu}
																		onchange={(e) => setCcSameDayUnique(cc.id, e.currentTarget.checked)}
																	/>
																	<span>ห้ามวันเดียวกันมีรหัสวิชาซ้ำ</span>
																</label>

																<!-- Phase D: CC preferred rooms -->
																<div>
																	<Label class="text-xs">ห้องที่ใช้สอน (เรียงตามลำดับ)</Label>
																	<div class="space-y-1 mt-1">
																		{#each ccRooms(cc.id) as r (r.room_id)}
																			<div class="flex items-center gap-2 border rounded px-2 py-1 bg-card">
																				<span class="text-xs text-muted-foreground w-5">{r.rank}.</span>
																				<span class="text-xs flex-1">
																					<span class="font-medium">{r.room_code}</span>
																					<span class="text-muted-foreground"> — {r.room_name}</span>
																				</span>
																				<label class="text-xs flex items-center gap-1">
																					<input
																						type="checkbox"
																						checked={r.is_required}
																						onchange={() => toggleCcRoomRequired(cc.id, r.room_id)}
																					/>
																					บังคับ
																				</label>
																				<button
																					class="text-xs px-1 hover:bg-accent rounded"
																					onclick={() => moveCcRoom(cc.id, r.room_id, -1)}
																					disabled={r.rank === 1}
																					aria-label="เลื่อนขึ้น"
																				>
																					↑
																				</button>
																				<button
																					class="text-xs px-1 hover:bg-accent rounded"
																					onclick={() => moveCcRoom(cc.id, r.room_id, 1)}
																					disabled={r.rank === ccRooms(cc.id).length}
																					aria-label="เลื่อนลง"
																				>
																					↓
																				</button>
																				<button
																					class="text-xs px-1 hover:bg-destructive/20 rounded text-destructive"
																					onclick={() => removeCcRoom(cc.id, r.room_id)}
																					aria-label="ลบ"
																				>
																					✕
																				</button>
																			</div>
																		{/each}
																		<select
																			class="text-xs border rounded px-2 py-1 bg-background w-full"
																			onchange={(e) => {
																				if (e.currentTarget.value) {
																					addCcRoom(cc.id, e.currentTarget.value);
																					e.currentTarget.value = '';
																				}
																			}}
																		>
																			<option value="">+ เพิ่มห้อง...</option>
																			{#each allRooms.filter((r) => !ccRooms(cc.id).some((cr) => cr.room_id === r.id)) as r (r.id)}
																				<option value={r.id}>{r.code} — {r.name_th}</option>
																			{/each}
																		</select>
																	</div>
																	<p class="text-xs text-muted-foreground mt-1">
																		scheduler ลองห้องตามลำดับ — "บังคับ" = ถ้าห้องเต็มจะ fail ไม่ลองห้องอื่น
																	</p>
																</div>

																<!-- CC unavailable grid -->
																<div>
																	<Label class="text-xs">คาบที่ห้ามจัดวิชานี้</Label>
																	<div class="overflow-x-auto mt-1">
																		<table class="text-xs border-collapse">
																			<thead>
																				<tr>
																					<th class="border p-1 bg-muted sticky left-0 z-10">วัน</th>
																					{#each periods as p (p.id)}
																						<th class="border p-1 bg-muted min-w-[50px]">
																							{p.name || `${p.order_index}`}
																						</th>
																					{/each}
																				</tr>
																			</thead>
																			<tbody>
																				{#each schoolDays as day (day.value)}
																					<tr>
																						<td class="border p-1 bg-muted font-medium sticky left-0 z-10">
																							{day.shortLabel}
																						</td>
																						{#each periods as p (p.id)}
																							{@const inherited = ccIsInheritedUnavailable(cc, day.value, p.id)}
																							{@const unavail = ccIsUnavailable(cc, day.value, p.id)}
																							<td class="border p-0 text-center">
																								<button
																									onclick={() => toggleCcUnavailable(cc, day.value, p.id)}
																									disabled={inherited}
																									title={inherited ? 'ครูในทีมไม่ว่าง' : ''}
																									class="w-full h-6 transition-colors {unavail
																										? inherited
																											? 'bg-muted-foreground/40 cursor-not-allowed'
																											: 'bg-destructive/80 hover:bg-destructive text-destructive-foreground'
																										: 'hover:bg-accent'}"
																								>
																									{#if unavail}
																										{inherited ? '🔒' : '✕'}
																									{/if}
																								</button>
																							</td>
																						{/each}
																					</tr>
																				{/each}
																			</tbody>
																		</table>
																	</div>
																	<p class="text-xs text-muted-foreground mt-1">
																		🔒 = inherited จากครูในทีม (แก้ที่ row ครู) — ✕ = subject-level
																	</p>
																</div>
															</div>
														{/if}
													</div>
												{/each}
											</div>
										{/if}
									</div>
								</div>
							{/if}
						</div>
					{/each}
				</div>
			{/if}
		</Card.Root>
	{/if}
</div>

<!-- Phase E: Auto-schedule result dialog -->
<Dialog.Root bind:open={showResultDialog}>
	<Dialog.Content class="max-w-2xl max-h-[80vh] overflow-y-auto">
		<Dialog.Header>
			<Dialog.Title>ผลการจัดตารางอัตโนมัติ</Dialog.Title>
		</Dialog.Header>
		{#if currentJob}
			<div class="space-y-3">
				<div class="grid grid-cols-3 gap-2 text-sm">
					<div class="border rounded p-2">
						<div class="text-xs text-muted-foreground">สถานะ</div>
						<div class="font-medium">{currentJob.status}</div>
					</div>
					<div class="border rounded p-2">
						<div class="text-xs text-muted-foreground">จัดสำเร็จ</div>
						<div class="font-medium text-emerald-700">
							{currentJob.scheduled_courses}/{currentJob.total_courses}
						</div>
					</div>
					<div class="border rounded p-2">
						<div class="text-xs text-muted-foreground">คะแนนคุณภาพ</div>
						<div class="font-medium">{currentJob.quality_score?.toFixed(1) ?? '-'}</div>
					</div>
				</div>

				{#if currentJob.failed_courses.length > 0}
					<div class="border rounded-md p-3 bg-destructive/5">
						<div class="flex items-center gap-2 font-medium text-destructive mb-2">
							<AlertCircle class="w-4 h-4" />
							วิชาที่จัดไม่ได้ ({currentJob.failed_courses.length})
						</div>
						<div class="space-y-2 max-h-[400px] overflow-y-auto">
							{#each currentJob.failed_courses as fc (fc.course_id)}
								<div class="border rounded p-2 bg-card text-sm">
									<div class="font-medium">
										{fc.subject_code} {fc.subject_name}
									</div>
									<div class="text-xs text-muted-foreground">{fc.classroom}</div>
									<div class="text-xs mt-1 text-destructive">{fc.reason}</div>
								</div>
							{/each}
						</div>
						<p class="text-xs text-muted-foreground mt-2">
							💡 ลองปรับ priority ครู / ลด constraint / เปลี่ยน pattern แล้วจัดใหม่
						</p>
					</div>
				{/if}

				{#if currentJob.error_message}
					<div class="border rounded-md p-3 bg-destructive/5 text-sm text-destructive">
						<div class="font-medium">ข้อผิดพลาด</div>
						<div class="mt-1">{currentJob.error_message}</div>
					</div>
				{/if}
			</div>
		{/if}
		<Dialog.Footer>
			{#if currentJob && currentJob.status === 'COMPLETED' && currentJob.scheduled_courses > 0}
				<Button variant="outline" onclick={handleUndo} disabled={undoing}>
					{#if undoing}
						<LoaderCircle class="w-4 h-4 animate-spin mr-2" />
					{:else}
						<Undo2 class="w-4 h-4 mr-2" />
					{/if}
					Undo
				</Button>
			{/if}
			<Button variant="outline" onclick={() => (showResultDialog = false)}>ปิด</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
