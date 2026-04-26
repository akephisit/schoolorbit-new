<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
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
		type SchedulingJobResponse
	} from '$lib/api/scheduling';
	import {
		getAcademicStructure,
		getSchoolDays,
		listClassrooms,
		type AcademicYear,
		type Semester
	} from '$lib/api/academic';
	import { Tabs, TabsContent, TabsList, TabsTrigger } from '$lib/components/ui/tabs';
	import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Badge } from '$lib/components/ui/badge';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Select from '$lib/components/ui/select';
	import {
		Sparkles,
		Save,
		LoaderCircle,
		Zap,
		TriangleAlert,
		Undo2,
		Pencil,
		Users,
		BookOpen,
		Briefcase,
		ArrowUp,
		ArrowDown
	} from 'lucide-svelte';

	let { data } = $props();

	// =========================================
	// Page State
	// =========================================

	let loading = $state(true);
	let activeYear = $state<AcademicYear | null>(null);
	let schoolDays = $state<{ value: string; label: string; shortLabel: string }[]>([]);
	let periods = $state<Period[]>([]);
	let allRooms = $state<RoomView[]>([]);

	let instructors = $state<InstructorConstraintView[]>([]);
	let allCcs = $state<ClassroomCourseConstraintView[]>([]);

	let defaultMaxConsecutive = $state(4);
	let semesters = $state<Semester[]>([]);
	let selectedSemesterId = $state('');

	// Search
	let instructorSearch = $state('');
	let ccSearch = $state('');

	// =========================================
	// Auto-schedule State
	// =========================================

	let autoScheduling = $state(false);
	let currentJob = $state<SchedulingJobResponse | null>(null);
	let showResultDialog = $state(false);
	let undoing = $state(false);
	let pollAbort: ReturnType<typeof setTimeout> | null = null;

	// =========================================
	// Edit Instructor Dialog
	// =========================================

	let showInstrDialog = $state(false);
	let editInstr = $state<InstructorConstraintView | null>(null);
	let instrSaving = $state(false);
	let instrUnavailable = $state<TimeSlot[]>([]);
	let instrRoomId = $state(''); // '' = no room

	function openInstrDialog(instr: InstructorConstraintView) {
		editInstr = instr;
		instrUnavailable = [...((instr.hard_unavailable_slots ?? []) as TimeSlot[])];
		instrRoomId = instr.assigned_room_id ?? '';
		showInstrDialog = true;
	}

	function instrSlotBusy(day: string, periodId: string): boolean {
		return instrUnavailable.some((s) => s.day === day && s.period_id === periodId);
	}

	function toggleInstrSlot(day: string, periodId: string) {
		const idx = instrUnavailable.findIndex((s) => s.day === day && s.period_id === periodId);
		if (idx >= 0) instrUnavailable = instrUnavailable.filter((_, i) => i !== idx);
		else instrUnavailable = [...instrUnavailable, { day, period_id: periodId }];
	}

	async function saveInstr() {
		if (!editInstr) return;
		instrSaving = true;
		try {
			const remoteRoom = editInstr.assigned_room_id ?? '';
			const roomChanged = instrRoomId !== remoteRoom;
			const req: Parameters<typeof updateInstructorConstraints>[1] = {
				hard_unavailable_slots: instrUnavailable
			};
			if (roomChanged) {
				if (instrRoomId === '') req.clear_assigned_room = true;
				else req.assigned_room_id = instrRoomId;
			}
			await updateInstructorConstraints(editInstr.id, req);
			toast.success('บันทึกเงื่อนไขครูเรียบร้อย');
			showInstrDialog = false;
			editInstr = null;
			await loadInstructors();
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'บันทึกไม่สำเร็จ');
		} finally {
			instrSaving = false;
		}
	}

	// =========================================
	// Edit CC Dialog
	// =========================================

	let showCcDialog = $state(false);
	let editCc = $state<ClassroomCourseConstraintView | null>(null);
	let ccSaving = $state(false);
	let ccPattern = $state<number[] | null>(null);
	let ccSameDay = $state(true);
	let ccUnavailable = $state<TimeSlot[]>([]);
	let ccRooms = $state<CcPreferredRoom[]>([]);
	let ccRoomsLoaded = $state(false);

	async function openCcDialog(cc: ClassroomCourseConstraintView) {
		editCc = cc;
		ccPattern = cc.consecutive_pattern ?? null;
		ccSameDay = cc.same_day_unique;
		ccUnavailable = [...(cc.hard_unavailable_slots ?? [])];
		ccRooms = [];
		ccRoomsLoaded = false;
		showCcDialog = true;
		// Lazy load preferred rooms
		try {
			const res = await listCcPreferredRooms(cc.id);
			ccRooms = (res.data ?? []).map((r) => ({ ...r }));
			ccRoomsLoaded = true;
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'โหลดห้องไม่สำเร็จ');
		}
	}

	function ccSlotBusy(day: string, periodId: string): boolean {
		return ccUnavailable.some((s) => s.day === day && s.period_id === periodId);
	}

	function ccSlotInherited(day: string, periodId: string): boolean {
		if (!editCc) return false;
		return (editCc.team_unavailable_slots ?? []).some(
			(s) => s.day === day && s.period_id === periodId
		);
	}

	function toggleCcSlot(day: string, periodId: string) {
		if (ccSlotInherited(day, periodId)) return;
		const idx = ccUnavailable.findIndex((s) => s.day === day && s.period_id === periodId);
		if (idx >= 0) ccUnavailable = ccUnavailable.filter((_, i) => i !== idx);
		else ccUnavailable = [...ccUnavailable, { day, period_id: periodId }];
	}

	function patternOptions(periodsCount: number): number[][] {
		if (periodsCount <= 0) return [[]];
		if (periodsCount > 6) return [[periodsCount]];
		const out: number[][] = [];
		const compose = (rem: number, acc: number[]) => {
			if (rem === 0) {
				out.push([...acc]);
				return;
			}
			for (let c = 1; c <= rem; c++) {
				acc.push(c);
				compose(rem - c, acc);
				acc.pop();
			}
		};
		compose(periodsCount, []);
		return out;
	}

	function patternEquals(a: number[] | null | undefined, b: number[] | null | undefined): boolean {
		if (!a && !b) return true;
		if (!a || !b) return false;
		if (a.length !== b.length) return false;
		return a.every((v, i) => v === b[i]);
	}

	function patternLabel(p: number[]): string {
		return p.join('+');
	}

	function addCcRoom(roomId: string) {
		if (ccRooms.some((r) => r.room_id === roomId)) return;
		const room = allRooms.find((r) => r.id === roomId);
		if (!room) return;
		ccRooms = [
			...ccRooms,
			{
				id: '',
				classroom_course_id: editCc?.id ?? '',
				room_id: roomId,
				room_code: room.code,
				room_name: room.name_th,
				rank: ccRooms.length + 1,
				is_required: false
			}
		];
	}

	function removeCcRoom(roomId: string) {
		ccRooms = ccRooms.filter((r) => r.room_id !== roomId).map((r, i) => ({ ...r, rank: i + 1 }));
	}

	function moveCcRoom(roomId: string, dir: -1 | 1) {
		const idx = ccRooms.findIndex((r) => r.room_id === roomId);
		const ni = idx + dir;
		if (idx < 0 || ni < 0 || ni >= ccRooms.length) return;
		const next = [...ccRooms];
		const [m] = next.splice(idx, 1);
		next.splice(ni, 0, m);
		ccRooms = next.map((r, i) => ({ ...r, rank: i + 1 }));
	}

	function toggleCcRoomRequired(roomId: string) {
		ccRooms = ccRooms.map((r) =>
			r.room_id === roomId ? { ...r, is_required: !r.is_required } : r
		);
	}

	async function saveCc() {
		if (!editCc) return;
		ccSaving = true;
		try {
			const tasks: Promise<unknown>[] = [];

			// CC constraints (pattern + same_day + unavailable)
			tasks.push(
				updateClassroomCourseConstraints(editCc.id, {
					consecutive_pattern: ccPattern,
					same_day_unique: ccSameDay,
					hard_unavailable_slots: ccUnavailable
				})
			);

			// Preferred rooms
			tasks.push(
				setCcPreferredRooms(editCc.id, {
					rooms: ccRooms.map((r) => ({
						room_id: r.room_id,
						rank: r.rank,
						is_required: r.is_required
					}))
				})
			);

			await Promise.all(tasks);
			toast.success('บันทึกเงื่อนไขรายวิชาเรียบร้อย');
			showCcDialog = false;
			editCc = null;
			await loadCcs();
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'บันทึกไม่สำเร็จ');
		} finally {
			ccSaving = false;
		}
	}

	// =========================================
	// Priority reorder (inline up/down buttons)
	// =========================================

	let priorityDirty = $state(false);

	async function moveInstructor(idx: number, dir: -1 | 1) {
		const ni = idx + dir;
		if (ni < 0 || ni >= instructors.length) return;
		const next = [...instructors];
		const [m] = next.splice(idx, 1);
		next.splice(ni, 0, m);
		instructors = next;
		priorityDirty = true;
	}

	async function savePriority() {
		if (!priorityDirty) return;
		try {
			await reorderInstructorPriority(instructors.map((i) => i.id));
			toast.success('บันทึกลำดับครูเรียบร้อย');
			priorityDirty = false;
			await loadInstructors();
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'บันทึกไม่สำเร็จ');
		}
	}

	// =========================================
	// Global settings
	// =========================================

	let savingSettings = $state(false);
	async function saveSettings() {
		savingSettings = true;
		try {
			await updateSchoolSettings({ default_max_consecutive: defaultMaxConsecutive });
			toast.success('บันทึกตั้งค่ารวมเรียบร้อย');
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'บันทึกไม่สำเร็จ');
		} finally {
			savingSettings = false;
		}
	}

	// =========================================
	// Auto-schedule
	// =========================================

	async function runAutoSchedule() {
		if (!selectedSemesterId) {
			toast.error('กรุณาเลือกภาคเรียน');
			return;
		}
		if (!activeYear) return;
		try {
			autoScheduling = true;
			const crRes = await listClassrooms({ year_id: activeYear.id });
			const classroom_ids = (crRes.data ?? []).map((c) => c.id);
			if (classroom_ids.length === 0) {
				toast.error('ไม่มีห้องเรียนในปีการศึกษานี้');
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
					pollAbort = setTimeout(check, 2000);
				}
			} catch (e) {
				toast.error(e instanceof Error ? e.message : 'ติดตามสถานะไม่สำเร็จ');
				autoScheduling = false;
			}
		};
		check();
	}

	async function handleUndo() {
		if (!currentJob || undoing) return;
		if (!window.confirm('Undo การจัดอัตโนมัติครั้งนี้? — จะลบ entries ที่ scheduler สร้าง'))
			return;
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

	// =========================================
	// Data Loading
	// =========================================

	async function loadInstructors() {
		const res = await listInstructorConstraints();
		instructors = (res.data ?? []).filter((i) => i.primary_course_count > 0);
	}

	async function loadCcs() {
		const res = await listClassroomCourseConstraints();
		allCcs = res.data ?? [];
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
			semesters = (struct.data.semesters ?? []).filter((s) => s.academic_year_id === activeYear!.id);
			const activeSem = semesters.find((s) => s.is_active) ?? semesters[0];
			if (activeSem) selectedSemesterId = activeSem.id;

			const [periodsRes, settingsRes, roomsRes] = await Promise.all([
				listPeriods(activeYear.id),
				getSchoolSettings(),
				listAllRooms(),
				loadInstructors(),
				loadCcs()
			]);
			periods = (periodsRes.data ?? []).sort((a, b) => a.order_index - b.order_index);
			defaultMaxConsecutive = settingsRes.data?.default_max_consecutive ?? 4;
			allRooms = roomsRes.data ?? [];
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'โหลดข้อมูลไม่สำเร็จ');
		} finally {
			loading = false;
		}
	}

	// =========================================
	// Derived (filtered lists)
	// =========================================

	const filteredInstructors = $derived(
		instructors.filter((i) =>
			(i.first_name + ' ' + i.last_name).toLowerCase().includes(instructorSearch.toLowerCase())
		)
	);

	const filteredCcs = $derived(
		allCcs.filter(
			(c) =>
				c.subject_code.toLowerCase().includes(ccSearch.toLowerCase()) ||
				c.subject_name.toLowerCase().includes(ccSearch.toLowerCase()) ||
				c.classroom_name.toLowerCase().includes(ccSearch.toLowerCase()) ||
				(c.primary_instructor_name ?? '').toLowerCase().includes(ccSearch.toLowerCase())
		)
	);

	onMount(loadAll);
	onDestroy(() => {
		if (pollAbort) clearTimeout(pollAbort);
	});
