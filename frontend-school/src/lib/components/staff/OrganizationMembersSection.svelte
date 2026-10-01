<script lang="ts">
	import {
		addOrganizationMember,
		updateOrganizationMember,
		removeOrganizationMember,
		type OrganizationMemberItem,
		type OrganizationUnit
	} from '$lib/api/staff';
	import { lookupStaff, type StaffLookupItem } from '$lib/api/lookup';
	import { onDestroy, untrack } from 'svelte';
	import { LatestRequest } from '$lib/async/latest-request';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import { Button } from '$lib/components/ui/button';
	import { Badge } from '$lib/components/ui/badge';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Label } from '$lib/components/ui/label';
	import * as Popover from '$lib/components/ui/popover';
	import * as Command from '$lib/components/ui/command';
	import * as Select from '$lib/components/ui/select';
	import {
		Users,
		Plus,
		Pencil,
		Trash2,
		Crown,
		UserCog,
		UserRound,
		Check,
		ChevronsUpDown
	} from '@lucide/svelte';

	interface Props {
		organizationUnitId: string;
		childUnits?: OrganizationUnit[];
		canAssignMembers?: boolean;
		members: OrganizationMemberItem[];
		loadingMembers: boolean;
		membersLoaded: boolean;
		membersError: string;
		onRefresh: () => void;
		onChanged?: () => Promise<void>;
	}

	type PositionCode =
		'director' | 'deputy_director' | 'head' | 'deputy_head' | 'coordinator' | 'member';

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

	const {
		organizationUnitId,
		childUnits = [],
		canAssignMembers = undefined,
		onChanged,
		members,
		loadingMembers,
		membersLoaded,
		membersError,
		onRefresh
	}: Props = $props();

	const includeChildren = $derived(childUnits.length > 0);
	const canManageMembers = $derived(canAssignMembers ?? $can.has(PERMISSIONS.ROLES_ASSIGN_ALL));

	let mutationError = $state('');
	const canReadPicker = $derived(
		canManageMembers ||
			$can.hasAny(
				PERMISSIONS.STAFF_PROFILE_READ_OWN,
				PERMISSIONS.STAFF_PROFILE_READ_ORGANIZATION_UNIT,
				PERMISSIONS.STAFF_PROFILE_READ_ORGANIZATION_TREE,
				PERMISSIONS.STAFF_PROFILE_READ_SCHOOL
			)
	);
	const pickerRequest = new LatestRequest();
	let disposed = false,
		ownerEpoch = 0,
		activeOwner = '',
		draftEpoch = 0;

	let showAddDialog = $state(false);
	let staffPickerOpen = $state(false);
	let staffSearch = $state('');
	let selectedStaffLabel = $state('');
	let staffResults: StaffLookupItem[] = $state([]);

	let searchLoading = $state(false);
	let staffSearchError = $state('');
	let staffSearchRequestId = 0;
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
	let removingMemberId = $state('');

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

	async function loadStaffOptions(query: string) {
		if (!showAddDialog || !canManageMembers || !canReadPicker) return;
		query = query.trim();
		const ticket = pickerRequest.begin();
		const requestId = ++staffSearchRequestId;
		searchLoading = true;
		staffSearchError = '';

		try {
			const res = await lookupStaff(
				{ search: query || undefined, limit: 50 },
				{ signal: ticket.signal }
			);
			if (requestId !== staffSearchRequestId || !pickerRequest.isCurrent(ticket.revision)) return;

			const activeMemberIds = new Set(members.map((member) => member.user_id));
			staffResults = res.filter(
				(staff) => !activeMemberIds.has(staff.id) || staff.id === addForm.user_id
			);
		} catch (error) {
			if (requestId !== staffSearchRequestId || !pickerRequest.isCurrent(ticket.revision)) return;
			console.error('Failed to load staff options:', error);
			staffSearchError = 'โหลดรายชื่อบุคลากรไม่สำเร็จ';
		} finally {
			if (requestId === staffSearchRequestId && pickerRequest.isCurrent(ticket.revision)) {
				searchLoading = false;
			}
		}
	}

	async function handleAdd() {
		if (
			!canManageMembers ||
			!showAddDialog ||
			!addForm.user_id ||
			addSubmitting ||
			editSubmitting ||
			removingMemberId
		)
			return;
		const epoch = ownerEpoch,
			draft = draftEpoch;
		const current = () => !disposed && epoch === ownerEpoch;
		const targetUnit =
			includeChildren && addForm.target_unit_id ? addForm.target_unit_id : organizationUnitId;
		addSubmitting = true;
		addError = '';
		try {
			const res = await addOrganizationMember(targetUnit, {
				user_id: addForm.user_id,
				position_code: addForm.position_code,
				is_primary: addForm.is_primary
			});
			if (!current()) return;
			if (!res.success) throw new Error(res.error ?? 'เพิ่มสมาชิกไม่สำเร็จ');
			if (draft === draftEpoch) showAddDialog = false;
			await onChanged?.();
		} catch (error) {
			if (current() && draft === draftEpoch)
				addError = error instanceof Error ? error.message : 'เพิ่มสมาชิกไม่สำเร็จ';
		} finally {
			if (current()) addSubmitting = false;
		}
	}

	function openEdit(member: OrganizationMemberItem) {
		if (!canManageMembers || editSubmitting || addSubmitting || removingMemberId) return;
		editingMember = member;
		editForm = {
			position_code: member.position_code as PositionCode,
			is_primary: member.is_primary,
			new_organization_unit_id: member.organization_unit_id
		};
		showEditDialog = true;
	}

	async function handleEdit() {
		if (
			!editingMember ||
			!canManageMembers ||
			!showEditDialog ||
			editSubmitting ||
			addSubmitting ||
			removingMemberId
		)
			return;
		const member = editingMember,
			epoch = ownerEpoch,
			draft = draftEpoch;
		const current = () => !disposed && epoch === ownerEpoch;
		const body = {
			position_code: editForm.position_code,
			is_primary: editForm.is_primary,
			new_organization_unit_id:
				includeChildren && editForm.new_organization_unit_id !== member.organization_unit_id
					? editForm.new_organization_unit_id
					: undefined
		};
		editSubmitting = true;
		mutationError = '';
		try {
			const res = await updateOrganizationMember(member.organization_unit_id, member.user_id, body);
			if (!current()) return;
			if (!res.success) throw new Error(res.error ?? 'แก้ไขสมาชิกไม่สำเร็จ');
			if (draft === draftEpoch) {
				showEditDialog = false;
				editingMember = null;
			}
			await onChanged?.();
		} catch (error) {
			if (current() && draft === draftEpoch)
				mutationError = error instanceof Error ? error.message : 'แก้ไขสมาชิกไม่สำเร็จ';
		} finally {
			if (current()) editSubmitting = false;
		}
	}
	async function handleRemove(member: OrganizationMemberItem) {
		if (!canManageMembers || editSubmitting || addSubmitting || removingMemberId) return;
		const epoch = ownerEpoch;
		const current = () => !disposed && epoch === ownerEpoch;
		removingMemberId = member.user_id;
		mutationError = '';
		try {
			const res = await removeOrganizationMember(member.organization_unit_id, member.user_id);
			if (!current()) return;
			if (!res.success) throw new Error(res.error ?? 'ลบสมาชิกไม่สำเร็จ');
			await onChanged?.();
		} catch (error) {
			if (current()) mutationError = error instanceof Error ? error.message : 'ลบสมาชิกไม่สำเร็จ';
		} finally {
			if (current()) removingMemberId = '';
		}
	}

	function openAddDialog() {
		if (!canManageMembers || editSubmitting || addSubmitting || removingMemberId) return;
		addForm = {
			user_id: '',
			position_code: 'member',
			is_primary: false,
			target_unit_id: organizationUnitId
		};
		staffPickerOpen = false;
		staffSearch = '';
		selectedStaffLabel = '';
		staffResults = [];
		staffSearchError = '';

		showAddDialog = true;
		void loadStaffOptions('');
	}

	function closeAddDialog() {
		showAddDialog = false;
		addError = '';
		staffPickerOpen = false;
	}

	function closeEditDialog() {
		showEditDialog = false;
		editingMember = null;
	}

	function selectStaff(staff: StaffLookupItem) {
		addForm.user_id = staff.id;
		selectedStaffLabel = staffDisplayName(staff);
		staffSearch = '';
		staffPickerOpen = false;
	}

	function staffDisplayName(staff: StaffLookupItem) {
		return staff.name;
	}

	function staffOptionValue(staff: StaffLookupItem) {
		return staffDisplayName(staff);
	}

	function unitOptionLabel(unitId: string) {
		return unitOptions.find((option) => option.id === unitId)?.name ?? 'เลือกหน่วยงาน';
	}

	function positionOptionLabel(positionCode: string) {
		return positionLabels[positionCode as PositionCode] ?? 'เลือกตำแหน่ง';
	}

	let debounceTimer: ReturnType<typeof setTimeout>;
	function onStaffSearchInput() {
		addForm.user_id = '';
		selectedStaffLabel = '';
		clearTimeout(debounceTimer);
		pickerRequest.abort();
		staffSearchRequestId++;
		staffResults = [];
		searchLoading = true;
		debounceTimer = setTimeout(() => loadStaffOptions(staffSearch), 300);
	}

	$effect.pre(() => {
		const id = organizationUnitId,
			addOpen = showAddDialog,
			editOpen = showEditDialog,
			allowed = canManageMembers,
			read = canReadPicker;
		untrack(() => {
			draftEpoch++;
			if (id !== activeOwner) {
				activeOwner = id;
				ownerEpoch++;
				showAddDialog = false;
				showEditDialog = false;
				addSubmitting = false;
				editSubmitting = false;
				removingMemberId = '';
				mutationError = '';
			}
			if (!addOpen || !allowed || !read) {
				pickerRequest.abort();
				staffSearchRequestId++;
				clearTimeout(debounceTimer);
				searchLoading = false;
				staffResults = [];
			}
			if (!allowed) {
				showAddDialog = false;
				showEditDialog = false;
			}
			if (!editOpen) editingMember = null;
		});
	});
	onDestroy(() => {
		disposed = true;
		ownerEpoch++;
		pickerRequest.abort();
		clearTimeout(debounceTimer);
	});
