<script lang="ts">
	import { onMount } from 'svelte';
	import {
		getAcademicStructure,
		listActivityGroups,
		createActivityGroup,
		updateActivityGroup,
		deleteActivityGroup,
		ACTIVITY_TYPE_LABELS,
		type ActivityGroup,
		type AcademicStructureData
	} from '$lib/api/academic';
	import { lookupStaff, type StaffLookupItem } from '$lib/api/lookup';
	import { lookupGradeLevels, type LookupItem } from '$lib/api/academic';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Textarea } from '$lib/components/ui/textarea';
	import { Badge } from '$lib/components/ui/badge';
	import * as Table from '$lib/components/ui/table';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Select from '$lib/components/ui/select';
	import * as Popover from '$lib/components/ui/popover';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import { toast } from 'svelte-sonner';
	import { Users, Plus, Pencil, Trash2, Search, Check, ChevronsUpDown, UserCog } from 'lucide-svelte';
	import { can } from '$lib/stores/permissions';
	import { goto } from '$app/navigation';

	let { data } = $props();

	// ── State ──────────────────────────────────────────
	let loading = $state(true);
	let saving = $state(false);

	let structure = $state<AcademicStructureData>({ years: [], semesters: [], levels: [] });
	let gradeLevels = $state<LookupItem[]>([]);
	let staffList = $state<StaffLookupItem[]>([]);
	let groups = $state<ActivityGroup[]>([]);

	// Filters
	let filterSemesterId = $state('');
	let filterType = $state('');
	let filterSearch = $state('');

	// Dialog
	let showDialog = $state(false);
	let isEdit = $state(false);
	let deleteTarget = $state<ActivityGroup | null>(null);

	function emptyGroup(): Omit<ActivityGroup, 'id' | 'is_active' | 'created_at' | 'instructor_name' | 'member_count' | 'semester_name'> {
		return {
			name: '',
			description: '',
			activity_type: 'club',
			semester_id: filterSemesterId || '',
			instructor_id: undefined,
			registration_type: 'assigned',
			max_capacity: undefined,
			registration_open: false,
			allowed_grade_level_ids: []
		};
	}

	let form = $state(emptyGroup());

	// ── Computed ───────────────────────────────────────
	let filteredGroups = $derived(
		groups.filter((g) => {
			if (filterType && g.activity_type !== filterType) return false;
			if (filterSearch && !g.name.toLowerCase().includes(filterSearch.toLowerCase())) return false;
			return true;
		})
	);

	let currentSemester = $derived(
		structure.semesters.find((s) => s.id === filterSemesterId)
	);

	// ── Load ───────────────────────────────────────────
	onMount(async () => {
		const [structRes, levelsRes, staffRes] = await Promise.all([
			getAcademicStructure(),
			lookupGradeLevels({ current_year: false }),
			lookupStaff({ activeOnly: true, limit: 1000 })
		]);
		structure = structRes.data;
		gradeLevels = levelsRes.data;
		staffList = staffRes;

		// default to current semester
		const current = structure.semesters.find((s) => s.is_active);
		if (current) filterSemesterId = current.id;

		await loadGroups();
		loading = false;
	});

	async function loadGroups() {
		if (!filterSemesterId) return;
		const res = await listActivityGroups({ semester_id: filterSemesterId });
		groups = res.data ?? [];
	}

	$effect(() => {
		if (filterSemesterId) loadGroups();
	});

	// ── Dialog helpers ─────────────────────────────────
	function openCreate() {
		form = emptyGroup();
		isEdit = false;
		showDialog = true;
	}

	function openEdit(g: ActivityGroup) {
		form = {
			name: g.name,
			description: g.description ?? '',
			activity_type: g.activity_type,
			semester_id: g.semester_id,
			instructor_id: g.instructor_id,
			registration_type: g.registration_type,
			max_capacity: g.max_capacity,
			registration_open: g.registration_open,
			allowed_grade_level_ids: g.allowed_grade_level_ids ?? []
		};
		isEdit = true;
		showDialog = true;
	}

	function toggleGradeLevel(id: string) {
		const ids = form.allowed_grade_level_ids ?? [];
		form.allowed_grade_level_ids = ids.includes(id) ? ids.filter((x) => x !== id) : [...ids, id];
	}

	// ── Save ───────────────────────────────────────────
	async function handleSave() {
		if (!form.name.trim()) { toast.error('กรุณาระบุชื่อกลุ่มกิจกรรม'); return; }
		if (!form.semester_id) { toast.error('กรุณาเลือกภาคเรียน'); return; }

		saving = true;
		try {
			if (isEdit) {
				await updateActivityGroup((deleteTarget as any)?.id ?? '', form);
				toast.success('แก้ไขกลุ่มกิจกรรมแล้ว');
			} else {
				await createActivityGroup(form);
				toast.success('สร้างกลุ่มกิจกรรมแล้ว');
			}
			showDialog = false;
			await loadGroups();
		} catch {
			toast.error('เกิดข้อผิดพลาด');
		} finally {
			saving = false;
		}
	}

	// ── Delete ─────────────────────────────────────────
	let showDeleteDialog = $state(false);
	let editTarget = $state<ActivityGroup | null>(null);

	function confirmDelete(g: ActivityGroup) {
		deleteTarget = g;
		showDeleteDialog = true;
	}

	async function handleDelete() {
		if (!deleteTarget) return;
		try {
			await deleteActivityGroup(deleteTarget.id);
			toast.success('ลบกลุ่มกิจกรรมแล้ว');
			showDeleteDialog = false;
			deleteTarget = null;
			await loadGroups();
		} catch {
			toast.error('เกิดข้อผิดพลาด');
		}
	}

	function openEditDialog(g: ActivityGroup) {
		editTarget = g;
		openEdit(g);
	}

	// ── Helpers ────────────────────────────────────────
	function activityTypeBadgeVariant(type: string) {
		return type === 'scout' ? 'default'
			: type === 'club' ? 'secondary'
			: type === 'guidance' ? 'outline'
			: 'outline';
	}

	function gradeLevelDisplay(ids: string[] | undefined) {
		if (!ids || ids.length === 0) return 'ทุกระดับชั้น';
		return ids.map((id) => {
			const l = gradeLevels.find((g) => g.id === id);
			return l?.short_name ?? l?.code ?? id;
		}).join(', ');
	}
