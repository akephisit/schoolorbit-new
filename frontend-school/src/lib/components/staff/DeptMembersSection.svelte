<script lang="ts">
	import {
		listDeptMembers,
		addDeptMember,
		updateDeptMember,
		removeDeptMember,
		listStaff,
		type DeptMemberItem,
		type StaffListItem,
		type Department
	} from '$lib/api/staff';
	import { can } from '$lib/stores/permissions';
	import { Button } from '$lib/components/ui/button';
	import { Users, Plus, Pencil, Trash2 } from 'lucide-svelte';

	interface Props {
		departmentId: string;
		subDepartments?: Department[]; // ถ้าส่งมา จะโหลด include_children และแสดง dropdown ฝ่าย
	}

	const { departmentId, subDepartments = [] }: Props = $props();

	const includeChildren = $derived(subDepartments.length > 0);

	let members: DeptMemberItem[] = $state([]);
	let loadingMembers = $state(false);

	// Add member dialog
	let showAddDialog = $state(false);
	let staffSearch = $state('');
	let staffResults: StaffListItem[] = $state([]);
	let searchLoading = $state(false);
	let addForm = $state({ user_id: '', position: 'member', is_primary: false, target_dept_id: '' });
	let addError = $state('');
	let addSubmitting = $state(false);

	// Edit dialog
	let showEditDialog = $state(false);
	let editingMember: DeptMemberItem | null = $state(null);
	let editForm = $state({ position: 'member', is_primary: false, new_department_id: '' });
	let editSubmitting = $state(false);

	const positionLabels: Record<string, string> = {
		head: 'หัวหน้ากลุ่ม',
		member: 'สมาชิก'
	};

	async function loadMembers() {
		if (!departmentId) return;
		loadingMembers = true;
		const res = await listDeptMembers(departmentId, { include_children: includeChildren });
		if (res.success && res.data) members = res.data;
		loadingMembers = false;
	}

	async function searchStaff() {
		if (staffSearch.length < 2) { staffResults = []; return; }
		searchLoading = true;
		const res = await listStaff({ search: staffSearch, page_size: 20 });
		staffResults = res.data ?? [];
		searchLoading = false;
	}

	async function handleAdd() {
		if (!addForm.user_id || !addForm.position) return;
		addSubmitting = true;
		addError = '';
		// ถ้าเลือกฝ่ายย่อย ให้ add เข้าฝ่ายนั้นแทน
		const targetDept = (includeChildren && addForm.target_dept_id) ? addForm.target_dept_id : departmentId;
		const res = await addDeptMember(targetDept, {
			user_id: addForm.user_id,
			position: addForm.position,
			is_primary: addForm.is_primary
		});
		if (res.success) {
			showAddDialog = false;
			addForm = { user_id: '', position: 'member', is_primary: false, target_dept_id: '' };
			staffSearch = '';
			staffResults = [];
			await loadMembers();
		} else {
			addError = res.error ?? 'เกิดข้อผิดพลาด';
		}
		addSubmitting = false;
	}

	function openEdit(member: DeptMemberItem) {
		editingMember = member;
		editForm = {
			position: member.position,
			is_primary: member.is_primary,
			new_department_id: member.department_id
		};
		showEditDialog = true;
	}

	async function handleEdit() {
		if (!editingMember) return;
		editSubmitting = true;
		const body: { position: string; is_primary: boolean; new_department_id?: string } = {
			position: editForm.position,
			is_primary: editForm.is_primary
		};
		// ส่ง new_department_id เฉพาะเมื่อเปลี่ยนฝ่าย
		if (includeChildren && editForm.new_department_id !== editingMember.department_id) {
			body.new_department_id = editForm.new_department_id;
		}
		const res = await updateDeptMember(editingMember.department_id, editingMember.user_id, body);
		if (res.success) {
			showEditDialog = false;
			editingMember = null;
			await loadMembers();
		}
		editSubmitting = false;
	}

	async function handleRemove(member: DeptMemberItem) {
		const res = await removeDeptMember(member.department_id, member.user_id);
		if (res.success) await loadMembers();
	}

	$effect(() => {
		if (departmentId) loadMembers();
	});

	let debounceTimer: ReturnType<typeof setTimeout>;
	function onSearchInput() {
		clearTimeout(debounceTimer);
		debounceTimer = setTimeout(searchStaff, 300);
	}

	// allDepts สำหรับ dropdown: กลุ่มหลัก + ฝ่ายย่อย
	const deptOptions = $derived([
		{ id: departmentId, name: 'กลุ่มหลัก (ไม่สังกัดฝ่าย)' },
		...subDepartments.map(d => ({ id: d.id, name: d.name }))
	]);
</script>

