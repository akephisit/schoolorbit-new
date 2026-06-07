<script lang="ts">
	import {
		listOrganizationMembers,
		addOrganizationMember,
		updateOrganizationMember,
		removeOrganizationMember,
		listStaff,
		type OrganizationMemberItem,
		type StaffListItem,
		type OrganizationUnit
	} from '$lib/api/staff';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import { Button } from '$lib/components/ui/button';
	import { Badge } from '$lib/components/ui/badge';
	import { Users, Plus, Pencil, Trash2, Crown, UserCog, UserRound } from 'lucide-svelte';

	interface Props {
		organizationUnitId: string;
		childUnits?: OrganizationUnit[];
		onChanged?: () => void;
	}

	type PositionCode =
		| 'director'
		| 'deputy_director'
		| 'head'
		| 'deputy_head'
		| 'coordinator'
		| 'member';

	type PositionOption = {
		value: PositionCode;
		label: string;
		group: 'leadership' | 'coordination' | 'member';
	};

	type MemberGroup = {
		key: string;
		title: string;
		members: OrganizationMemberItem[];
	};

	const { organizationUnitId, childUnits = [], onChanged }: Props = $props();

	const includeChildren = $derived(childUnits.length > 0);

	let members: OrganizationMemberItem[] = $state([]);
	let loadingMembers = $state(false);

	let showAddDialog = $state(false);
	let staffSearch = $state('');
	let staffResults: StaffListItem[] = $state([]);
	let searchLoading = $state(false);
	let addForm = $state({
		user_id: '',
		position_code: 'member' as PositionCode,
		is_primary: false,
		target_unit_id: ''
	});
	let addError = $state('');
	let addSubmitting = $state(false);

	let showEditDialog = $state(false);
	let editingMember: OrganizationMemberItem | null = $state(null);
	let editForm = $state({
		position_code: 'member' as PositionCode,
		is_primary: false,
		new_organization_unit_id: ''
	});
	let editSubmitting = $state(false);

	const positionOptions: PositionOption[] = [
		{ value: 'director', label: 'ผู้อำนวยการ', group: 'leadership' },
		{ value: 'deputy_director', label: 'รองผู้อำนวยการ', group: 'leadership' },
		{ value: 'head', label: 'หัวหน้า', group: 'leadership' },
		{ value: 'deputy_head', label: 'รองหัวหน้า', group: 'coordination' },
		{ value: 'coordinator', label: 'ผู้ประสานงาน', group: 'coordination' },
		{ value: 'member', label: 'สมาชิก', group: 'member' }
	];

	const positionLabels = Object.fromEntries(
		positionOptions.map((position) => [position.value, position.label])
	) as Record<PositionCode, string>;

	const positionRank = Object.fromEntries(
		positionOptions.map((position, index) => [position.value, index])
	) as Record<PositionCode, number>;

	const activeMemberCount = $derived(members.length);
	const leaderCount = $derived(
		members.filter((member) =>
			['director', 'deputy_director', 'head'].includes(member.position_code)
		).length
	);

	const groupedMembers = $derived.by(() => {
		const sortedMembers = [...members].sort((a, b) => {
			const rankA = positionRank[a.position_code as PositionCode] ?? 99;
			const rankB = positionRank[b.position_code as PositionCode] ?? 99;
			return rankA - rankB || a.name.localeCompare(b.name, 'th');
		});

		const groups: MemberGroup[] = [
			{
				key: 'leadership',
				title: 'ผู้บริหารและหัวหน้า',
				members: sortedMembers.filter((member) =>
					['director', 'deputy_director', 'head'].includes(member.position_code)
				)
			},
			{
				key: 'coordination',
				title: 'รองหัวหน้าและผู้ประสานงาน',
				members: sortedMembers.filter((member) =>
					['deputy_head', 'coordinator'].includes(member.position_code)
				)
			},
			{
				key: 'member',
				title: 'สมาชิก',
				members: sortedMembers.filter((member) => member.position_code === 'member')
			}
		];

		return groups.filter((group) => group.members.length > 0);
	});

	const unitOptions = $derived([
		{ id: organizationUnitId, name: 'หน่วยงานหลัก' },
		...childUnits.map((unit) => ({ id: unit.id, name: unit.name }))
	]);

	async function loadMembers() {
		if (!organizationUnitId) return;
		loadingMembers = true;
		const res = await listOrganizationMembers(organizationUnitId, {
			include_children: includeChildren
		});
		if (res.success && res.data) members = res.data;
		loadingMembers = false;
	}

	async function searchStaff() {
		if (staffSearch.length < 2) {
			staffResults = [];
			return;
		}
		searchLoading = true;
		const res = await listStaff({ search: staffSearch, page_size: 20 });
		staffResults = res.data ?? [];
		searchLoading = false;
	}

	async function handleAdd() {
		if (!addForm.user_id || !addForm.position_code) return;
		addSubmitting = true;
		addError = '';
		const targetUnit =
			includeChildren && addForm.target_unit_id ? addForm.target_unit_id : organizationUnitId;
		const res = await addOrganizationMember(targetUnit, {
			user_id: addForm.user_id,
			position_code: addForm.position_code,
			is_primary: addForm.is_primary
		});
		if (res.success) {
			showAddDialog = false;
			addForm = { user_id: '', position_code: 'member', is_primary: false, target_unit_id: '' };
			staffSearch = '';
			staffResults = [];
			await loadMembers();
			onChanged?.();
		} else {
			addError = res.error ?? 'เกิดข้อผิดพลาด';
		}
		addSubmitting = false;
	}

	function openEdit(member: OrganizationMemberItem) {
		editingMember = member;
		editForm = {
			position_code: member.position_code as PositionCode,
			is_primary: member.is_primary,
			new_organization_unit_id: member.organization_unit_id
		};
		showEditDialog = true;
	}

	async function handleEdit() {
		if (!editingMember) return;
		editSubmitting = true;
		const body: {
			position_code: string;
			is_primary: boolean;
			new_organization_unit_id?: string;
		} = {
			position_code: editForm.position_code,
			is_primary: editForm.is_primary
		};
		if (
			includeChildren &&
			editForm.new_organization_unit_id !== editingMember.organization_unit_id
		) {
			body.new_organization_unit_id = editForm.new_organization_unit_id;
		}
		const res = await updateOrganizationMember(
			editingMember.organization_unit_id,
			editingMember.user_id,
			body
		);
		if (res.success) {
			showEditDialog = false;
			editingMember = null;
			await loadMembers();
			onChanged?.();
		}
		editSubmitting = false;
	}

	async function handleRemove(member: OrganizationMemberItem) {
		const res = await removeOrganizationMember(member.organization_unit_id, member.user_id);
		if (res.success) {
			await loadMembers();
			onChanged?.();
		}
	}

	function openAddDialog() {
		addForm.target_unit_id = organizationUnitId;
		showAddDialog = true;
	}

	let debounceTimer: ReturnType<typeof setTimeout>;
	function onSearchInput() {
		clearTimeout(debounceTimer);
		debounceTimer = setTimeout(searchStaff, 300);
	}

	$effect(() => {
		if (organizationUnitId) loadMembers();
	});