</script>

<div class="space-y-4 p-4">
	<!-- Header -->
	<div class="flex items-center justify-between">
		<div class="flex items-center gap-2">
			<Users class="h-5 w-5" />
			<h1 class="text-xl font-semibold">กิจกรรมพัฒนาผู้เรียน</h1>
		</div>
		{#if $can('activity.manage.all') || $can('activity.manage.own')}
			<Button onclick={openCreate}>
				<Plus class="mr-1 h-4 w-4" />
				สร้างกลุ่มกิจกรรม
			</Button>
		{/if}
	</div>

	<!-- Filters -->
	<div class="flex flex-wrap gap-3">
		<!-- Semester -->
		<Select.Root
			value={filterSemesterId}
			onValueChange={(v) => { filterSemesterId = v; }}
		>
			<Select.Trigger class="w-52">
				{currentSemester?.name ?? 'เลือกภาคเรียน'}
			</Select.Trigger>
			<Select.Content>
				{#each structure.semesters as s}
					<Select.Item value={s.id}>{s.name}</Select.Item>
				{/each}
			</Select.Content>
		</Select.Root>

		<!-- Type -->
		<Select.Root value={filterType} onValueChange={(v) => { filterType = v === 'all' ? '' : v; }}>
			<Select.Trigger class="w-48">
				{filterType ? ACTIVITY_TYPE_LABELS[filterType] : 'ทุกประเภท'}
			</Select.Trigger>
			<Select.Content>
				<Select.Item value="all">ทุกประเภท</Select.Item>
				{#each Object.entries(ACTIVITY_TYPE_LABELS) as [val, label]}
					<Select.Item value={val}>{label}</Select.Item>
				{/each}
			</Select.Content>
		</Select.Root>

		<!-- Search -->
		<div class="relative">
			<Search class="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
			<Input
				class="pl-8 w-56"
				placeholder="ค้นหาชื่อกลุ่ม..."
				bind:value={filterSearch}
			/>
		</div>
	</div>

	<!-- Table -->
	{#if loading}
		<p class="text-muted-foreground text-sm">กำลังโหลด...</p>
	{:else if filteredGroups.length === 0}
		<p class="text-muted-foreground text-sm">ไม่พบกลุ่มกิจกรรม</p>
	{:else}
		<Table.Root>
			<Table.Header>
				<Table.Row>
					<Table.Head>ชื่อกลุ่มกิจกรรม</Table.Head>
					<Table.Head>ประเภท</Table.Head>
					<Table.Head>ครูที่ดูแล</Table.Head>
					<Table.Head>ระดับชั้น</Table.Head>
					<Table.Head class="text-center">สมาชิก</Table.Head>
					<Table.Head class="text-center">รับสมัคร</Table.Head>
					<Table.Head></Table.Head>
				</Table.Row>
			</Table.Header>
			<Table.Body>
				{#each filteredGroups as g}
					<Table.Row>
						<Table.Cell class="font-medium">{g.name}</Table.Cell>
						<Table.Cell>
							<Badge variant={activityTypeBadgeVariant(g.activity_type)}>
								{ACTIVITY_TYPE_LABELS[g.activity_type] ?? g.activity_type}
							</Badge>
						</Table.Cell>
						<Table.Cell class="text-sm text-muted-foreground">
							{g.instructor_name ?? '—'}
						</Table.Cell>
						<Table.Cell class="text-sm">
							{gradeLevelDisplay(g.allowed_grade_level_ids)}
						</Table.Cell>
						<Table.Cell class="text-center text-sm">
							{g.member_count ?? 0}{g.max_capacity ? `/${g.max_capacity}` : ''}
						</Table.Cell>
						<Table.Cell class="text-center">
							{#if g.registration_type === 'self'}
								<Badge variant={g.registration_open ? 'default' : 'outline'}>
									{g.registration_open ? 'เปิด' : 'ปิด'}
								</Badge>
							{:else}
								<span class="text-xs text-muted-foreground">ครูจัดสรร</span>
							{/if}
						</Table.Cell>
						<Table.Cell>
							<div class="flex gap-1 justify-end">
								<Button
									variant="ghost"
									size="icon"
									title="จัดการสมาชิก"
									onclick={() => goto(`/staff/academic/activities/${g.id}`)}
								>
									<UserCog class="h-4 w-4" />
								</Button>
								{#if $can('activity.manage.all') || $can('activity.manage.own')}
									<Button variant="ghost" size="icon" onclick={() => openEditDialog(g)}>
										<Pencil class="h-4 w-4" />
									</Button>
									<Button variant="ghost" size="icon" onclick={() => confirmDelete(g)}>
										<Trash2 class="h-4 w-4 text-destructive" />
									</Button>
								{/if}
							</div>
						</Table.Cell>
					</Table.Row>
				{/each}
			</Table.Body>
		</Table.Root>
	{/if}
</div>

<!-- Create/Edit Dialog -->
<Dialog.Root bind:open={showDialog}>
	<Dialog.Content class="max-w-lg">
		<Dialog.Header>
			<Dialog.Title>{isEdit ? 'แก้ไขกลุ่มกิจกรรม' : 'สร้างกลุ่มกิจกรรม'}</Dialog.Title>
		</Dialog.Header>

		<div class="space-y-4 py-2">
			<!-- Name -->
			<div class="space-y-1">
				<Label>ชื่อกลุ่มกิจกรรม <span class="text-destructive">*</span></Label>
				<Input bind:value={form.name} placeholder="เช่น ลูกเสือกลุ่ม 1, ชุมนุมคอมพิวเตอร์" />
			</div>

			<!-- Type + Registration type -->
			<div class="grid grid-cols-2 gap-3">
				<div class="space-y-1">
					<Label>ประเภท</Label>
					<Select.Root value={form.activity_type} onValueChange={(v) => { form.activity_type = v as any; }}>
						<Select.Trigger class="w-full">
							{ACTIVITY_TYPE_LABELS[form.activity_type]}
						</Select.Trigger>
						<Select.Content>
							{#each Object.entries(ACTIVITY_TYPE_LABELS) as [val, label]}
								<Select.Item value={val}>{label}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>

				<div class="space-y-1">
					<Label>การรับสมาชิก</Label>
					<Select.Root value={form.registration_type} onValueChange={(v) => { form.registration_type = v as any; }}>
						<Select.Trigger class="w-full">
							{form.registration_type === 'self' ? 'นักเรียนเลือกเอง' : 'ครู/admin จัดสรร'}
						</Select.Trigger>
						<Select.Content>
							<Select.Item value="assigned">ครู/admin จัดสรร</Select.Item>
							<Select.Item value="self">นักเรียนเลือกเอง</Select.Item>
						</Select.Content>
					</Select.Root>
				</div>
			</div>

			<!-- Semester + Instructor -->
			<div class="grid grid-cols-2 gap-3">
				<div class="space-y-1">
					<Label>ภาคเรียน <span class="text-destructive">*</span></Label>
					<Select.Root value={form.semester_id} onValueChange={(v) => { form.semester_id = v; }}>
						<Select.Trigger class="w-full">
							{structure.semesters.find((s) => s.id === form.semester_id)?.name ?? 'เลือก...'}
						</Select.Trigger>
						<Select.Content>
							{#each structure.semesters as s}
								<Select.Item value={s.id}>{s.name}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>

				<div class="space-y-1">
					<Label>ครูที่ดูแล</Label>
					<Select.Root
						value={form.instructor_id ?? ''}
						onValueChange={(v) => { form.instructor_id = v || undefined; }}
					>
						<Select.Trigger class="w-full">
							{staffList.find((s) => s.id === form.instructor_id)
								? `${staffList.find((s) => s.id === form.instructor_id)!.first_name} ${staffList.find((s) => s.id === form.instructor_id)!.last_name}`
								: 'ไม่ระบุ'}
						</Select.Trigger>
						<Select.Content class="max-h-56 overflow-y-auto">
							<Select.Item value="">ไม่ระบุ</Select.Item>
							{#each staffList as s}
								<Select.Item value={s.id}>{s.first_name} {s.last_name}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
			</div>

			<!-- Max capacity + Registration open -->
			<div class="grid grid-cols-2 gap-3">
				<div class="space-y-1">
					<Label>จำนวนรับสูงสุด</Label>
					<Input
						type="number"
						min="1"
						placeholder="ไม่จำกัด"
						value={form.max_capacity ?? ''}
						oninput={(e) => {
							const v = (e.target as HTMLInputElement).valueAsNumber;
							form.max_capacity = isNaN(v) ? undefined : v;
						}}
					/>
				</div>
				{#if form.registration_type === 'self'}
					<div class="space-y-1 flex flex-col justify-end pb-1">
						<label class="flex items-center gap-2 cursor-pointer">
							<Checkbox
								checked={form.registration_open}
								onCheckedChange={(v) => { form.registration_open = !!v; }}
							/>
							<span class="text-sm">เปิดรับสมัครแล้ว</span>
						</label>
					</div>
				{/if}
			</div>

			<!-- Grade levels -->
			<div class="space-y-1">
				<Label>ระดับชั้นที่รับ (ว่าง = รับทุกระดับ)</Label>
				<Popover.Root>
					<Popover.Trigger class="w-full">
						<Button variant="outline" class="w-full justify-between font-normal">
							{#if form.allowed_grade_level_ids && form.allowed_grade_level_ids.length > 0}
								{form.allowed_grade_level_ids
									.map((id) => {
										const l = gradeLevels.find((l) => l.id === id);
										return l?.short_name ?? l?.code ?? id;
									})
									.join(', ')}
							{:else}
								<span class="text-muted-foreground">ทุกระดับชั้น</span>
							{/if}
							<ChevronsUpDown class="ml-2 h-4 w-4 shrink-0 opacity-50" />
						</Button>
					</Popover.Trigger>
					<Popover.Content class="w-[--radix-popover-trigger-width] p-1 max-h-56 overflow-y-auto">
						{#each gradeLevels as level}
							{@const checked = form.allowed_grade_level_ids?.includes(level.id) ?? false}
							<button
								type="button"
								class="flex w-full items-center gap-2 rounded px-2 py-1.5 text-sm hover:bg-accent"
								onclick={() => toggleGradeLevel(level.id)}
							>
								<Check class="h-4 w-4 {checked ? 'opacity-100' : 'opacity-0'}" />
								{level.name}
							</button>
						{/each}
					</Popover.Content>
				</Popover.Root>
			</div>

			<!-- Description -->
			<div class="space-y-1">
				<Label>คำอธิบาย</Label>
				<Textarea bind:value={form.description} placeholder="รายละเอียดเพิ่มเติม..." rows={2} />
			</div>
		</div>

		<Dialog.Footer>
			<Button variant="outline" onclick={() => { showDialog = false; }}>ยกเลิก</Button>
			<Button onclick={handleSave} disabled={saving}>
				{saving ? 'กำลังบันทึก...' : 'บันทึก'}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<!-- Delete Confirm Dialog -->
<Dialog.Root bind:open={showDeleteDialog}>
	<Dialog.Content class="max-w-sm">
		<Dialog.Header>
			<Dialog.Title>ยืนยันการลบ</Dialog.Title>
			<Dialog.Description>
				ลบกลุ่ม "<strong>{deleteTarget?.name}</strong>" และสมาชิกทั้งหมดในกลุ่มนี้?
			</Dialog.Description>
		</Dialog.Header>
		<Dialog.Footer>
			<Button variant="outline" onclick={() => { showDeleteDialog = false; }}>ยกเลิก</Button>
			<Button variant="destructive" onclick={handleDelete}>ลบ</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