</script>

<svelte:head>
	<title>{data.title}</title>
</svelte:head>

<div class="space-y-6">
	<!-- Header -->
	<div class="flex items-center justify-between flex-wrap gap-2">
		<div>
			<h1 class="text-3xl font-bold tracking-tight flex items-center gap-2">
				<Sparkles class="h-7 w-7 text-primary" />
				ตั้งค่าจัดตารางอัตโนมัติ
			</h1>
			<p class="text-muted-foreground">
				กำหนดข้อจำกัดของครูและรายวิชา + ลำดับการจัด → กดจัดอัตโนมัติ
			</p>
		</div>
		<div class="flex items-center gap-2">
			{#if priorityDirty}
				<Button variant="outline" onclick={savePriority}>
					<Save class="w-4 h-4 mr-2" />
					บันทึกลำดับ
				</Button>
			{/if}
			<Button onclick={runAutoSchedule} disabled={loading || autoScheduling || !selectedSemesterId}>
				{#if autoScheduling}
					<LoaderCircle class="w-4 h-4 animate-spin mr-2" />
				{:else}
					<Zap class="w-4 h-4 mr-2" />
				{/if}
				จัดอัตโนมัติ
			</Button>
		</div>
	</div>

	{#if loading}
		<div class="flex items-center justify-center py-20">
			<LoaderCircle class="w-8 h-8 animate-spin text-muted-foreground" />
		</div>
	{:else}
		<!-- Global settings -->
		<Card>
			<CardHeader>
				<CardTitle class="text-base">ตั้งค่ารวม</CardTitle>
			</CardHeader>
			<CardContent>
				<div class="grid gap-3 md:grid-cols-2">
					<div class="grid gap-2">
						<Label for="max-consec">ครูสอนติดสูงสุดต่อวัน (คาบ)</Label>
						<div class="flex items-center gap-2">
							<Input
								id="max-consec"
								type="number"
								min="1"
								max="20"
								bind:value={defaultMaxConsecutive}
								class="w-32"
							/>
							<Button variant="outline" size="sm" onclick={saveSettings} disabled={savingSettings}>
								{#if savingSettings}
									<LoaderCircle class="w-3 h-3 animate-spin" />
								{:else}
									บันทึก
								{/if}
							</Button>
						</div>
					</div>
					<div class="grid gap-2">
						<Label for="semester">ภาคเรียนที่จะจัด</Label>
						<Select.Root type="single" bind:value={selectedSemesterId}>
							<Select.Trigger id="semester" class="w-full">
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
			</CardContent>
		</Card>

		<Tabs value="instructors" class="w-full">
			<TabsList class="grid w-full grid-cols-2 lg:w-[400px]">
				<TabsTrigger value="instructors" class="flex gap-2">
					<Users class="h-4 w-4" />
					ครูผู้สอน
				</TabsTrigger>
				<TabsTrigger value="courses" class="flex gap-2">
					<BookOpen class="h-4 w-4" />
					รายวิชา (ห้อง × วิชา)
				</TabsTrigger>
			</TabsList>

			<!-- ===== Instructors Tab ===== -->
			<TabsContent value="instructors" class="mt-6">
				<Card>
					<CardHeader class="flex flex-row items-center justify-between">
						<div>
							<CardTitle>ครูผู้สอน + ลำดับการจัด</CardTitle>
							<p class="text-sm text-muted-foreground mt-1">
								ลำดับด้านบน = ได้คาบดี ๆ ก่อน — ใช้ลูกศร ↑↓ เพื่อสลับลำดับ
							</p>
						</div>
						<Input
							placeholder="ค้นหาครู..."
							class="max-w-sm"
							bind:value={instructorSearch}
						/>
					</CardHeader>
					<CardContent>
						<div class="rounded-md border">
							<table class="w-full caption-bottom text-sm text-left">
								<thead class="[&_tr]:border-b">
									<tr>
										<th class="h-12 px-2 align-middle font-medium text-muted-foreground w-[80px]">
											ลำดับ
										</th>
										<th class="h-12 px-4 align-middle font-medium text-muted-foreground">ชื่อ-สกุล</th>
										<th class="h-12 px-4 align-middle font-medium text-muted-foreground w-[100px]">
											จำนวนวิชา
										</th>
										<th class="h-12 px-4 align-middle font-medium text-muted-foreground">ห้องประจำ</th>
										<th class="h-12 px-4 align-middle font-medium text-muted-foreground w-[110px]">
											ไม่ว่าง
										</th>
										<th
											class="h-12 px-4 align-middle font-medium text-muted-foreground text-right w-[120px]"
										>
											ดำเนินการ
										</th>
									</tr>
								</thead>
								<tbody class="[&_tr:last-child]:border-0">
									{#each filteredInstructors as instr, idx (instr.id)}
										<tr class="border-b transition-colors hover:bg-muted/50">
											<td class="p-2 align-middle">
												<div class="flex items-center gap-1">
													<Badge variant="secondary" class="w-8 justify-center">
														{idx + 1}
													</Badge>
													<div class="flex flex-col">
														<button
															onclick={() => moveInstructor(idx, -1)}
															disabled={idx === 0 || !!instructorSearch}
															aria-label="เลื่อนขึ้น"
															class="text-muted-foreground hover:text-foreground disabled:opacity-30 px-1"
														>
															<ArrowUp class="h-3 w-3" />
														</button>
														<button
															onclick={() => moveInstructor(idx, 1)}
															disabled={idx === filteredInstructors.length - 1 || !!instructorSearch}
															aria-label="เลื่อนลง"
															class="text-muted-foreground hover:text-foreground disabled:opacity-30 px-1"
														>
															<ArrowDown class="h-3 w-3" />
														</button>
													</div>
												</div>
											</td>
											<td class="p-4 align-middle font-medium">
												{instr.first_name} {instr.last_name}
											</td>
											<td class="p-4 align-middle">
												<span class="px-2 py-1 bg-secondary rounded-md text-xs font-medium">
													{instr.primary_course_count}
												</span>
											</td>
											<td class="p-4 align-middle">
												{#if instr.assigned_room_name}
													<div class="flex items-center gap-1 text-blue-600">
														<Briefcase class="h-3 w-3" />
														{instr.assigned_room_name}
													</div>
												{:else}
													<span class="text-muted-foreground text-xs">-</span>
												{/if}
											</td>
											<td class="p-4 align-middle">
												{#if (instr.hard_unavailable_slots ?? []).length > 0}
													<Badge variant="outline">
														{(instr.hard_unavailable_slots ?? []).length} คาบ
													</Badge>
												{:else}
													<span class="text-muted-foreground text-xs">-</span>
												{/if}
											</td>
											<td class="p-4 align-middle text-right">
												<Button variant="ghost" size="sm" onclick={() => openInstrDialog(instr)}>
													<Pencil class="h-4 w-4 mr-2" />
													ตั้งค่า
												</Button>
											</td>
										</tr>
									{:else}
										<tr>
											<td colspan="6" class="p-8 text-center text-muted-foreground">
												{instructorSearch ? 'ไม่พบครูที่ค้นหา' : 'ยังไม่มีครูที่เป็น primary instructor'}
											</td>
										</tr>
									{/each}
								</tbody>
							</table>
						</div>
						{#if instructorSearch}
							<p class="text-xs text-muted-foreground mt-2">
								💡 ลบคำค้นหาเพื่อสลับลำดับครู
							</p>
						{/if}
					</CardContent>
				</Card>
			</TabsContent>

			<!-- ===== Classroom Courses Tab ===== -->
			<TabsContent value="courses" class="mt-6">
				<Card>
					<CardHeader class="flex flex-row items-center justify-between">
						<div>
							<CardTitle>เงื่อนไขรายวิชา (ห้อง × วิชา)</CardTitle>
							<p class="text-sm text-muted-foreground mt-1">
								Pattern, ห้อง, คาบที่ห้ามจัด — ตั้งค่าแยกต่อ (subject × classroom)
							</p>
						</div>
						<Input placeholder="ค้นหาวิชา/ห้อง/ครู..." class="max-w-sm" bind:value={ccSearch} />
					</CardHeader>
					<CardContent>
						<div class="rounded-md border">
							<table class="w-full caption-bottom text-sm text-left">
								<thead class="[&_tr]:border-b">
									<tr>
										<th class="h-12 px-4 align-middle font-medium text-muted-foreground w-[110px]">
											รหัสวิชา
										</th>
										<th class="h-12 px-4 align-middle font-medium text-muted-foreground">ชื่อวิชา</th>
										<th class="h-12 px-4 align-middle font-medium text-muted-foreground w-[100px]">
											ห้อง
										</th>
										<th class="h-12 px-4 align-middle font-medium text-muted-foreground w-[80px]">
											คาบ/สัปดาห์
										</th>
										<th class="h-12 px-4 align-middle font-medium text-muted-foreground">ครูหลัก</th>
										<th class="h-12 px-4 align-middle font-medium text-muted-foreground w-[110px]">
											Pattern
										</th>
										<th
											class="h-12 px-4 align-middle font-medium text-muted-foreground text-right w-[120px]"
										>
											ดำเนินการ
										</th>
									</tr>
								</thead>
								<tbody class="[&_tr:last-child]:border-0">
									{#each filteredCcs as cc (cc.id)}
										<tr class="border-b transition-colors hover:bg-muted/50">
											<td class="p-4 align-middle font-mono text-xs">{cc.subject_code}</td>
											<td class="p-4 align-middle">{cc.subject_name}</td>
											<td class="p-4 align-middle text-muted-foreground">{cc.classroom_name}</td>
											<td class="p-4 align-middle">
												<span class="px-2 py-1 bg-secondary rounded-md text-xs font-medium">
													{cc.periods_per_week ?? '-'}
												</span>
											</td>
											<td class="p-4 align-middle">
												{#if cc.primary_instructor_name}
													{cc.primary_instructor_name}
												{:else}
													<span class="text-muted-foreground text-xs">— ยังไม่มี —</span>
												{/if}
											</td>
											<td class="p-4 align-middle">
												{#if cc.consecutive_pattern && cc.consecutive_pattern.length > 0}
													<Badge variant="outline">{patternLabel(cc.consecutive_pattern)}</Badge>
												{:else}
													<span class="text-muted-foreground text-xs">auto</span>
												{/if}
											</td>
											<td class="p-4 align-middle text-right">
												<Button variant="ghost" size="sm" onclick={() => openCcDialog(cc)}>
													<Pencil class="h-4 w-4 mr-2" />
													ตั้งค่า
												</Button>
											</td>
										</tr>
									{:else}
										<tr>
											<td colspan="7" class="p-8 text-center text-muted-foreground">
												{ccSearch ? 'ไม่พบรายวิชาที่ค้นหา' : 'ยังไม่มีรายวิชา'}
											</td>
										</tr>
									{/each}
								</tbody>
							</table>
						</div>
					</CardContent>
				</Card>
			</TabsContent>
		</Tabs>
	{/if}
</div>

<!-- =========================================
     Instructor Dialog
     ========================================= -->
<Dialog.Root bind:open={showInstrDialog}>
	<Dialog.Content class="sm:max-w-[800px]">
		<Dialog.Header>
			<Dialog.Title>
				ตั้งค่าเงื่อนไขครู: {editInstr?.first_name}
				{editInstr?.last_name}
			</Dialog.Title>
			<Dialog.Description>กำหนดห้องประจำ + คาบที่ไม่ว่าง</Dialog.Description>
		</Dialog.Header>

		<div class="grid gap-6 py-4">
			<!-- Settings -->
			<div class="grid gap-2">
				<Label for="instr-room">ห้องประจำ</Label>
				<select
					id="instr-room"
					class="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
					bind:value={instrRoomId}
				>
					<option value="">— ไม่กำหนด —</option>
					{#each allRooms as r (r.id)}
						<option value={r.id}>{r.code} — {r.name_th}</option>
					{/each}
				</select>
				<p class="text-xs text-muted-foreground">
					Scheduler จะใช้ห้องนี้เป็น fallback ถ้าวิชาไม่ได้กำหนดห้องเฉพาะ
				</p>
			</div>

			<!-- Availability Grid -->
			<div class="space-y-2">
				<div class="flex items-center justify-between flex-wrap gap-2">
					<Label>คาบที่ไม่ว่าง (คลิกเพื่อ toggle)</Label>
					<div class="flex gap-4 text-xs">
						<div class="flex items-center gap-1">
							<div class="w-3 h-3 bg-white border rounded"></div>
							<span>ว่าง</span>
						</div>
						<div class="flex items-center gap-1">
							<div class="w-3 h-3 bg-red-100 border border-red-200 rounded"></div>
							<span>ไม่ว่าง</span>
						</div>
					</div>
				</div>

				<div class="border rounded-md p-2 overflow-x-auto">
					<div class="min-w-[500px]">
						<!-- Header -->
						<div
							class="grid gap-1 mb-1"
							style="grid-template-columns: 60px repeat({periods.length}, 1fr)"
						>
							<div class="font-bold text-xs text-center p-2">วัน</div>
							{#each periods as p (p.id)}
								<div class="font-bold text-xs text-center p-2 bg-muted rounded">
									{p.name || `P${p.order_index}`}
								</div>
							{/each}
						</div>

						<!-- Rows -->
						{#each schoolDays as day (day.value)}
							<div
								class="grid gap-1 mb-1"
								style="grid-template-columns: 60px repeat({periods.length}, 1fr)"
							>
								<div class="font-bold text-xs flex items-center justify-center bg-muted rounded">
									{day.shortLabel}
								</div>
								{#each periods as p (p.id)}
									{@const busy = instrSlotBusy(day.value, p.id)}
									<button
										class="h-9 rounded border transition-colors text-xs flex items-center justify-center {busy
											? 'bg-red-100 border-red-200 text-red-700 hover:bg-red-200'
											: 'bg-white hover:bg-slate-50'}"
										onclick={() => toggleInstrSlot(day.value, p.id)}
									>
										{busy ? 'BUSY' : ''}
									</button>
								{/each}
							</div>
						{/each}
					</div>
				</div>
			</div>
		</div>

		<Dialog.Footer>
			<Button variant="outline" onclick={() => (showInstrDialog = false)}>ยกเลิก</Button>
			<Button onclick={saveInstr} disabled={instrSaving}>
				{#if instrSaving}
					<LoaderCircle class="mr-2 h-4 w-4 animate-spin" />
					กำลังบันทึก...
				{:else}
					บันทึก
				{/if}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<!-- =========================================
     Classroom Course Dialog
     ========================================= -->
<Dialog.Root bind:open={showCcDialog}>
	<Dialog.Content class="sm:max-w-[850px] max-h-[90vh] overflow-y-auto">
		<Dialog.Header>
			<Dialog.Title>
				{editCc?.subject_code} {editCc?.subject_name} — {editCc?.classroom_name}
			</Dialog.Title>
			<Dialog.Description>
				ครูหลัก: {editCc?.primary_instructor_name ?? '— ยังไม่มี —'}
				· {editCc?.periods_per_week ?? 0} คาบ/สัปดาห์
			</Dialog.Description>
		</Dialog.Header>

		<div class="grid gap-6 py-4">
			<!-- Pattern + same_day_unique -->
			<div class="grid gap-2">
				<Label>รูปแบบการจัดคาบ</Label>
				<div class="flex flex-wrap gap-2">
					<button
						type="button"
						onclick={() => (ccPattern = null)}
						class="text-xs px-3 py-1.5 rounded border transition-colors {ccPattern === null
							? 'bg-primary text-primary-foreground border-primary'
							: 'bg-background hover:bg-accent'}"
					>
						Auto
					</button>
					{#each patternOptions(editCc?.periods_per_week ?? 0) as opt (patternLabel(opt))}
						<button
							type="button"
							onclick={() => (ccPattern = opt)}
							class="text-xs px-3 py-1.5 rounded border transition-colors {patternEquals(ccPattern, opt)
								? 'bg-primary text-primary-foreground border-primary'
								: 'bg-background hover:bg-accent'}"
						>
							{patternLabel(opt)}
						</button>
					{/each}
				</div>
				<p class="text-xs text-muted-foreground">
					Auto = scheduler จัดเอง · 1+1+1 = 3 คาบแยกวัน · 2+1 = 2 คาบติด + 1 คาบแยก · 3 = 3 คาบติดวันเดียว
				</p>
			</div>

			<label class="flex items-center gap-2 cursor-pointer">
				<input type="checkbox" bind:checked={ccSameDay} class="cursor-pointer" />
				<span class="text-sm">ห้ามวันเดียวกันมีรหัสวิชาซ้ำ</span>
			</label>

			<!-- Preferred Rooms -->
			<div class="grid gap-2">
				<Label>ห้องที่ใช้สอน (เรียงตามลำดับ)</Label>
				{#if !ccRoomsLoaded}
					<div class="flex items-center gap-2 text-sm text-muted-foreground">
						<LoaderCircle class="w-4 h-4 animate-spin" /> กำลังโหลด...
					</div>
				{:else}
					<div class="space-y-1">
						{#each ccRooms as r (r.room_id)}
							<div class="flex items-center gap-2 border rounded px-2 py-1.5 bg-card">
								<span class="text-xs text-muted-foreground w-5">{r.rank}.</span>
								<span class="text-sm flex-1">
									<span class="font-medium">{r.room_code}</span>
									<span class="text-muted-foreground"> — {r.room_name}</span>
								</span>
								<label class="text-xs flex items-center gap-1 cursor-pointer">
									<input
										type="checkbox"
										checked={r.is_required}
										onchange={() => toggleCcRoomRequired(r.room_id)}
									/>
									บังคับ
								</label>
								<Button
									variant="ghost"
									size="sm"
									onclick={() => moveCcRoom(r.room_id, -1)}
									disabled={r.rank === 1}
								>
									<ArrowUp class="h-3 w-3" />
								</Button>
								<Button
									variant="ghost"
									size="sm"
									onclick={() => moveCcRoom(r.room_id, 1)}
									disabled={r.rank === ccRooms.length}
								>
									<ArrowDown class="h-3 w-3" />
								</Button>
								<Button
									variant="ghost"
									size="sm"
									onclick={() => removeCcRoom(r.room_id)}
									class="text-destructive hover:text-destructive"
								>
									✕
								</Button>
							</div>
						{/each}
						<select
							class="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
							onchange={(e) => {
								if (e.currentTarget.value) {
									addCcRoom(e.currentTarget.value);
									e.currentTarget.value = '';
								}
							}}
						>
							<option value="">+ เพิ่มห้อง...</option>
							{#each allRooms.filter((r) => !ccRooms.some((cr) => cr.room_id === r.id)) as r (r.id)}
								<option value={r.id}>{r.code} — {r.name_th}</option>
							{/each}
						</select>
					</div>
					<p class="text-xs text-muted-foreground">
						"บังคับ" = ถ้าห้องเต็มจะ fail ไม่ลอง fallback
					</p>
				{/if}
			</div>

			<!-- CC Unavailable Grid -->
			<div class="space-y-2">
				<div class="flex items-center justify-between flex-wrap gap-2">
					<Label>คาบที่ห้ามจัดวิชานี้ (ห้องนี้)</Label>
					<div class="flex gap-4 text-xs">
						<div class="flex items-center gap-1">
							<div class="w-3 h-3 bg-white border rounded"></div>
							<span>ว่าง</span>
						</div>
						<div class="flex items-center gap-1">
							<div class="w-3 h-3 bg-red-100 border border-red-200 rounded"></div>
							<span>ห้าม (cc)</span>
						</div>
						<div class="flex items-center gap-1">
							<div class="w-3 h-3 bg-muted-foreground/30 border rounded"></div>
							<span>🔒 ครูใน team ไม่ว่าง</span>
						</div>
					</div>
				</div>

				<div class="border rounded-md p-2 overflow-x-auto">
					<div class="min-w-[500px]">
						<div
							class="grid gap-1 mb-1"
							style="grid-template-columns: 60px repeat({periods.length}, 1fr)"
						>
							<div class="font-bold text-xs text-center p-2">วัน</div>
							{#each periods as p (p.id)}
								<div class="font-bold text-xs text-center p-2 bg-muted rounded">
									{p.name || `P${p.order_index}`}
								</div>
							{/each}
						</div>

						{#each schoolDays as day (day.value)}
							<div
								class="grid gap-1 mb-1"
								style="grid-template-columns: 60px repeat({periods.length}, 1fr)"
							>
								<div class="font-bold text-xs flex items-center justify-center bg-muted rounded">
									{day.shortLabel}
								</div>
								{#each periods as p (p.id)}
									{@const inherited = ccSlotInherited(day.value, p.id)}
									{@const busy = ccSlotBusy(day.value, p.id) || inherited}
									<button
										class="h-9 rounded border transition-colors text-xs flex items-center justify-center {inherited
											? 'bg-muted-foreground/30 border-muted-foreground/40 cursor-not-allowed'
											: busy
												? 'bg-red-100 border-red-200 text-red-700 hover:bg-red-200'
												: 'bg-white hover:bg-slate-50'}"
										onclick={() => toggleCcSlot(day.value, p.id)}
										disabled={inherited}
										title={inherited ? 'ครูใน team ไม่ว่าง' : ''}
									>
										{inherited ? '🔒' : busy ? 'BUSY' : ''}
									</button>
								{/each}
							</div>
						{/each}
					</div>
				</div>
			</div>
		</div>

		<Dialog.Footer>
			<Button variant="outline" onclick={() => (showCcDialog = false)}>ยกเลิก</Button>
			<Button onclick={saveCc} disabled={ccSaving}>
				{#if ccSaving}
					<LoaderCircle class="mr-2 h-4 w-4 animate-spin" />
					กำลังบันทึก...
				{:else}
					บันทึก
				{/if}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<!-- =========================================
     Auto-schedule Result Dialog
     ========================================= -->
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
							<TriangleAlert class="w-4 h-4" />
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