</script>

<section class="rounded-lg border bg-card">
	<div class="flex flex-col gap-4 border-b p-5 sm:flex-row sm:items-center sm:justify-between">
		<div class="space-y-1">
			<h2 class="flex items-center gap-2 text-lg font-semibold">
				<Users class="h-5 w-5" />
				สมาชิก
			</h2>
			<div class="flex flex-wrap gap-2">
				<Badge variant="outline">{activeMemberCount} คน</Badge>
				<Badge variant="secondary">{leaderCount} ผู้บริหาร/หัวหน้า</Badge>
				{#if includeChildren}
					<Badge variant="outline">{childUnits.length} หน่วยงานย่อย</Badge>
				{/if}
			</div>
		</div>
		{#if $can.has(PERMISSIONS.ROLES_ASSIGN_ALL)}
			<Button size="sm" onclick={openAddDialog}>
				<Plus class="mr-1 h-4 w-4" />
				เพิ่มสมาชิก
			</Button>
		{/if}
	</div>

	<div class="p-5">
		{#if loadingMembers}
			<p class="py-6 text-center text-sm text-muted-foreground">กำลังโหลด...</p>
		{:else if members.length === 0}
			<div class="rounded-lg border border-dashed py-10 text-center text-sm text-muted-foreground">
				ยังไม่มีสมาชิก
			</div>
		{:else}
			<div class="space-y-5">
				{#each groupedMembers as group (group.key)}
					<div class="space-y-2">
						<div class="flex items-center gap-2 text-sm font-medium text-muted-foreground">
							{#if group.key === 'leadership'}
								<Crown class="h-4 w-4" />
							{:else if group.key === 'coordination'}
								<UserCog class="h-4 w-4" />
							{:else}
								<UserRound class="h-4 w-4" />
							{/if}
							<span>{group.title}</span>
							<span class="text-xs">({group.members.length})</span>
						</div>

						<div class="grid gap-2">
							{#each group.members as member (member.user_id + '-' + member.organization_unit_id)}
								<div
									class="flex items-center justify-between gap-3 rounded-md border bg-background px-3 py-3"
								>
									<div class="flex min-w-0 items-center gap-3">
										<div
											class="flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-primary/10 text-sm font-semibold text-primary"
										>
											{member.name.charAt(0)}
										</div>
										<div class="min-w-0">
											<p class="truncate text-sm font-medium">{member.name}</p>
											<div class="flex flex-wrap items-center gap-1 text-xs text-muted-foreground">
												<span
													>{positionLabels[member.position_code as PositionCode] ??
														member.position_code}</span
												>
												{#if includeChildren}
													<span>·</span>
													<span class="text-primary/80">{member.organization_unit_name}</span>
												{/if}
												{#if member.is_primary}
													<span>·</span>
													<span>สังกัดหลัก</span>
												{/if}
											</div>
										</div>
									</div>

									{#if $can.has(PERMISSIONS.ROLES_ASSIGN_ALL)}
										<div class="flex shrink-0 items-center gap-1">
											<Button variant="ghost" size="sm" onclick={() => openEdit(member)}>
												<Pencil class="h-3.5 w-3.5" />
											</Button>
											<Button
												variant="ghost"
												size="sm"
												class="text-destructive hover:text-destructive"
												onclick={() => handleRemove(member)}
											>
												<Trash2 class="h-3.5 w-3.5" />
											</Button>
										</div>
									{/if}
								</div>
							{/each}
						</div>
					</div>
				{/each}
			</div>
		{/if}
	</div>
</section>

{#if showAddDialog}
	<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4">
		<div class="w-full max-w-md space-y-4 rounded-lg border bg-background p-6 shadow-lg">
			<h3 class="text-lg font-semibold">เพิ่มสมาชิก</h3>

			{#if addError}
				<div class="rounded-md bg-destructive/10 p-3 text-sm text-destructive">{addError}</div>
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
						<div class="max-h-48 overflow-y-auto rounded-md border">
							{#each staffResults as staff (staff.id)}
								<button
									type="button"
									class="w-full px-3 py-2 text-left text-sm transition-colors hover:bg-muted {addForm.user_id ===
									staff.id
										? 'bg-primary/10 font-medium'
										: ''}"
									onclick={() => {
										addForm.user_id = staff.id;
										staffSearch = `${staff.title}${staff.first_name} ${staff.last_name}`;
										staffResults = [];
									}}
								>
									{staff.title}{staff.first_name}
									{staff.last_name}
								</button>
							{/each}
						</div>
					{/if}
				</div>

				{#if includeChildren}
					<div class="space-y-1">
						<label for="add-target-unit" class="text-sm font-medium">หน่วยงานที่สังกัด</label>
						<select
							id="add-target-unit"
							bind:value={addForm.target_unit_id}
							class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
						>
							{#each unitOptions as option (option.id)}
								<option value={option.id}>{option.name}</option>
							{/each}
						</select>
					</div>
				{/if}

				<div class="space-y-1">
					<label for="add-position" class="text-sm font-medium">ตำแหน่ง *</label>
					<select
						id="add-position"
						bind:value={addForm.position_code}
						class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
					>
						{#each positionOptions as position (position.value)}
							<option value={position.value}>{position.label}</option>
						{/each}
					</select>
				</div>

				<label class="flex cursor-pointer items-center gap-2 text-sm">
					<input type="checkbox" bind:checked={addForm.is_primary} class="rounded" />
					เป็นสังกัดหลักของบุคลากรนี้
				</label>
			</div>

			<div class="flex justify-end gap-2 pt-2">
				<Button
					variant="outline"
					onclick={() => {
						showAddDialog = false;
						addError = '';
					}}>ยกเลิก</Button
				>
				<Button onclick={handleAdd} disabled={addSubmitting || !addForm.user_id}>
					{addSubmitting ? 'กำลังบันทึก...' : 'เพิ่ม'}
				</Button>
			</div>
		</div>
	</div>
{/if}

{#if showEditDialog && editingMember}
	<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4">
		<div class="w-full max-w-sm space-y-4 rounded-lg border bg-background p-6 shadow-lg">
			<h3 class="text-lg font-semibold">แก้ไขสมาชิก</h3>
			<p class="text-sm text-muted-foreground">{editingMember.name}</p>

			<div class="space-y-3">
				{#if includeChildren}
					<div class="space-y-1">
						<label for="edit-unit" class="text-sm font-medium">หน่วยงานที่สังกัด</label>
						<select
							id="edit-unit"
							bind:value={editForm.new_organization_unit_id}
							class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
						>
							{#each unitOptions as option (option.id)}
								<option value={option.id}>{option.name}</option>
							{/each}
						</select>
					</div>
				{/if}

				<div class="space-y-1">
					<label for="edit-position" class="text-sm font-medium">ตำแหน่ง</label>
					<select
						id="edit-position"
						bind:value={editForm.position_code}
						class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
					>
						{#each positionOptions as position (position.value)}
							<option value={position.value}>{position.label}</option>
						{/each}
					</select>
				</div>

				<label class="flex cursor-pointer items-center gap-2 text-sm">
					<input type="checkbox" bind:checked={editForm.is_primary} class="rounded" />
					เป็นสังกัดหลัก
				</label>
			</div>

			<div class="flex justify-end gap-2 pt-2">
				<Button
					variant="outline"
					onclick={() => {
						showEditDialog = false;
						editingMember = null;
					}}>ยกเลิก</Button
				>
				<Button onclick={handleEdit} disabled={editSubmitting}>
					{editSubmitting ? 'กำลังบันทึก...' : 'บันทึก'}
				</Button>
			</div>
		</div>
	</div>
{/if}
