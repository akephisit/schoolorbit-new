<script lang="ts">
	import { onMount } from 'svelte';
	import {
		getAcademicStructure,
		listActivitySlots,
		updateActivitySlot,
		deleteActivitySlot,
		listActivityGroups,
		createActivityGroup,
		updateActivityGroup,
		deleteActivityGroup,
		listClassrooms,
		listSlotInstructors,
		addSlotInstructor,
		removeSlotInstructor,
		removeAllSlotInstructors,
		ACTIVITY_TYPE_LABELS,
		type SlotInstructor,
		type ActivitySlot,
		type ActivityGroup,
		type AcademicStructureData,
		type Classroom,
		listSlotClassroomAssignments,
		batchUpsertSlotClassroomAssignments,
		deleteAllSlotClassroomAssignments,
		deleteAllSlotGroups,
		deleteSlotTimetableEntries,
		type SlotClassroomAssignment,
		listStudyPlanVersions,
		generateActivitiesFromPlan,
		type StudyPlanVersion
	} from '$lib/api/academic';
	import { lookupStaff, type StaffLookupItem } from '$lib/api/lookup';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Textarea } from '$lib/components/ui/textarea';
	import { Badge } from '$lib/components/ui/badge';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Select from '$lib/components/ui/select';
	import * as Popover from '$lib/components/ui/popover';
	import { toast } from 'svelte-sonner';
	import { Users, Plus, Pencil, Trash2, Check, ChevronsUpDown, UserCog, ChevronDown, ChevronRight, Settings, FolderInput } from 'lucide-svelte';
	import { can } from '$lib/stores/permissions';
	import { goto } from '$app/navigation';

	// ── State ──────────────────────────────────────────
	let loading = $state(true);
	let saving = $state(false);

	let structure = $state<AcademicStructureData>({ years: [], semesters: [], levels: [] });
	let classrooms = $state<Classroom[]>([]);
	let staffList = $state<StaffLookupItem[]>([]);
	let slotInstructorsMap = $state<Record<string, SlotInstructor[]>>({});
	let slotClassroomAssignmentsMap = $state<Record<string, SlotClassroomAssignment[]>>({});
	let slots = $state<ActivitySlot[]>([]);
	let groups = $state<ActivityGroup[]>([]);

	// Filters
	let filterYearId = $state('');
	let filterSemesterId = $state('');
	let filterType = $state('');

	// Expanded slots
	let expandedSlots = $state<Set<string>>(new Set());

	// Slot Dialog
	let showSlotDialog = $state(false);
	let isSlotEdit = $state(false);
	let editSlotTarget = $state<ActivitySlot | null>(null);
	let slotName = $state('');
	let slotDescription = $state('');
	let slotActivityType = $state('club');
	let slotSemesterId = $state('');
	let slotRegistrationType = $state('assigned');
	let slotPeriodsPerWeek = $state(1);
	let slotSchedulingMode = $state<'synchronized' | 'independent'>('synchronized');
	let slotAllowedGradeLevelIds = $state<string[]>([]);

	// Group Dialog (สร้าง/แก้ไขชุมนุมภายใต้ slot)
	let showGroupDialog = $state(false);
	let isGroupEdit = $state(false);
	let editGroupTarget = $state<ActivityGroup | null>(null);
	let groupSlotId = $state('');
	let groupName = $state('');
	let groupDescription = $state('');
	let groupInstructorId = $state('');
	let groupMaxCapacity = $state('');
	let groupAllowedGradeLevelIds = $state<string[]>([]);

	// Delete
	let deleteSlotTarget = $state<ActivitySlot | null>(null);
	let showDeleteSlotDialog = $state(false);
	let deleteGroupTarget = $state<ActivityGroup | null>(null);
	let showDeleteGroupDialog = $state(false);
	let showSwitchModeDialog = $state(false);
	let switchModeGroupCount = $state(0);
	let switchModeMemberCount = $state(0);

	// Generate from Plan Dialog
	let showGenerateDialog = $state(false);
	let generateVersionId = $state('');
	let planVersions = $state<StudyPlanVersion[]>([]);
	let generating = $state(false);


	// ── Computed ───────────────────────────────────────
	let filteredSlots = $derived(
		slots.filter((s) => {
			if (filterType && s.activity_type !== filterType) return false;
			return true;
		})
	);

	let yearSemesters = $derived(
		structure.semesters.filter((s) => s.academic_year_id === filterYearId)
	);

	let currentYearName = $derived(
		structure.years.find((y) => y.id === filterYearId)?.name ?? 'เลือกปีการศึกษา'
	);

	let currentSemesterName = $derived(
		structure.semesters.find((s) => s.id === filterSemesterId)?.name ?? 'เลือกภาคเรียน'
	);

	// ชั้นที่เปิดสอนในปีนี้ (จาก classrooms)
	let yearGradeLevels = $derived.by(() => {
		const gradeIds = [...new Set(classrooms.map((c) => c.grade_level_id))];
		return structure.levels
			.filter((l) => gradeIds.includes(l.id))
			.sort((a, b) => (a.code > b.code ? 1 : -1));
	});

	let slotSemesterName = $derived(
		structure.semesters.find((s) => s.id === slotSemesterId)?.name ?? 'เลือก...'
	);

	// ── Load ───────────────────────────────────────────
	onMount(async () => {
		const [structRes, staffRes] = await Promise.all([
			getAcademicStructure(),
			lookupStaff({ activeOnly: true, limit: 1000 })
		]);
		structure = structRes.data;
		staffList = staffRes;

		// Default to active year + semester
		const activeSem = structure.semesters.find((s) => s.is_active);
		if (activeSem) {
			filterYearId = activeSem.academic_year_id;
			filterSemesterId = activeSem.id;
		} else if (structure.years.length > 0) {
			const activeYear = structure.years.find((y) => y.is_active) ?? structure.years[0];
			filterYearId = activeYear.id;
		}

		await loadClassrooms();
		await loadData();
		loading = false;
	});

	async function loadClassrooms() {
		if (!filterYearId) return;
		classrooms = (await listClassrooms({ year_id: filterYearId })).data ?? [];
	}

	async function loadData() {
		if (!filterSemesterId) return;
		const [slotsRes, groupsRes] = await Promise.all([
			listActivitySlots({ semester_id: filterSemesterId }),
			listActivityGroups({ semester_id: filterSemesterId })
		]);
		slots = slotsRes.data ?? [];
		groups = groupsRes.data ?? [];
		expandedSlots = new Set(slots.map((s) => s.id));
		// Load slot instructors
		const instrMap: Record<string, SlotInstructor[]> = {};
		await Promise.all(slots.map(async (s) => {
			try { instrMap[s.id] = (await listSlotInstructors(s.id)).data ?? []; }
			catch { instrMap[s.id] = []; }
		}));
		slotInstructorsMap = instrMap;
		// Load classroom assignments for independent slots
		const assignMap: Record<string, SlotClassroomAssignment[]> = {};
		await Promise.all(slots.filter((s) => s.scheduling_mode === 'independent').map(async (s) => {
			try { assignMap[s.id] = (await listSlotClassroomAssignments(s.id)).data ?? []; }
			catch { assignMap[s.id] = []; }
		}));
		slotClassroomAssignmentsMap = assignMap;
	}

	let prevYearId = $state('');

	$effect(() => {
		if (filterYearId && filterYearId !== prevYearId) {
			prevYearId = filterYearId;
			loadClassrooms();
			// Auto-select first semester of new year
			const sems = structure.semesters.filter((s) => s.academic_year_id === filterYearId);
			const activeSem = sems.find((s) => s.is_active) ?? sems[0];
			if (activeSem) filterSemesterId = activeSem.id;
		}
	});

	$effect(() => {
		if (filterSemesterId) loadData();
	});

	function groupsForSlot(slotId: string) {
		return groups.filter((g) => g.slot_id === slotId);
	}

	function toggleSlot(id: string) {
		const next = new Set(expandedSlots);
		if (next.has(id)) next.delete(id); else next.add(id);
		expandedSlots = next;
	}

	// ── Generate from Plan ────────────────────────────
	async function openGenerateDialog() {
		try {
			const res = await listStudyPlanVersions();
			planVersions = res.data ?? [];
		} catch { planVersions = []; }
		generateVersionId = '';
		showGenerateDialog = true;
	}

	async function handleGenerate() {
		if (!generateVersionId || !filterSemesterId) {
			toast.error('กรุณาเลือกหลักสูตรและภาคเรียน');
			return;
		}
		generating = true;
		try {
			const res = await generateActivitiesFromPlan({
				study_plan_version_id: generateVersionId,
				semester_id: filterSemesterId
			});
			toast.success(`สร้าง ${res.created} กิจกรรม (ข้าม ${res.skipped} อันที่มีอยู่แล้ว)`);
			showGenerateDialog = false;
			await loadData();
		} catch {
			toast.error('Generate ไม่สำเร็จ');
		} finally {
			generating = false;
		}
	}

	// ── Slot Dialog ───────────────────────────────────
	function openCreateSlot() {
		slotName = ''; slotDescription = ''; slotActivityType = 'club';
		slotSemesterId = filterSemesterId; slotRegistrationType = 'assigned';
		slotPeriodsPerWeek = 1; slotSchedulingMode = 'synchronized';
		slotAllowedGradeLevelIds = [];
		isSlotEdit = false; editSlotTarget = null;
		showSlotDialog = true;
	}

	function openEditSlot(s: ActivitySlot) {
		// Template fields live in catalog — read-only here. Only registration is editable.
		slotName = s.name;
		slotDescription = s.description ?? '';
		slotActivityType = s.activity_type;
		slotSemesterId = s.semester_id;
		slotRegistrationType = s.registration_type;
		slotPeriodsPerWeek = s.periods_per_week;
		slotSchedulingMode = s.scheduling_mode;
		slotAllowedGradeLevelIds = s.allowed_grade_level_ids ?? [];
		isSlotEdit = true; editSlotTarget = s;
		showSlotDialog = true;
	}


	function toggleSlotGrade(id: string) {
		slotAllowedGradeLevelIds = slotAllowedGradeLevelIds.includes(id)
			? slotAllowedGradeLevelIds.filter((x) => x !== id) : [...slotAllowedGradeLevelIds, id];
	}

	async function handleSaveSlot() {
		doSaveSlot();
	}

	async function handleToggleTeacherReg(slot: ActivitySlot) {
		await updateActivitySlot(slot.id, { teacher_reg_open: !slot.teacher_reg_open } as any);
		await loadData();
		toast.success(slot.teacher_reg_open ? 'ปิดลงทะเบียนครูแล้ว' : 'เปิดลงทะเบียนครูแล้ว');
	}

	async function handleToggleStudentReg(slot: ActivitySlot) {
		await updateActivitySlot(slot.id, { student_reg_open: !slot.student_reg_open } as any);
		await loadData();
		toast.success(slot.student_reg_open ? 'ปิดลงทะเบียนนักเรียนแล้ว' : 'เปิดลงทะเบียนนักเรียนแล้ว');
	}

	function confirmDeleteSlot(s: ActivitySlot) { deleteSlotTarget = s; showDeleteSlotDialog = true; }
	async function handleDeleteSlot() {
		if (!deleteSlotTarget) return;
		try {
			await deleteSlotTimetableEntries(deleteSlotTarget.id);
			await deleteActivitySlot(deleteSlotTarget.id);
			toast.success('ลบแล้ว');
			showDeleteSlotDialog = false;
			await loadData();
		} catch { toast.error('เกิดข้อผิดพลาด'); }
	}

	// ── Group Dialog ──────────────────────────────────
	function openCreateGroup(slotId: string) {
		groupSlotId = slotId; groupName = ''; groupDescription = '';
		groupInstructorId = ''; groupMaxCapacity = '';
		groupAllowedGradeLevelIds = [];
		isGroupEdit = false; editGroupTarget = null;
		showGroupDialog = true;
	}

	function openEditGroup(g: ActivityGroup) {
		groupSlotId = g.slot_id ?? '';
		groupName = g.name;
		groupDescription = g.description ?? '';
		groupInstructorId = g.instructor_id ?? '';
		groupMaxCapacity = g.max_capacity ? String(g.max_capacity) : '';
		groupAllowedGradeLevelIds = g.allowed_grade_level_ids ?? [];
		isGroupEdit = true; editGroupTarget = g;
		showGroupDialog = true;
	}

	function toggleGroupGrade(id: string) {
		groupAllowedGradeLevelIds = groupAllowedGradeLevelIds.includes(id)
			? groupAllowedGradeLevelIds.filter((x) => x !== id) : [...groupAllowedGradeLevelIds, id];
	}

	async function handleSaveGroup() {
		if (!groupName.trim()) { toast.error('กรุณาระบุชื่อ'); return; }
		saving = true;
		try {
			const payload = {
				name: groupName.trim(),
				description: groupDescription || undefined,
				instructor_id: groupInstructorId || undefined,
				max_capacity: groupMaxCapacity ? parseInt(groupMaxCapacity) : undefined,
				allowed_grade_level_ids: groupAllowedGradeLevelIds.length > 0 ? groupAllowedGradeLevelIds : undefined,
			};
			if (isGroupEdit && editGroupTarget) {
				await updateActivityGroup(editGroupTarget.id, payload as any);
				toast.success('แก้ไขกิจกรรมแล้ว');
			} else {
				await createActivityGroup({ slot_id: groupSlotId, ...payload });
				toast.success('สร้างกิจกรรมแล้ว');
			}
			showGroupDialog = false;
			await loadData();
		} catch { toast.error('เกิดข้อผิดพลาด'); } finally { saving = false; }
	}

	function confirmDeleteGroup(g: ActivityGroup) { deleteGroupTarget = g; showDeleteGroupDialog = true; }

	async function handleDeleteGroup() {
		if (!deleteGroupTarget) return;
		try { await deleteActivityGroup(deleteGroupTarget.id); toast.success('ลบแล้ว'); showDeleteGroupDialog = false; await loadData(); }
		catch { toast.error('เกิดข้อผิดพลาด'); }
	}

	// ── Helpers ────────────────────────────────────────
	function gradeLevelDisplay(ids: string[] | undefined) {
		if (!ids || ids.length === 0) return 'ทุกระดับชั้น';
		// Look up in the full school grade_levels list (not just current-year filter) —
		// a catalog can target grades that don't have classrooms in the selected year.
		// Drop orphan UUIDs (referenced grades that no longer exist in the lookup).
		const names = ids
			.map((id) => structure.levels.find((g) => g.id === id)?.short_name)
			.filter((n): n is string => !!n);
		if (names.length === 0) return 'ทุกระดับชั้น';
		return names.join(', ');
	}
	async function confirmSwitchToIndependent() {
		showSwitchModeDialog = false;
		if (!editSlotTarget) return;
		try {
			await deleteAllSlotGroups(editSlotTarget.id);
			await deleteSlotTimetableEntries(editSlotTarget.id);
		} catch { toast.error('ลบข้อมูลไม่สำเร็จ'); return; }
		doSaveSlot();
	}

	async function doSaveSlot() {
		if (!isSlotEdit || !editSlotTarget) {
			toast.error('สร้าง slot ใหม่ต้องทำผ่านหลักสูตร + generate');
			return;
		}
		saving = true;
		try {
			// Semester-specific fields only. Template (name/type/periods/mode/grade)
			// lives in activity_catalog — edit at คลังกิจกรรม.
			await updateActivitySlot(editSlotTarget.id, {
				registration_type: slotRegistrationType,
			});
			toast.success('แก้ไขการลงทะเบียนแล้ว');
			showSlotDialog = false;
			await loadData();
		} catch { toast.error('เกิดข้อผิดพลาด'); } finally { saving = false; }
	}

	async function handleAssignClassroomInstructor(slotId: string, classroomId: string, instructorId: string) {
		try {
			await batchUpsertSlotClassroomAssignments(slotId, [{ classroom_id: classroomId, instructor_id: instructorId }]);
			slotClassroomAssignmentsMap[slotId] = (await listSlotClassroomAssignments(slotId)).data ?? [];
			slotClassroomAssignmentsMap = { ...slotClassroomAssignmentsMap };
			toast.success('กำหนดครูแล้ว');
		} catch { toast.error('เกิดข้อผิดพลาด'); }
	}

	async function handleAddSlotInstructor(slotId: string, userId: string) {
		try {
			await addSlotInstructor(slotId, userId);
			toast.success('เพิ่มครูแล้ว');
			slotInstructorsMap[slotId] = (await listSlotInstructors(slotId)).data ?? [];
			slotInstructorsMap = { ...slotInstructorsMap };
		} catch { toast.error('เกิดข้อผิดพลาด'); }
	}

	async function handleRemoveSlotInstructor(slotId: string, userId: string) {
		try {
			await removeSlotInstructor(slotId, userId);
			toast.success('ลบครูแล้ว');
			slotInstructorsMap[slotId] = (await listSlotInstructors(slotId)).data ?? [];
			slotInstructorsMap = { ...slotInstructorsMap };
		} catch { toast.error('เกิดข้อผิดพลาด'); }
	}

	// Slot instructor dialog
	let showSlotInstructorDialog = $state(false);
	let slotInstructorSlotId = $state('');
	let slotInstructorSelectedIds = $state<string[]>([]);
	let slotInstructorSearch = $state('');
	let addingSlotInstructors = $state(false);

	let slotInstructorCandidates = $derived(
		staffList.filter((s) => {
			if ((slotInstructorsMap[slotInstructorSlotId] ?? []).some((i) => i.user_id === s.id)) return false;
			if (slotInstructorSearch && !s.name.toLowerCase().includes(slotInstructorSearch.toLowerCase())) return false;
			return true;
		})
	);

	function toggleSlotInstructor(id: string) {
		slotInstructorSelectedIds = slotInstructorSelectedIds.includes(id)
			? slotInstructorSelectedIds.filter((x) => x !== id) : [...slotInstructorSelectedIds, id];
	}

	async function handleAddSlotInstructorsBatch() {
		if (!slotInstructorSelectedIds.length) { toast.error('กรุณาเลือกครู'); return; }
		addingSlotInstructors = true;
		try {
			for (const userId of slotInstructorSelectedIds) {
				await addSlotInstructor(slotInstructorSlotId, userId);
			}
			toast.success(`เพิ่มครู ${slotInstructorSelectedIds.length} คนแล้ว`);
			slotInstructorsMap[slotInstructorSlotId] = (await listSlotInstructors(slotInstructorSlotId)).data ?? [];
			slotInstructorsMap = { ...slotInstructorsMap };
			showSlotInstructorDialog = false;
		} catch { toast.error('เกิดข้อผิดพลาด'); }
		finally { addingSlotInstructors = false; }
	}