</script>

<section
	data-testid="organization-members"
	aria-busy={loadingMembers}
	class="rounded-lg border bg-card"
>
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
		{#if canManageMembers}
			<Button size="sm" onclick={openAddDialog}>
				<Plus class="mr-1 h-4 w-4" />
				เพิ่มสมาชิก
			</Button>
		{/if}
	</div>

	<div class="p-5">
		{#if mutationError}<PageState
				variant="error"
				title="แก้ไขสมาชิกไม่สำเร็จ"
				description={mutationError}
			/>{/if}
		{#if membersError && membersLoaded}<PageState
				variant="error"
				title="โหลดสมาชิกไม่สำเร็จ"
				description={membersError}
				actionLabel="ลองอีกครั้ง"
				onaction={onRefresh}
			/>{/if}
		{#if loadingMembers && membersLoaded}<p role="status">กำลังอัปเดตสมาชิก...</p>{/if}
		{#if loadingMembers && !membersLoaded}<div role="status" aria-label="กำลังโหลดสมาชิก">
				<PageSkeleton variant="table" rows={3} />
			</div>
		{:else if membersError && !membersLoaded}<PageState
				variant="error"
				title="โหลดสมาชิกไม่สำเร็จ"
				description={membersError}
				actionLabel="ลองอีกครั้ง"
				onaction={onRefresh}
			/>
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

									{#if canManageMembers}
										<div class="flex shrink-0 items-center gap-1">
											<Button
												variant="ghost"
												size="sm"
												onclick={() => openEdit(member)}
												aria-label={`แก้ไขสมาชิก ${member.name}`}
												disabled={editSubmitting || addSubmitting || !!removingMemberId}
											>
												<Pencil class="h-3.5 w-3.5" />
											</Button>
											<Button
												variant="ghost"
												size="sm"
												class="text-destructive hover:text-destructive"
												onclick={() => handleRemove(member)}
												aria-label={`ลบสมาชิก ${member.name}`}
												disabled={editSubmitting || addSubmitting || !!removingMemberId}
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

<Dialog.Root bind:open={showAddDialog}>
	<Dialog.Content class="max-h-[90vh] overflow-y-auto sm:max-w-[520px]">
		<Dialog.Header>
			<Dialog.Title>เพิ่มสมาชิก</Dialog.Title>
			<Dialog.Description>ค้นหาบุคลากรและกำหนดตำแหน่งในหน่วยงานนี้</Dialog.Description>
		</Dialog.Header>

		<div class="space-y-4 py-2">
			{#if !canReadPicker}<PageState
					variant="permission"
					title="ไม่มีสิทธิ์อ่านตัวเลือกบุคลากร"
				/>{/if}
			{#if addError}
				<div class="rounded-md bg-destructive/10 p-3 text-sm text-destructive">{addError}</div>
			{/if}

			<div class="space-y-2">
				<Label for="staff-search">ค้นหาบุคลากร *</Label>
				<Popover.Root bind:open={staffPickerOpen}>
					<Popover.Trigger>
						{#snippet child({ props })}
							<Button
								type="button"
								variant="outline"
								role="combobox"
								aria-expanded={staffPickerOpen}
								class="w-full justify-between font-normal"
								{...props}
							>
								<span class="truncate {selectedStaffLabel ? '' : 'text-muted-foreground'}">
									{selectedStaffLabel || 'เลือกบุคลากร'}
								</span>
								<ChevronsUpDown class="ml-2 h-4 w-4 shrink-0 opacity-50" />
							</Button>
						{/snippet}
					</Popover.Trigger>
					<Popover.Content class="w-[--bits-popover-trigger-width] p-0">
						<Command.Root shouldFilter={false}>
							<Command.Input
								id="staff-search"
								placeholder="ค้นหาด้วยชื่อหรือรหัส..."
								bind:value={staffSearch}
								oninput={onStaffSearchInput}
							/>
							<Command.List class="max-h-64">
								{#if searchLoading}
									<div role="status" aria-label="กำลังค้นหาบุคลากร">
										<PageSkeleton variant="table" rows={3} />
									</div>
								{:else if staffSearchError}
									<PageState
										variant="error"
										title="โหลดตัวเลือกบุคลากรไม่สำเร็จ"
										description={staffSearchError}
										actionLabel="ลองอีกครั้ง"
										onaction={() => loadStaffOptions(staffSearch)}
									/>
								{:else if staffResults.length === 0}
									<div class="py-6 text-center text-sm text-muted-foreground">
										ไม่พบรายชื่อบุคลากร
									</div>
								{:else}
									<Command.Group>
										{#each staffResults as staff (staff.id)}
											<Command.Item
												value={staffOptionValue(staff)}
												onSelect={() => selectStaff(staff)}
											>
												<Check
													class="mr-2 h-4 w-4 shrink-0 {addForm.user_id === staff.id
														? 'opacity-100'
														: 'opacity-0'}"
												/>
												<span class="truncate">{staffDisplayName(staff)}</span>
												<span class="ml-auto shrink-0 text-xs text-muted-foreground">
													{staff.title ?? 'บุคลากร'}
												</span>
											</Command.Item>
										{/each}
									</Command.Group>
								{/if}
							</Command.List>
						</Command.Root>
					</Popover.Content>
				</Popover.Root>
			</div>

			{#if includeChildren}
				<div class="space-y-2">
					<Label>หน่วยงานที่สังกัด</Label>
					<Select.Root type="single" bind:value={addForm.target_unit_id}>
						<Select.Trigger class="w-full">{unitOptionLabel(addForm.target_unit_id)}</Select.Trigger
						>
						<Select.Content>
							{#each unitOptions as option (option.id)}
								<Select.Item value={option.id}>{option.name}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
			{/if}

			<div class="space-y-2">
				<Label>ตำแหน่ง *</Label>
				<Select.Root type="single" bind:value={addForm.position_code}>
					<Select.Trigger class="w-full">
						{positionOptionLabel(addForm.position_code)}
					</Select.Trigger>
					<Select.Content>
						{#each positionOptions as position (position.value)}
							<Select.Item value={position.value}>{position.label}</Select.Item>
						{/each}
					</Select.Content>
				</Select.Root>
			</div>

			<div class="flex items-start gap-3 rounded-md border p-3">
				<Checkbox id="add-primary" bind:checked={addForm.is_primary} class="mt-0.5" />
				<Label for="add-primary" class="cursor-pointer leading-5">
					เป็นสังกัดหลักของบุคลากรนี้
				</Label>
			</div>
		</div>

		<Dialog.Footer>
			<Button variant="outline" onclick={closeAddDialog}>ยกเลิก</Button>
			<Button onclick={handleAdd} disabled={addSubmitting || !addForm.user_id}>
				{addSubmitting ? 'กำลังบันทึก...' : 'เพิ่ม'}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<Dialog.Root bind:open={showEditDialog}>
	<Dialog.Content class="sm:max-w-[420px]">
		<Dialog.Header>
			<Dialog.Title>แก้ไขสมาชิก</Dialog.Title>
			<Dialog.Description>{editingMember?.name ?? 'สมาชิกในหน่วยงาน'}</Dialog.Description>
		</Dialog.Header>

		<div class="space-y-4 py-2">
			{#if includeChildren}
				<div class="space-y-2">
					<Label>หน่วยงานที่สังกัด</Label>
					<Select.Root type="single" bind:value={editForm.new_organization_unit_id}>
						<Select.Trigger class="w-full">
							{unitOptionLabel(editForm.new_organization_unit_id)}
						</Select.Trigger>
						<Select.Content>
							{#each unitOptions as option (option.id)}
								<Select.Item value={option.id}>{option.name}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
			{/if}

			<div class="space-y-2">
				<Label>ตำแหน่ง</Label>
				<Select.Root type="single" bind:value={editForm.position_code}>
					<Select.Trigger class="w-full">
						{positionOptionLabel(editForm.position_code)}
					</Select.Trigger>
					<Select.Content>
						{#each positionOptions as position (position.value)}
							<Select.Item value={position.value}>{position.label}</Select.Item>
						{/each}
					</Select.Content>
				</Select.Root>
			</div>

			<div class="flex items-start gap-3 rounded-md border p-3">
				<Checkbox id="edit-primary" bind:checked={editForm.is_primary} class="mt-0.5" />
				<Label for="edit-primary" class="cursor-pointer leading-5">เป็นสังกัดหลัก</Label>
			</div>
		</div>

		<Dialog.Footer>
			<Button variant="outline" onclick={closeEditDialog}>ยกเลิก</Button>
			<Button onclick={handleEdit} disabled={editSubmitting || !editingMember}>
				{editSubmitting ? 'กำลังบันทึก...' : 'บันทึก'}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