<div class="bg-card border border-border rounded-lg p-6 space-y-4">
	<div class="flex items-center justify-between">
		<h2 class="text-lg font-semibold flex items-center gap-2">
			<Users class="w-5 h-5" />
			สมาชิก ({members.length} คน)
		</h2>
		{#if $can.hasAny('roles.assign.all', '*')}
			<Button size="sm" onclick={() => (showAddDialog = true)}>
				<Plus class="w-4 h-4 mr-1" />
				เพิ่มสมาชิก
			</Button>
		{/if}
	</div>

	{#if loadingMembers}
		<p class="text-muted-foreground text-sm text-center py-4">กำลังโหลด...</p>
	{:else if members.length === 0}
		<p class="text-muted-foreground text-sm text-center py-4">ยังไม่มีสมาชิก</p>
	{:else}
		<div class="divide-y divide-border">
			{#each members as member}
				<div class="py-3 flex items-center justify-between gap-3">
					<div class="flex items-center gap-3 min-w-0">
						<div class="w-9 h-9 rounded-full bg-muted flex items-center justify-center shrink-0 text-sm font-medium">
							{member.name.charAt(0)}
						</div>
						<div class="min-w-0">
							<p class="font-medium text-sm truncate">{member.name}</p>
							<p class="text-xs text-muted-foreground">
								{positionLabels[member.position] ?? member.position}
								{#if includeChildren}
									· <span class="text-primary/70">{member.department_name}</span>
								{/if}
							</p>
						</div>
					</div>
					{#if $can.hasAny('roles.assign.all', '*')}
						<div class="flex items-center gap-1 shrink-0">
							<Button variant="ghost" size="sm" onclick={() => openEdit(member)}>
								<Pencil class="w-3.5 h-3.5" />
							</Button>
							<Button
								variant="ghost"
								size="sm"
								class="text-destructive hover:text-destructive"
								onclick={() => handleRemove(member)}
							>
								<Trash2 class="w-3.5 h-3.5" />
							</Button>
						</div>
					{/if}
				</div>
			{/each}
		</div>
	{/if}
</div>

<!-- Add Member Dialog -->
{#if showAddDialog}
	<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
		<div class="bg-background border border-border rounded-xl shadow-lg w-full max-w-md p-6 space-y-4">
			<h3 class="text-lg font-semibold">เพิ่มสมาชิก</h3>

			{#if addError}
				<div class="text-sm text-destructive bg-destructive/10 rounded p-3">{addError}</div>
			{/if}

			<div class="space-y-3">
				<div class="space-y-1">
					<label for="staff-search" class="text-sm font-medium">ค้นหาบุคลากร *</label>
					<input
						id="staff-search"
						type="text"
						bind:value={staffSearch}
						oninput={onSearchInput}
						placeholder="พิมพ์ชื่อหรือรหัส..."
						class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
					/>
					{#if searchLoading}
						<p class="text-xs text-muted-foreground">กำลังค้นหา...</p>
					{:else if staffResults.length > 0}
						<div class="border border-border rounded-md overflow-hidden max-h-48 overflow-y-auto">
							{#each staffResults as staff}
								<button
									type="button"
									class="w-full text-left px-3 py-2 text-sm hover:bg-muted transition-colors {addForm.user_id === staff.id ? 'bg-primary/10 font-medium' : ''}"
									onclick={() => { addForm.user_id = staff.id; staffSearch = `${staff.title}${staff.first_name} ${staff.last_name}`; staffResults = []; }}
								>
									{staff.title}{staff.first_name} {staff.last_name}
								</button>
							{/each}
						</div>
					{/if}
				</div>

				{#if includeChildren}
					<div class="space-y-1">
						<label for="add-target-dept" class="text-sm font-medium">ฝ่ายที่สังกัด</label>
						<select
							id="add-target-dept"
							bind:value={addForm.target_dept_id}
							class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
						>
							{#each deptOptions as opt}
								<option value={opt.id}>{opt.name}</option>
							{/each}
						</select>
					</div>
				{/if}

				<div class="space-y-1">
					<label for="add-position" class="text-sm font-medium">ตำแหน่งในกลุ่ม *</label>
					<select
						id="add-position"
						bind:value={addForm.position}
						class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
					>
						<option value="head">หัวหน้ากลุ่ม</option>
						<option value="member">สมาชิก</option>
					</select>
				</div>

				<label class="flex items-center gap-2 text-sm cursor-pointer">
					<input type="checkbox" bind:checked={addForm.is_primary} class="rounded" />
					เป็นฝ่าย/กลุ่มหลักของบุคลากรนี้
				</label>
			</div>

			<div class="flex justify-end gap-2 pt-2">
				<Button variant="outline" onclick={() => { showAddDialog = false; addError = ''; }}>ยกเลิก</Button>
				<Button onclick={handleAdd} disabled={addSubmitting || !addForm.user_id}>
					{addSubmitting ? 'กำลังบันทึก...' : 'เพิ่ม'}
				</Button>
			</div>
		</div>
	</div>
{/if}

<!-- Edit Dialog -->
{#if showEditDialog && editingMember}
	<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
		<div class="bg-background border border-border rounded-xl shadow-lg w-full max-w-sm p-6 space-y-4">
			<h3 class="text-lg font-semibold">แก้ไขสมาชิก — {editingMember.name}</h3>

			<div class="space-y-3">
				{#if includeChildren}
					<div class="space-y-1">
						<label for="edit-dept" class="text-sm font-medium">ฝ่ายที่สังกัด</label>
						<select
							id="edit-dept"
							bind:value={editForm.new_department_id}
							class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
						>
							{#each deptOptions as opt}
								<option value={opt.id}>{opt.name}</option>
							{/each}
						</select>
					</div>
				{/if}

				<div class="space-y-1">
					<label for="edit-position" class="text-sm font-medium">ตำแหน่งในกลุ่ม</label>
					<select
						id="edit-position"
						bind:value={editForm.position}
						class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
					>
						<option value="head">หัวหน้ากลุ่ม</option>
						<option value="member">สมาชิก</option>
					</select>
				</div>

				<label class="flex items-center gap-2 text-sm cursor-pointer">
					<input type="checkbox" bind:checked={editForm.is_primary} class="rounded" />
					เป็นฝ่าย/กลุ่มหลัก
				</label>
			</div>

			<div class="flex justify-end gap-2 pt-2">
				<Button variant="outline" onclick={() => { showEditDialog = false; editingMember = null; }}>ยกเลิก</Button>
				<Button onclick={handleEdit} disabled={editSubmitting}>
					{editSubmitting ? 'กำลังบันทึก...' : 'บันทึก'}
				</Button>
			</div>
		</div>
	</div>
{/if}