</script>

<svelte:head>
	<title>กิจกรรมพัฒนาผู้เรียน</title>
</svelte:head>

<div class="space-y-4 p-4">
	<!-- Header -->
	<div class="flex items-center justify-between">
		<div class="flex items-center gap-2">
			<Users class="h-5 w-5" />
			<h1 class="text-xl font-semibold">กิจกรรมพัฒนาผู้เรียน</h1>
		</div>
		{#if $can.has('activity.manage.all')}
			<div class="flex gap-2">
				<Button variant="outline" onclick={openGenerateDialog} disabled={!filterSemesterId}>
					<FolderInput class="w-4 h-4 mr-1" />
					Generate จากหลักสูตร
				</Button>
			</div>
		{/if}
	</div>

	<!-- Filters -->
	<div class="flex flex-wrap gap-3">
		<Select.Root type="single" bind:value={filterYearId}>
			<Select.Trigger class="w-52">{currentYearName}</Select.Trigger>
			<Select.Content>
				{#each structure.years as y}
					<Select.Item value={y.id}>{y.name}</Select.Item>
				{/each}
			</Select.Content>
		</Select.Root>

		<Select.Root type="single" bind:value={filterSemesterId}>
			<Select.Trigger class="w-48">{currentSemesterName}</Select.Trigger>
			<Select.Content>
				{#each yearSemesters as s}
					<Select.Item value={s.id}>{s.name}</Select.Item>
				{/each}
			</Select.Content>
		</Select.Root>

		<Select.Root type="single" bind:value={filterType}>
			<Select.Trigger class="w-48">
				{filterType ? ACTIVITY_TYPE_LABELS[filterType] : 'ทุกประเภท'}
			</Select.Trigger>
			<Select.Content>
				<Select.Item value="">ทุกประเภท</Select.Item>
				{#each Object.entries(ACTIVITY_TYPE_LABELS) as [val, label]}
					<Select.Item value={val}>{label}</Select.Item>
				{/each}
			</Select.Content>
		</Select.Root>
	</div>

	<!-- Slots & Groups -->
	{#if loading}
		<p class="text-muted-foreground text-sm">กำลังโหลด...</p>
	{:else if filteredSlots.length === 0}
		<p class="text-muted-foreground text-sm">ไม่พบช่องกิจกรรม</p>
	{:else}
		<div class="space-y-3">
			{#each filteredSlots as slot}
				{@const slotGroups = groupsForSlot(slot.id)}
				{@const expanded = expandedSlots.has(slot.id)}
				<div class="rounded-lg border bg-card">
					<!-- Slot Header -->
					<button type="button" class="w-full flex items-center gap-3 p-4 text-left hover:bg-accent/50 transition-colors" onclick={() => toggleSlot(slot.id)}>
						{#if expanded}<ChevronDown class="h-4 w-4 shrink-0" />{:else}<ChevronRight class="h-4 w-4 shrink-0" />{/if}
						<div class="flex-1 min-w-0">
							<div class="flex items-center gap-2 flex-wrap">
								<span class="font-semibold">{slot.name}</span>
								<Badge variant="secondary">{ACTIVITY_TYPE_LABELS[slot.activity_type] ?? slot.activity_type}</Badge>
								{#if slot.activity_catalog_id}
									<Badge variant="outline" class="text-[10px] border-blue-300 text-blue-700">
										📎 จากคลังกิจกรรม
									</Badge>
								{/if}
								<span class="text-sm text-muted-foreground">{gradeLevelDisplay(slot.allowed_grade_level_ids)}</span>
							</div>
							<div class="text-sm text-muted-foreground mt-0.5">
								{slotGroups.length} กิจกรรม
								· {(slotInstructorsMap[slot.id] ?? []).length} ครู
								· {slot.registration_type === 'self' ? 'นักเรียนเลือกเอง' : 'ครู/admin จัดสรร'}
								· {slot.periods_per_week} คาบ/สัปดาห์
								· {slot.scheduling_mode === 'independent' ? 'แต่ละห้องจัดเอง' : 'จัดพร้อมกันทุกห้อง'}
							</div>
						</div>
						<div class="flex items-center gap-2 shrink-0">
							{#if slot.scheduling_mode !== 'independent'}
								{#if slot.teacher_reg_open}
									<Badge variant="default">ครูลงทะเบียน</Badge>
								{/if}
								{#if slot.student_reg_open}
									<Badge>นร.ลงทะเบียน</Badge>
								{/if}
							{/if}
						</div>
					</button>

					<!-- Expanded Content -->
					{#if expanded}
						<div class="border-t px-4 pb-4">
							<!-- Slot Actions -->
							{#if $can.has('activity.manage.all')}
								<div class="flex flex-wrap gap-2 py-3">
									{#if slot.scheduling_mode !== 'independent'}
										<Button variant="outline" size="sm" onclick={() => handleToggleTeacherReg(slot)}>
											{slot.teacher_reg_open ? 'ปิดลงทะเบียนครู' : 'เปิดลงทะเบียนครู'}
										</Button>
										{#if slot.registration_type === 'self'}
											<Button variant="outline" size="sm" onclick={() => handleToggleStudentReg(slot)}>
												{slot.student_reg_open ? 'ปิดลงทะเบียนนักเรียน' : 'เปิดลงทะเบียนนักเรียน'}
											</Button>
										{/if}
									{/if}
									<Button variant="outline" size="sm" onclick={() => openEditSlot(slot)}>
										<Settings class="mr-1 h-3 w-3" />ตั้งค่า
									</Button>
								</div>
								<div class="rounded-md border bg-muted/40 px-3 py-2 text-xs text-muted-foreground mb-2">
									จัดการการเข้าร่วมของห้องเรียนผ่านหน้า
									<a href="/staff/academic/planning" class="underline hover:text-primary">Course Planning</a>
									— slot จะถูกลบอัตโนมัติเมื่อไม่มีห้องใดเข้าร่วมแล้ว
								</div>
							{/if}

							<!-- Slot Instructors (synchronized only) -->
							{#if $can.has('activity.manage.all') && slot.scheduling_mode !== 'independent'}
								{@const instrList = slotInstructorsMap[slot.id] ?? []}
								<div class="space-y-1 pb-3">
									<Label class="text-xs font-semibold text-muted-foreground">ครูผู้สอน ({instrList.length} คน)</Label>
									<div class="flex flex-wrap gap-1.5">
										{#each instrList as instr}
											<Badge variant="secondary" class="gap-1">
												{instr.instructor_name ?? '—'}
												<button type="button" class="ml-0.5 hover:text-destructive" onclick={() => handleRemoveSlotInstructor(slot.id, instr.user_id)}>×</button>
											</Badge>
										{/each}
										<Button variant="outline" size="sm" class="h-6 text-xs" onclick={() => { slotInstructorSlotId = slot.id; slotInstructorSelectedIds = []; slotInstructorSearch = ''; showSlotInstructorDialog = true; }}>
											<Plus class="h-3 w-3 mr-1" />เพิ่มครู
										</Button>
									</div>
								</div>
							{/if}

							<!-- Classroom Instructor Assignments (independent slots) -->
							{#if slot.scheduling_mode === 'independent' && $can.has('activity.manage.all')}
								{@const assignments = slotClassroomAssignmentsMap[slot.id] ?? []}
								{@const slotClassrooms = classrooms.filter((c) => {
									if (!slot.allowed_grade_level_ids || slot.allowed_grade_level_ids.length === 0) return true;
									return slot.allowed_grade_level_ids.includes(c.grade_level_id);
								})}
								<div class="space-y-2 pb-3">
									<Label class="text-xs font-semibold text-muted-foreground">ครูประจำห้อง ({assignments.length}/{slotClassrooms.length} ห้อง)</Label>
									<div class="border rounded-lg overflow-hidden">
										<table class="w-full text-sm">
											<thead>
												<tr class="bg-muted/50 text-xs text-muted-foreground">
													<th class="text-left px-3 py-1.5 font-medium">ห้องเรียน</th>
													<th class="text-left px-3 py-1.5 font-medium">ครูผู้สอน</th>
												</tr>
											</thead>
											<tbody>
												{#each slotClassrooms as classroom}
													{@const existing = assignments.find((a) => a.classroom_id === classroom.id)}
													<tr class="border-t hover:bg-accent/30">
														<td class="px-3 py-1.5 text-xs">{classroom.name}</td>
														<td class="px-3 py-1">
															<Select.Root type="single"
																value={existing?.instructor_id ?? ''}
																onValueChange={(val) => {
																	if (val) handleAssignClassroomInstructor(slot.id, classroom.id, val);
																}}
															>
																<Select.Trigger class="h-7 text-xs w-full max-w-[200px]">
																	{existing?.instructor_name ?? 'เลือกครู'}
																</Select.Trigger>
																<Select.Content class="max-h-[200px] overflow-y-auto">
																	{#each staffList as staff}
																		<Select.Item value={staff.id}>{staff.name}</Select.Item>
																	{/each}
																</Select.Content>
															</Select.Root>
														</td>
													</tr>
												{/each}
											</tbody>
										</table>
									</div>
								</div>
							{/if}

							{#if slot.scheduling_mode === 'independent'}
								<!-- Independent: ไม่ต้องสร้างกลุ่ม -->
								<p class="text-sm text-muted-foreground py-2">กิจกรรมแบบอิสระ — เรียนตามห้อง ไม่ต้องสร้างกลุ่ม</p>
							{:else}
								<!-- Groups List -->
								{#if slotGroups.length === 0}
									<p class="text-sm text-muted-foreground py-2">ยังไม่มีกิจกรรมในช่องนี้</p>
								{:else}
									<div class="divide-y rounded border">
										{#each slotGroups as g}
											<div class="flex items-center gap-3 px-3 py-2">
												<div class="flex-1 min-w-0">
													<div class="font-medium text-sm">{g.name}</div>
													<div class="text-xs text-muted-foreground">
														{g.instructor_name ?? '—'}
														· {g.member_count ?? 0}{g.max_capacity ? `/${g.max_capacity}` : ''} คน
														{#if g.allowed_grade_level_ids && g.allowed_grade_level_ids.length > 0}
															· {gradeLevelDisplay(g.allowed_grade_level_ids)}
														{/if}
													</div>
												</div>
												<div class="flex gap-1 shrink-0">
													<Button variant="ghost" size="icon" title="จัดการสมาชิก" onclick={() => goto(`/staff/academic/activities/${g.id}`)}>
														<UserCog class="h-4 w-4" />
													</Button>
													{#if $can.has('activity.manage.all') || $can.has('activity.manage.own')}
														<Button variant="ghost" size="icon" title="แก้ไข" onclick={() => openEditGroup(g)}>
															<Pencil class="h-3 w-3" />
														</Button>
													{/if}
													{#if $can.has('activity.manage.all')}
														<Button variant="ghost" size="icon" onclick={() => confirmDeleteGroup(g)}>
															<Trash2 class="h-3 w-3 text-destructive" />
														</Button>
													{/if}
												</div>
											</div>
										{/each}
									</div>
								{/if}

								<!-- Add Group Button -->
								{#if $can.has('activity.manage.all') || ($can.has('activity.manage.own') && slot.teacher_reg_open)}
									<Button variant="outline" size="sm" class="mt-3" onclick={() => openCreateGroup(slot.id)}>
										<Plus class="mr-1 h-3 w-3" />สร้างกิจกรรม
									</Button>
								{/if}
							{/if}
						</div>
					{/if}
				</div>
			{/each}
		</div>
	{/if}
</div>

<!-- Slot Edit Dialog — template fields มาจาก catalog (อ่านเท่านั้น) -->
<Dialog.Root bind:open={showSlotDialog}>
	<Dialog.Content class="max-w-lg">
		<Dialog.Header>
			<Dialog.Title>แก้ไขช่องกิจกรรม</Dialog.Title>
		</Dialog.Header>

		<div class="rounded-md border bg-muted/50 px-3 py-2 text-xs">
			📎 ช่องนี้สร้างจากคลังกิจกรรม — <b>ชื่อ / ประเภท / คาบ / โหมด / ระดับชั้น</b> แก้ที่
			<a href="/staff/academic/subjects" class="underline hover:text-primary">คลังรายวิชา → tab กิจกรรม</a>
		</div>

		<div class="space-y-3 py-2">
			<!-- Read-only summary of template fields -->
			<div class="rounded-md border p-3 space-y-2 bg-background">
				<div class="flex items-center gap-2 text-sm">
					<span class="font-semibold">{slotName}</span>
					<Badge variant="secondary">{ACTIVITY_TYPE_LABELS[slotActivityType] ?? slotActivityType}</Badge>
				</div>
				<div class="text-xs text-muted-foreground space-y-1">
					<div>
						<span class="text-foreground">ระดับชั้น:</span>
						{gradeLevelDisplay(slotAllowedGradeLevelIds)}
					</div>
					<div>
						<span class="text-foreground">คาบ/สัปดาห์:</span> {slotPeriodsPerWeek}
						· <span class="text-foreground">โหมด:</span>
						{slotSchedulingMode === 'independent' ? 'แต่ละห้องจัดเอง' : 'จัดพร้อมกันทุกห้อง'}
					</div>
					{#if slotDescription}
						<div>{slotDescription}</div>
					{/if}
				</div>
			</div>

			<!-- Editable semester-specific fields -->
			{#if slotSchedulingMode !== 'independent'}
				<div class="space-y-1">
					<Label>การรับสมาชิก</Label>
					<Select.Root type="single" bind:value={slotRegistrationType}>
						<Select.Trigger class="w-full">{slotRegistrationType === 'self' ? 'นักเรียนเลือกเอง' : 'ครู/admin จัดสรร'}</Select.Trigger>
						<Select.Content>
							<Select.Item value="assigned">ครู/admin จัดสรร</Select.Item>
							<Select.Item value="self">นักเรียนเลือกเอง</Select.Item>
						</Select.Content>
					</Select.Root>
				</div>
			{/if}
		</div>
		<Dialog.Footer>
			<Button variant="outline" onclick={() => { showSlotDialog = false; }}>ยกเลิก</Button>
			<Button onclick={handleSaveSlot} disabled={saving}>{saving ? 'กำลังบันทึก...' : 'บันทึก'}</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<!-- Group Create/Edit Dialog -->
<Dialog.Root bind:open={showGroupDialog}>
	<Dialog.Content class="max-w-md">
		<Dialog.Header>
			<Dialog.Title>{isGroupEdit ? 'แก้ไขกิจกรรม' : 'สร้างกิจกรรม'}</Dialog.Title>
		</Dialog.Header>
		<div class="space-y-3 py-2">
			<div class="space-y-1">
				<Label>ชื่อกิจกรรม <span class="text-destructive">*</span></Label>
				<Input bind:value={groupName} placeholder="เช่น ชุมนุมวิทยาศาสตร์, ฟุตบอล" />
			</div>
			<div class="space-y-1">
				<Label>ครูผู้ดูแล (หลัก)</Label>
				<Select.Root type="single" bind:value={groupInstructorId}>
					<Select.Trigger class="w-full">
						{groupInstructorId ? staffList.find((s) => s.id === groupInstructorId)?.name ?? 'เลือก...' : 'ไม่ระบุ'}
					</Select.Trigger>
					<Select.Content class="max-h-56 overflow-y-auto">
						<Select.Item value="">ไม่ระบุ</Select.Item>
						{#each staffList as s}<Select.Item value={s.id}>{s.name}</Select.Item>{/each}
					</Select.Content>
				</Select.Root>
				<p class="text-xs text-muted-foreground">ครูผู้ช่วยเพิ่มได้ในหน้าจัดการสมาชิก</p>
			</div>
			<div class="space-y-1">
				<Label>จำนวนรับสูงสุด</Label>
				<Input type="number" min="1" placeholder="ไม่จำกัด" bind:value={groupMaxCapacity} />
			</div>
			<div class="space-y-1">
				<Label>ชั้นที่รับ (ว่าง = ตามช่องกิจกรรม)</Label>
				<div class="flex flex-wrap gap-1.5">
					{#each yearGradeLevels as level}
						{@const selected = groupAllowedGradeLevelIds.includes(level.id)}
						<button type="button"
							class="rounded border px-2 py-1 text-xs transition-colors {selected ? 'bg-primary text-primary-foreground border-primary' : 'bg-background hover:bg-accent border-input'}"
							onclick={() => toggleGroupGrade(level.id)}>{level.short_name}</button>
					{/each}
				</div>
			</div>
			<div class="space-y-1">
				<Label>คำอธิบาย</Label>
				<Textarea bind:value={groupDescription} placeholder="รายละเอียด..." rows={2} />
			</div>
		</div>
		<Dialog.Footer>
			<Button variant="outline" onclick={() => { showGroupDialog = false; }}>ยกเลิก</Button>
			<Button onclick={handleSaveGroup} disabled={saving}>{saving ? 'กำลังบันทึก...' : isGroupEdit ? 'บันทึก' : 'สร้าง'}</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<!-- Add Slot Instructor Dialog (Multi-select) -->
<Dialog.Root bind:open={showSlotInstructorDialog}>
	<Dialog.Content class="max-w-md">
		<Dialog.Header>
			<Dialog.Title>เพิ่มครูผู้สอน</Dialog.Title>
			<Dialog.Description>เลือกครูที่จะสอนในช่องกิจกรรมนี้{#if slotInstructorSelectedIds.length > 0} · เลือก {slotInstructorSelectedIds.length} คน{/if}</Dialog.Description>
		</Dialog.Header>
		<div class="space-y-3 py-2">
			<div class="relative">
				<Input class="pl-8" placeholder="ค้นหาครู..." bind:value={slotInstructorSearch} />
			</div>
			<div class="max-h-64 overflow-y-auto divide-y rounded border">
				{#each slotInstructorCandidates as s}
					{@const checked = slotInstructorSelectedIds.includes(s.id)}
					<button type="button" class="flex w-full items-center gap-3 px-3 py-2 text-sm hover:bg-accent text-left" onclick={() => toggleSlotInstructor(s.id)}>
						<div class="flex h-4 w-4 items-center justify-center rounded border {checked ? 'bg-primary border-primary' : 'border-input'}">
							{#if checked}<span class="text-primary-foreground text-xs">✓</span>{/if}
						</div>
						<span>{s.name}</span>
					</button>
				{:else}
					<div class="px-3 py-4 text-sm text-muted-foreground text-center">ไม่พบครู</div>
				{/each}
			</div>
		</div>
		<Dialog.Footer>
			<Button variant="outline" onclick={() => { showSlotInstructorDialog = false; }}>ยกเลิก</Button>
			<Button onclick={handleAddSlotInstructorsBatch} disabled={addingSlotInstructors || !slotInstructorSelectedIds.length}>
				{addingSlotInstructors ? 'กำลังเพิ่ม...' : `เพิ่ม ${slotInstructorSelectedIds.length} คน`}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<!-- Delete Slot Dialog -->
<Dialog.Root bind:open={showDeleteSlotDialog}>
	<Dialog.Content class="max-w-sm">
		<Dialog.Header>
			<Dialog.Title>ยืนยันการลบ</Dialog.Title>
			<Dialog.Description>
				ลบช่อง "<strong>{deleteSlotTarget?.name}</strong>"
				รวมถึงกิจกรรม สมาชิก และรายการในตารางสอนทั้งหมด?
			</Dialog.Description>
		</Dialog.Header>
		<div class="rounded-md border bg-amber-50 text-amber-900 px-3 py-2 text-xs">
			⚠️ ช่องนี้มาจากคลังกิจกรรมในหลักสูตร — หากลบแล้วกด "สร้างอัตโนมัติ" ที่หน้า planning ก็จะถูกสร้างกลับมา
			<br />หากต้องการปิดแค่ภาคเรียนนี้ โปรดถอนออกจากหลักสูตรแทน
		</div>
		<Dialog.Footer>
			<Button variant="outline" onclick={() => { showDeleteSlotDialog = false; }}>ยกเลิก</Button>
			<Button variant="destructive" onclick={handleDeleteSlot}>ลบต่อไป</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<!-- Delete Group Dialog -->
<Dialog.Root bind:open={showDeleteGroupDialog}>
	<Dialog.Content class="max-w-sm">
		<Dialog.Header>
			<Dialog.Title>ยืนยันการลบ</Dialog.Title>
			<Dialog.Description>ลบกิจกรรม "<strong>{deleteGroupTarget?.name}</strong>"?</Dialog.Description>
		</Dialog.Header>
		<Dialog.Footer>
			<Button variant="outline" onclick={() => { showDeleteGroupDialog = false; }}>ยกเลิก</Button>
			<Button variant="destructive" onclick={handleDeleteGroup}>ลบ</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<!-- Generate from Plan Dialog -->
<Dialog.Root bind:open={showGenerateDialog}>
	<Dialog.Content class="max-w-md">
		<Dialog.Header>
			<Dialog.Title>Generate กิจกรรมจากหลักสูตร</Dialog.Title>
			<Dialog.Description>
				สร้าง activity slots ตามแม่แบบในหลักสูตรสำหรับภาคเรียนที่เลือก
				(ข้ามกิจกรรมที่มีอยู่แล้ว)
			</Dialog.Description>
		</Dialog.Header>

		<div class="space-y-3 py-2">
			<div class="space-y-1">
				<Label>หลักสูตร *</Label>
				<Select.Root type="single" bind:value={generateVersionId}>
					<Select.Trigger class="w-full">
						{planVersions.find((v) => v.id === generateVersionId)?.version_name ?? 'เลือกหลักสูตร'}
					</Select.Trigger>
					<Select.Content>
						{#each planVersions as v}
							<Select.Item value={v.id}>{v.version_name}</Select.Item>
						{/each}
					</Select.Content>
				</Select.Root>
			</div>
		</div>

		<Dialog.Footer>
			<Button variant="outline" onclick={() => { showGenerateDialog = false; }}>ยกเลิก</Button>
			<Button onclick={handleGenerate} disabled={generating || !generateVersionId}>
				{generating ? 'กำลังสร้าง...' : 'Generate'}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<!-- Switch Mode Confirmation Dialog -->
<Dialog.Root bind:open={showSwitchModeDialog}>
	<Dialog.Content class="max-w-sm">
		<Dialog.Header>
			<Dialog.Title>เปลี่ยนเป็นแบบอิสระ</Dialog.Title>
			<Dialog.Description>
				จะลบกิจกรรม <strong>{switchModeGroupCount} กลุ่ม</strong>
				{#if switchModeMemberCount > 0}
					({switchModeMemberCount} สมาชิก)
				{/if}
				และรายการในตารางสอนทั้งหมด ดำเนินการต่อ?
			</Dialog.Description>
		</Dialog.Header>
		<Dialog.Footer>
			<Button variant="outline" onclick={() => { showSwitchModeDialog = false; }}>ยกเลิก</Button>
			<Button variant="destructive" onclick={confirmSwitchToIndependent}>ลบและเปลี่ยน</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
