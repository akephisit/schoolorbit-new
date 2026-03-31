<script lang="ts">
	import { onMount } from 'svelte';
	import {
		getAcademicStructure,
		listActivitySlots,
		createActivitySlot,
		updateActivitySlot,
		deleteActivitySlot,
		listActivityGroups,
		createActivityGroup,
		deleteActivityGroup,
		listClassrooms,
		ACTIVITY_TYPE_LABELS,
		type ActivitySlot,
		type ActivityGroup,
		type AcademicStructureData,
		type Classroom,
		type GradeLevel
	} from '$lib/api/academic';
	import { listPeriods, type AcademicPeriod } from '$lib/api/timetable';
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
	import { Users, Plus, Trash2, Check, ChevronsUpDown, UserCog, ChevronDown, ChevronRight, Settings } from 'lucide-svelte';
	import { can } from '$lib/stores/permissions';
	import { goto } from '$app/navigation';

	// ── State ──────────────────────────────────────────
	let loading = $state(true);
	let saving = $state(false);

	let structure = $state<AcademicStructureData>({ years: [], semesters: [], levels: [] });
	let classrooms = $state<Classroom[]>([]);
	let staffList = $state<StaffLookupItem[]>([]);
	let periods = $state<AcademicPeriod[]>([]);
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
	let slotDayOfWeek = $state('');
	let slotPeriodIds = $state<string[]>([]);
	let slotRegistrationType = $state('assigned');
	let slotAllowedGradeLevelIds = $state<string[]>([]);

	// Group Dialog (สร้างชุมนุมภายใต้ slot)
	let showGroupDialog = $state(false);
	let groupSlotId = $state('');
	let groupName = $state('');
	let groupDescription = $state('');
	let groupInstructorId = $state('');
	let groupMaxCapacity = $state('');

	// Delete
	let deleteSlotTarget = $state<ActivitySlot | null>(null);
	let showDeleteSlotDialog = $state(false);

	const DAYS = [
		{ value: 'MON', label: 'จันทร์' }, { value: 'TUE', label: 'อังคาร' },
		{ value: 'WED', label: 'พุธ' },    { value: 'THU', label: 'พฤหัสบดี' },
		{ value: 'FRI', label: 'ศุกร์' }
	];

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
	let yearGradeLevels = $derived(() => {
		const gradeIds = [...new Set(classrooms.map((c) => c.grade_level_id))];
		return structure.levels
			.filter((l) => gradeIds.includes(l.id))
			.sort((a, b) => (a.code > b.code ? 1 : -1));
	});

	let slotSemesterName = $derived(
		structure.semesters.find((s) => s.id === slotSemesterId)?.name ?? 'เลือก...'
	);
	let slotDayLabel = $derived(DAYS.find((d) => d.value === slotDayOfWeek)?.label ?? 'เลือกวัน...');

	// ── Load ───────────────────────────────────────────
	onMount(async () => {
		const [structRes, staffRes, periodsRes] = await Promise.all([
			getAcademicStructure(),
			lookupStaff({ activeOnly: true, limit: 1000 }),
			listPeriods()
		]);
		structure = structRes.data;
		staffList = staffRes;
		periods = periodsRes.data ?? [];

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
	}

	$effect(() => {
		if (filterYearId) {
			loadClassrooms();
			// Auto-select first semester of year
			const sems = structure.semesters.filter((s) => s.academic_year_id === filterYearId);
			const activeSem = sems.find((s) => s.is_active) ?? sems[0];
			if (activeSem && activeSem.id !== filterSemesterId) {
				filterSemesterId = activeSem.id;
			}
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

	// ── Slot Dialog ───────────────────────────────────
	function openCreateSlot() {
		slotName = ''; slotDescription = ''; slotActivityType = 'club';
		slotSemesterId = filterSemesterId; slotDayOfWeek = '';
		slotPeriodIds = []; slotRegistrationType = 'assigned';
		slotAllowedGradeLevelIds = [];
		isSlotEdit = false; editSlotTarget = null;
		showSlotDialog = true;
	}

	function openEditSlot(s: ActivitySlot) {
		slotName = s.name; slotDescription = s.description ?? '';
		slotActivityType = s.activity_type; slotSemesterId = s.semester_id;
		slotDayOfWeek = s.day_of_week ?? ''; slotPeriodIds = s.period_ids ?? [];
		slotRegistrationType = s.registration_type;
		slotAllowedGradeLevelIds = s.allowed_grade_level_ids ?? [];
		isSlotEdit = true; editSlotTarget = s;
		showSlotDialog = true;
	}

	function toggleSlotPeriod(id: string) {
		slotPeriodIds = slotPeriodIds.includes(id)
			? slotPeriodIds.filter((x) => x !== id) : [...slotPeriodIds, id];
	}

	function toggleSlotGrade(id: string) {
		slotAllowedGradeLevelIds = slotAllowedGradeLevelIds.includes(id)
			? slotAllowedGradeLevelIds.filter((x) => x !== id) : [...slotAllowedGradeLevelIds, id];
	}

	async function handleSaveSlot() {
		if (!slotName.trim()) { toast.error('กรุณาระบุชื่อ'); return; }
		saving = true;
		try {
			if (isSlotEdit && editSlotTarget) {
				await updateActivitySlot(editSlotTarget.id, {
					name: slotName.trim(),
					description: slotDescription || undefined,
					activity_type: slotActivityType as any,
					allowed_grade_level_ids: slotAllowedGradeLevelIds.length > 0 ? slotAllowedGradeLevelIds : undefined,
					day_of_week: slotDayOfWeek || undefined,
					period_ids: slotPeriodIds.length > 0 ? slotPeriodIds : undefined,
					registration_type: slotRegistrationType as any,
				} as any);
				toast.success('แก้ไขช่องกิจกรรมแล้ว');
			} else {
				await createActivitySlot({
					name: slotName.trim(),
					description: slotDescription || undefined,
					activity_type: slotActivityType,
					semester_id: slotSemesterId,
					allowed_grade_level_ids: slotAllowedGradeLevelIds.length > 0 ? slotAllowedGradeLevelIds : undefined,
					day_of_week: slotDayOfWeek || undefined,
					period_ids: slotPeriodIds.length > 0 ? slotPeriodIds : undefined,
					registration_type: slotRegistrationType || undefined,
				});
				toast.success('สร้างช่องกิจกรรมแล้ว');
			}
			showSlotDialog = false;
			await loadData();
		} catch { toast.error('เกิดข้อผิดพลาด'); } finally { saving = false; }
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
		try { await deleteActivitySlot(deleteSlotTarget.id); toast.success('ลบแล้ว'); showDeleteSlotDialog = false; await loadData(); }
		catch { toast.error('เกิดข้อผิดพลาด'); }
	}

	// ── Group Dialog ──────────────────────────────────
	function openCreateGroup(slotId: string) {
		groupSlotId = slotId; groupName = ''; groupDescription = '';
		groupInstructorId = ''; groupMaxCapacity = '';
		showGroupDialog = true;
	}

	async function handleSaveGroup() {
		if (!groupName.trim()) { toast.error('กรุณาระบุชื่อ'); return; }
		saving = true;
		try {
			await createActivityGroup({
				slot_id: groupSlotId,
				name: groupName.trim(),
				description: groupDescription || undefined,
				instructor_id: groupInstructorId || undefined,
				max_capacity: groupMaxCapacity ? parseInt(groupMaxCapacity) : undefined,
			});
			toast.success('สร้างกิจกรรมแล้ว');
			showGroupDialog = false;
			await loadData();
		} catch { toast.error('เกิดข้อผิดพลาด'); } finally { saving = false; }
	}

	async function handleDeleteGroup(g: ActivityGroup) {
		if (!confirm(`ลบ "${g.name}" ?`)) return;
		try { await deleteActivityGroup(g.id); toast.success('ลบแล้ว'); await loadData(); }
		catch { toast.error('เกิดข้อผิดพลาด'); }
	}

	// ── Helpers ────────────────────────────────────────
	function gradeLevelDisplay(ids: string[] | undefined) {
		if (!ids || ids.length === 0) return 'ทุกระดับชั้น';
		return ids.map((id) => yearGradeLevels().find((g) => g.id === id)?.short_name ?? id).join(', ');
	}
	function dayLabel(d?: string) { return DAYS.find((x) => x.value === d)?.label ?? '—'; }
	function periodNames(ids?: string[]) {
		if (!ids || ids.length === 0) return '—';
		return ids.map((id) => (periods as any[]).find((p) => p.id === id)?.name ?? '?').join(', ');
	}
</script>

<div class="space-y-4 p-4">
	<!-- Header -->
	<div class="flex items-center justify-between">
		<div class="flex items-center gap-2">
			<Users class="h-5 w-5" />
			<h1 class="text-xl font-semibold">กิจกรรมพัฒนาผู้เรียน</h1>
		</div>
		{#if $can.has('activity.manage.all')}
			<Button onclick={openCreateSlot}>
				<Plus class="mr-1 h-4 w-4" />สร้างช่องกิจกรรม
			</Button>
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
								<span class="text-sm text-muted-foreground">{gradeLevelDisplay(slot.allowed_grade_level_ids)}</span>
							</div>
							<div class="text-sm text-muted-foreground mt-0.5">
								{dayLabel(slot.day_of_week)} คาบ {periodNames(slot.period_ids)}
								· {slotGroups.length} กิจกรรม
								· {slot.registration_type === 'self' ? 'นักเรียนเลือกเอง' : 'ครู/admin จัดสรร'}
							</div>
						</div>
						<div class="flex items-center gap-2 shrink-0">
							{#if slot.teacher_reg_open}
								<Badge variant="default">ครูลงทะเบียน</Badge>
							{/if}
							{#if slot.student_reg_open}
								<Badge>นร.ลงทะเบียน</Badge>
							{/if}
						</div>
					</button>

					<!-- Expanded Content -->
					{#if expanded}
						<div class="border-t px-4 pb-4">
							<!-- Slot Actions -->
							{#if $can.has('activity.manage.all')}
								<div class="flex flex-wrap gap-2 py-3">
									<Button variant="outline" size="sm" onclick={() => handleToggleTeacherReg(slot)}>
										{slot.teacher_reg_open ? 'ปิดลงทะเบียนครู' : 'เปิดลงทะเบียนครู'}
									</Button>
									{#if slot.registration_type === 'self'}
										<Button variant="outline" size="sm" onclick={() => handleToggleStudentReg(slot)}>
											{slot.student_reg_open ? 'ปิดลงทะเบียนนักเรียน' : 'เปิดลงทะเบียนนักเรียน'}
										</Button>
									{/if}
									<Button variant="outline" size="sm" onclick={() => openEditSlot(slot)}>
										<Settings class="mr-1 h-3 w-3" />ตั้งค่า
									</Button>
									<Button variant="outline" size="sm" class="text-destructive" onclick={() => confirmDeleteSlot(slot)}>
										<Trash2 class="mr-1 h-3 w-3" />ลบช่อง
									</Button>
								</div>
							{/if}

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
												</div>
											</div>
											<div class="flex gap-1 shrink-0">
												<Button variant="ghost" size="icon" title="จัดการ" onclick={() => goto(`/staff/academic/activities/${g.id}`)}>
													<UserCog class="h-4 w-4" />
												</Button>
												{#if $can.has('activity.manage.all')}
													<Button variant="ghost" size="icon" onclick={() => handleDeleteGroup(g)}>
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
						</div>
					{/if}
				</div>
			{/each}
		</div>
	{/if}
</div>

<!-- Slot Create/Edit Dialog -->
<Dialog.Root bind:open={showSlotDialog}>
	<Dialog.Content class="max-w-lg">
		<Dialog.Header>
			<Dialog.Title>{isSlotEdit ? 'แก้ไขช่องกิจกรรม' : 'สร้างช่องกิจกรรม'}</Dialog.Title>
		</Dialog.Header>
		<div class="space-y-4 py-2">
			<div class="space-y-1">
				<Label>ชื่อช่อง <span class="text-destructive">*</span></Label>
				<Input bind:value={slotName} placeholder="เช่น ชุมนุม ม.ต้น, ลูกเสือ ม.ปลาย" />
			</div>
			<div class="grid grid-cols-2 gap-3">
				<div class="space-y-1">
					<Label>ประเภท</Label>
					<Select.Root type="single" bind:value={slotActivityType}>
						<Select.Trigger class="w-full">{ACTIVITY_TYPE_LABELS[slotActivityType] ?? slotActivityType}</Select.Trigger>
						<Select.Content>
							{#each Object.entries(ACTIVITY_TYPE_LABELS) as [val, label]}
								<Select.Item value={val}>{label}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
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
			</div>
			{#if !isSlotEdit}
				<div class="space-y-1">
					<Label>ภาคเรียน <span class="text-destructive">*</span></Label>
					<Select.Root type="single" bind:value={slotSemesterId}>
						<Select.Trigger class="w-full">{slotSemesterName}</Select.Trigger>
						<Select.Content>
							{#each yearSemesters as s}
								<Select.Item value={s.id}>{s.name}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
			{/if}
			<div class="grid grid-cols-2 gap-3">
				<div class="space-y-1">
					<Label>วันที่สอน</Label>
					<Select.Root type="single" bind:value={slotDayOfWeek}>
						<Select.Trigger class="w-full">{slotDayLabel}</Select.Trigger>
						<Select.Content>
							{#each DAYS as d}<Select.Item value={d.value}>{d.label}</Select.Item>{/each}
						</Select.Content>
					</Select.Root>
				</div>
				<div class="space-y-1">
					<Label>คาบเรียน</Label>
					<div class="flex flex-wrap gap-1.5">
						{#each (periods as any[]).filter((p) => p.type !== 'BREAK') as p}
							{@const selected = slotPeriodIds.includes(p.id)}
							<button type="button"
								class="rounded border px-2 py-1 text-xs transition-colors {selected ? 'bg-primary text-primary-foreground border-primary' : 'bg-background hover:bg-accent border-input'}"
								onclick={() => toggleSlotPeriod(p.id)}>{p.name}</button>
						{/each}
					</div>
				</div>
			</div>
			<div class="space-y-1">
				<Label>ระดับชั้นที่รับ (ว่าง = ทุกระดับ)</Label>
				<Popover.Root>
					<Popover.Trigger class="w-full">
						<Button variant="outline" class="w-full justify-between font-normal">
							{#if slotAllowedGradeLevelIds.length > 0}
								{slotAllowedGradeLevelIds.map((id) => yearGradeLevels().find((l) => l.id === id)?.short_name ?? id).join(', ')}
							{:else}
								<span class="text-muted-foreground">ทุกระดับชั้น</span>
							{/if}
							<ChevronsUpDown class="ml-2 h-4 w-4 shrink-0 opacity-50" />
						</Button>
					</Popover.Trigger>
					<Popover.Content class="w-[--radix-popover-trigger-width] p-1 max-h-56 overflow-y-auto">
						{#each yearGradeLevels() as level}
							{@const checked = slotAllowedGradeLevelIds.includes(level.id)}
							<button type="button" class="flex w-full items-center gap-2 rounded px-2 py-1.5 text-sm hover:bg-accent"
								onclick={() => toggleSlotGrade(level.id)}>
								<Check class="h-4 w-4 {checked ? 'opacity-100' : 'opacity-0'}" />
								{level.name}
							</button>
						{/each}
					</Popover.Content>
				</Popover.Root>
			</div>
			<div class="space-y-1">
				<Label>คำอธิบาย</Label>
				<Textarea bind:value={slotDescription} placeholder="รายละเอียดเพิ่มเติม..." rows={2} />
			</div>
		</div>
		<Dialog.Footer>
			<Button variant="outline" onclick={() => { showSlotDialog = false; }}>ยกเลิก</Button>
			<Button onclick={handleSaveSlot} disabled={saving}>{saving ? 'กำลังบันทึก...' : 'บันทึก'}</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<!-- Group Create Dialog -->
<Dialog.Root bind:open={showGroupDialog}>
	<Dialog.Content class="max-w-sm">
		<Dialog.Header>
			<Dialog.Title>สร้างกิจกรรม</Dialog.Title>
		</Dialog.Header>
		<div class="space-y-3 py-2">
			<div class="space-y-1">
				<Label>ชื่อกิจกรรม <span class="text-destructive">*</span></Label>
				<Input bind:value={groupName} placeholder="เช่น ชุมนุมวิทยาศาสตร์, ฟุตบอล" />
			</div>
			<div class="space-y-1">
				<Label>ครูผู้ดูแล</Label>
				<Select.Root type="single" bind:value={groupInstructorId}>
					<Select.Trigger class="w-full">
						{groupInstructorId ? staffList.find((s) => s.id === groupInstructorId)?.name ?? 'เลือก...' : 'ไม่ระบุ'}
					</Select.Trigger>
					<Select.Content class="max-h-56 overflow-y-auto">
						<Select.Item value="">ไม่ระบุ</Select.Item>
						{#each staffList as s}<Select.Item value={s.id}>{s.name}</Select.Item>{/each}
					</Select.Content>
				</Select.Root>
			</div>
			<div class="space-y-1">
				<Label>จำนวนรับสูงสุด</Label>
				<Input type="number" min="1" placeholder="ไม่จำกัด" bind:value={groupMaxCapacity} />
			</div>
			<div class="space-y-1">
				<Label>คำอธิบาย</Label>
				<Textarea bind:value={groupDescription} placeholder="รายละเอียด..." rows={2} />
			</div>
		</div>
		<Dialog.Footer>
			<Button variant="outline" onclick={() => { showGroupDialog = false; }}>ยกเลิก</Button>
			<Button onclick={handleSaveGroup} disabled={saving}>{saving ? 'กำลังบันทึก...' : 'สร้าง'}</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<!-- Delete Slot Dialog -->
<Dialog.Root bind:open={showDeleteSlotDialog}>
	<Dialog.Content class="max-w-sm">
		<Dialog.Header>
			<Dialog.Title>ยืนยันการลบ</Dialog.Title>
			<Dialog.Description>ลบช่อง "<strong>{deleteSlotTarget?.name}</strong>" และกิจกรรมทั้งหมดภายใน?</Dialog.Description>
		</Dialog.Header>
		<Dialog.Footer>
			<Button variant="outline" onclick={() => { showDeleteSlotDialog = false; }}>ยกเลิก</Button>
			<Button variant="destructive" onclick={handleDeleteSlot}>ลบ</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
