<script lang="ts">
	import { onMount } from 'svelte';
	import {
		deleteMenuItem,
		listMenuGroups,
		listMenuItems,
		listMenuWorkspaces,
		reorderMenuGroups,
		reorderMenuItems,
		reorderMenuWorkspaces,
		type MenuGroup,
		type MenuItem,
		type MenuWorkspace
	} from '$lib/api/menu-admin';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import GroupManagementDialog from '$lib/components/menu/GroupManagementDialog.svelte';
	import AcademicMenuTemplateDialog from '$lib/components/menu/AcademicMenuTemplateDialog.svelte';
	import MenuGroupContainer from '$lib/components/menu/MenuGroupContainer.svelte';
	import MenuItemManagementDialog from '$lib/components/menu/MenuItemManagementDialog.svelte';
	import SortableItem from '$lib/components/menu/SortableItem.svelte';
	import WorkspaceManagementDialog from '$lib/components/menu/WorkspaceManagementDialog.svelte';
	import MobileDragDropPolyfill from '$lib/components/MobileDragDropPolyfill.svelte';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Card } from '$lib/components/ui/card';
	import * as Select from '$lib/components/ui/select';
	import * as Tabs from '$lib/components/ui/tabs';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import { getIconComponent } from '$lib/utils/icon-mapper';
	import { GripVertical, Pencil } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';

	type GroupContainer = {
		data: MenuGroup;
		nesteds: MenuItem[];
	};

	type ActiveTab = 'items' | 'groups' | 'workspaces';
	type DragType = 'item' | 'group' | 'workspace' | null;

	let workspaces = $state<MenuWorkspace[]>([]);
	let groups = $state<MenuGroup[]>([]);
	let items = $state<MenuItem[]>([]);
	let containers = $state<GroupContainer[]>([]);
	let loading = $state(true);
	let activeTab = $state<ActiveTab>('items');
	let userTypeFilter = $state<'all' | 'staff' | 'student' | 'parent'>('all');

	let groupDialogOpen = $state(false);
	let editingGroup = $state<MenuGroup | null>(null);
	let workspaceDialogOpen = $state(false);
	let editingWorkspace = $state<MenuWorkspace | null>(null);
	let itemDialogOpen = $state(false);
	let editingItem = $state<MenuItem | null>(null);
	let academicTemplateDialogOpen = $state(false);

	let draggedItem = $state<MenuItem | null>(null);
	let draggedGroup = $state<MenuGroup | null>(null);
	let draggedWorkspace = $state<MenuWorkspace | null>(null);
	let dragType = $state<DragType>(null);

	const canReadMenu = $derived($can.has(PERMISSIONS.MENU_READ_ALL));
	const canCreateMenu = $derived($can.has(PERMISSIONS.MENU_CREATE_ALL));
	const canUpdateMenu = $derived($can.has(PERMISSIONS.MENU_UPDATE_ALL));
	const canDeleteMenu = $derived($can.has(PERMISSIONS.MENU_DELETE_ALL));

	const workspaceNameByCode = $derived(
		new Map(workspaces.map((workspace) => [workspace.code, workspace.name]))
	);

	const groupedWorkspaces = $derived(
		workspaces.map((workspace) => ({
			workspace,
			groups: groups.filter((group) => group.workspace_code === workspace.code)
		}))
	);

	const displayContainers = $derived(
		userTypeFilter === 'all'
			? containers
			: containers
					.map((container) => ({
						...container,
						nesteds: container.nesteds.filter((item) => item.user_type === userTypeFilter)
					}))
					.filter((container) => container.nesteds.length > 0)
	);

	onMount(() => {
		void loadData();
	});

	async function loadData() {
		if (!canReadMenu) {
			workspaces = [];
			groups = [];
			items = [];
			containers = [];
			loading = false;
			return;
		}

		loading = true;
		try {
			[workspaces, groups, items] = await Promise.all([
				listMenuWorkspaces(),
				listMenuGroups(),
				listMenuItems()
			]);
			sortAdministrationData();
			rebuildContainers();
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ไม่สามารถโหลดโครงสร้างเมนูได้');
		} finally {
			loading = false;
		}
	}

	function sortAdministrationData() {
		workspaces = [...workspaces].sort(
			(a, b) => a.display_order - b.display_order || a.name.localeCompare(b.name)
		);

		const workspaceOrder = new Map(workspaces.map((workspace, index) => [workspace.code, index]));
		groups = [...groups].sort(
			(a, b) =>
				(workspaceOrder.get(a.workspace_code) ?? 999) -
					(workspaceOrder.get(b.workspace_code) ?? 999) ||
				a.display_order - b.display_order ||
				a.name.localeCompare(b.name)
		);
	}

	function rebuildContainers() {
		const sortedItems = [...items].sort(
			(a, b) => a.display_order - b.display_order || a.name.localeCompare(b.name)
		);
		containers = groups.map((group) => ({
			data: group,
			nesteds: sortedItems.filter((item) => item.group_id === group.id)
		}));
	}

	function resetDragState() {
		draggedItem = null;
		draggedGroup = null;
		draggedWorkspace = null;
		dragType = null;
	}

	function replaceMenuGroup(group: MenuGroup) {
		groups = groups.some((current) => current.id === group.id)
			? groups.map((current) => (current.id === group.id ? group : current))
			: [...groups, group];
		sortAdministrationData();
		rebuildContainers();
	}

	function replaceMenuItem(item: MenuItem) {
		items = items.map((current) => (current.id === item.id ? item : current));
		rebuildContainers();
	}

	function handleGroupMutation(
		result: { type: 'upsert'; group: MenuGroup } | { type: 'delete'; groupId: string }
	) {
		if (result.type === 'upsert') {
			replaceMenuGroup(result.group);
			return;
		}
		void loadData();
	}

	function handleWorkspaceMutation(
		result: { type: 'upsert'; workspace: MenuWorkspace } | { type: 'delete'; workspaceId: string }
	) {
		if (result.type === 'upsert') {
			workspaces = workspaces.some((current) => current.id === result.workspace.id)
				? workspaces.map((current) =>
						current.id === result.workspace.id ? result.workspace : current
					)
				: [...workspaces, result.workspace];
			sortAdministrationData();
			rebuildContainers();
			return;
		}
		void loadData();
	}

	function handleItemDragStart(event: DragEvent, item: MenuItem) {
		if (!canUpdateMenu || activeTab !== 'items') return;
		event.dataTransfer?.setData('text/plain', item.id);
		if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move';
		draggedItem = item;
		dragType = 'item';
	}

	function handleItemDragEnter(_event: DragEvent, targetItem: MenuItem) {
		if (!canUpdateMenu || dragType !== 'item' || !draggedItem) return;
		if (draggedItem.id === targetItem.id) return;

		const sourceGroupIndex = containers.findIndex((container) =>
			container.nesteds.some((item) => item.id === draggedItem?.id)
		);
		const targetGroupIndex = containers.findIndex((container) =>
			container.nesteds.some((item) => item.id === targetItem.id)
		);
		if (sourceGroupIndex === -1 || targetGroupIndex === -1) return;

		const sourceList = [...containers[sourceGroupIndex].nesteds];
		const targetList =
			sourceGroupIndex === targetGroupIndex
				? sourceList
				: [...containers[targetGroupIndex].nesteds];
		const oldIndex = sourceList.findIndex((item) => item.id === draggedItem?.id);
		const newIndex = targetList.findIndex((item) => item.id === targetItem.id);
		const [removed] = sourceList.splice(oldIndex, 1);
		const movedItem = {
			...removed,
			group_id: containers[targetGroupIndex].data.id
		};

		if (sourceGroupIndex === targetGroupIndex) {
			sourceList.splice(newIndex, 0, movedItem);
			containers[sourceGroupIndex].nesteds = sourceList;
		} else {
			targetList.splice(newIndex, 0, movedItem);
			containers[sourceGroupIndex].nesteds = sourceList;
			containers[targetGroupIndex].nesteds = targetList;
		}
	}

	function handleGroupDragOver(event: DragEvent) {
		if (canUpdateMenu && dragType === 'item') {
			event.preventDefault();
			if (event.dataTransfer) event.dataTransfer.dropEffect = 'move';
		}
	}

	function handleGroupDrop(event: DragEvent, targetGroup: MenuGroup) {
		if (!canUpdateMenu || dragType !== 'item' || !draggedItem) return;
		event.preventDefault();

		const sourceIndex = containers.findIndex((container) =>
			container.nesteds.some((item) => item.id === draggedItem?.id)
		);
		const targetIndex = containers.findIndex((container) => container.data.id === targetGroup.id);
		if (sourceIndex === -1 || targetIndex === -1 || sourceIndex === targetIndex) return;

		const sourceList = [...containers[sourceIndex].nesteds];
		const targetList = [...containers[targetIndex].nesteds];
		const itemIndex = sourceList.findIndex((item) => item.id === draggedItem?.id);
		const [removed] = sourceList.splice(itemIndex, 1);
		targetList.push({ ...removed, group_id: targetGroup.id });
		containers[sourceIndex].nesteds = sourceList;
		containers[targetIndex].nesteds = targetList;
	}

	async function commitItemReorder() {
		const payload = containers.flatMap((container) =>
			container.nesteds.map((item, index) => ({
				id: item.id,
				display_order: index + 1,
				group_id: container.data.id
			}))
		);
		if (payload.length === 0) return;

		await reorderMenuItems(payload);
		items = containers.flatMap((container) =>
			container.nesteds.map((item, index) => ({
				...item,
				display_order: index + 1,
				group_id: container.data.id
			}))
		);
	}

	function handleGroupDragStart(event: DragEvent, group: MenuGroup) {
		if (!canUpdateMenu || activeTab !== 'groups') return;
		event.dataTransfer?.setData('text/plain', group.id);
		if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move';
		draggedGroup = group;
		dragType = 'group';
	}

	function handleGroupDragEnter(_event: DragEvent, targetGroup: MenuGroup) {
		if (!canUpdateMenu || dragType !== 'group' || !draggedGroup) return;
		if (
			draggedGroup.id === targetGroup.id ||
			draggedGroup.workspace_code !== targetGroup.workspace_code
		) {
			return;
		}

		const oldIndex = groups.findIndex((group) => group.id === draggedGroup?.id);
		const newIndex = groups.findIndex((group) => group.id === targetGroup.id);
		if (oldIndex === -1 || newIndex === -1) return;

		const next = [...groups];
		const [removed] = next.splice(oldIndex, 1);
		next.splice(newIndex, 0, removed);
		groups = next;
		rebuildContainers();
	}

	async function commitGroupReorder() {
		const payload = workspaces.flatMap((workspace) =>
			groups
				.filter((group) => group.workspace_code === workspace.code)
				.map((group, index) => ({ id: group.id, display_order: index + 1 }))
		);
		await reorderMenuGroups(payload);
		const orderById = new Map(payload.map((entry) => [entry.id, entry.display_order]));
		groups = groups.map((group) => ({
			...group,
			display_order: orderById.get(group.id) ?? group.display_order
		}));
		rebuildContainers();
	}

	function handleWorkspaceDragStart(event: DragEvent, workspace: MenuWorkspace) {
		if (!canUpdateMenu || activeTab !== 'workspaces') return;
		event.dataTransfer?.setData('text/plain', workspace.id);
		if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move';
		draggedWorkspace = workspace;
		dragType = 'workspace';
	}

	function handleWorkspaceDragEnter(_event: DragEvent, targetWorkspace: MenuWorkspace) {
		if (!canUpdateMenu || dragType !== 'workspace' || !draggedWorkspace) return;
		if (draggedWorkspace.id === targetWorkspace.id) return;

		const oldIndex = workspaces.findIndex((workspace) => workspace.id === draggedWorkspace?.id);
		const newIndex = workspaces.findIndex((workspace) => workspace.id === targetWorkspace.id);
		if (oldIndex === -1 || newIndex === -1) return;

		const next = [...workspaces];
		const [removed] = next.splice(oldIndex, 1);
		next.splice(newIndex, 0, removed);
		workspaces = next;
	}

	async function commitWorkspaceReorder() {
		const payload = workspaces.map((workspace, index) => ({
			id: workspace.id,
			display_order: index + 1
		}));
		await reorderMenuWorkspaces(payload);
		workspaces = workspaces.map((workspace, index) => ({
			...workspace,
			display_order: index + 1
		}));
		rebuildContainers();
	}

	async function handleDragEnd(event: DragEvent) {
		event.preventDefault();
		if (!canUpdateMenu || !dragType) {
			resetDragState();
			return;
		}

		const completedType = dragType;
		try {
			if (completedType === 'item') await commitItemReorder();
			if (completedType === 'group') await commitGroupReorder();
			if (completedType === 'workspace') await commitWorkspaceReorder();
			toast.success('บันทึกลำดับสำเร็จ');
		} catch {
			toast.error('บันทึกลำดับไม่สำเร็จ');
			await loadData();
		} finally {
			resetDragState();
		}
	}

	function openItemDialog(item: MenuItem) {
		if (!canUpdateMenu) return;
		editingItem = item;
		itemDialogOpen = true;
	}

	async function handleDeleteItem(item: MenuItem) {
		if (!canDeleteMenu || !confirm(`ต้องการลบเมนู "${item.name}" ใช่หรือไม่?`)) return;
		try {
			await deleteMenuItem(item.id);
			items = items.filter((current) => current.id !== item.id);
			rebuildContainers();
			toast.success('ลบเมนูสำเร็จ');
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ไม่สามารถลบเมนูได้');
		}
	}
</script>

<MobileDragDropPolyfill />

<PageShell
	title="จัดโครงสร้างเมนูบริการ"
	description="กำหนดกลุ่มบริหาร ฝ่าย/งาน และตำแหน่งเมนู โดยไม่เปลี่ยนสิทธิ์การเข้าถึง"
>
	{#if !canReadMenu}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ดูโครงสร้างเมนู"
			description="บัญชีนี้ยังไม่มีสิทธิ์ menu.read.all"
		/>
	{:else}
		<div class="flex justify-end">
			<AcademicMenuTemplateDialog
				bind:open={academicTemplateDialogOpen}
				canApply={canUpdateMenu}
				onApplied={loadData}
			/>
		</div>
		<Tabs.Root bind:value={activeTab}>
			<Tabs.List class="grid w-full max-w-xl grid-cols-3">
				<Tabs.Trigger value="items">เมนูบริการ</Tabs.Trigger>
				<Tabs.Trigger value="groups">ฝ่าย/งาน</Tabs.Trigger>
				<Tabs.Trigger value="workspaces">กลุ่มบริหาร</Tabs.Trigger>
			</Tabs.List>

			<Tabs.Content value="items" class="space-y-4">
				<div class="flex flex-wrap items-center gap-3 rounded-xl border bg-card p-3">
					<span class="text-sm font-medium">ประเภทผู้ใช้</span>
					<Select.Root type="single" bind:value={userTypeFilter}>
						<Select.Trigger class="w-full sm:w-[190px]">
							{userTypeFilter === 'all'
								? 'ทั้งหมด'
								: userTypeFilter === 'staff'
									? 'บุคลากร'
									: userTypeFilter === 'student'
										? 'นักเรียน'
										: 'ผู้ปกครอง'}
						</Select.Trigger>
						<Select.Content>
							<Select.Item value="all">ทั้งหมด</Select.Item>
							<Select.Item value="staff">บุคลากร</Select.Item>
							<Select.Item value="student">นักเรียน</Select.Item>
							<Select.Item value="parent">ผู้ปกครอง</Select.Item>
						</Select.Content>
					</Select.Root>
					<p class="text-xs text-muted-foreground sm:ml-auto">ลากเมนูเพื่อเรียงหรือย้ายฝ่าย/งาน</p>
				</div>

				{#if loading}
					<PageSkeleton variant="cards" rows={4} />
				{:else if displayContainers.length === 0}
					<PageState
						title="ไม่พบเมนูบริการ"
						description="ลองเปลี่ยนประเภทผู้ใช้ หรือตรวจสอบรายการ route ของระบบ"
					/>
				{:else}
					<div class="space-y-6 pb-20">
						{#each displayContainers as { data, nesteds } (data.id)}
							<MenuGroupContainer
								{data}
								itemCount={nesteds.length}
								draggable={false}
								onDragOver={handleGroupDragOver}
								onDrop={handleGroupDrop}
							>
								<div class="mb-2 flex items-center gap-2 px-2 text-xs text-muted-foreground">
									<span>{workspaceNameByCode.get(data.workspace_code) ?? data.workspace_code}</span>
									<span aria-hidden="true">/</span>
									<span>{data.name}</span>
								</div>
								{#each nesteds as item (item.id)}
									<SortableItem
										{item}
										onEdit={openItemDialog}
										onDelete={handleDeleteItem}
										canUpdate={canUpdateMenu}
										canDelete={canDeleteMenu}
										canReorder={canUpdateMenu}
										onDragStart={handleItemDragStart}
										onDragEnter={handleItemDragEnter}
										onDragEnd={handleDragEnd}
									/>
								{:else}
									<div class="rounded-lg border-2 border-dashed p-8 text-center">
										<p class="text-sm text-muted-foreground">ยังไม่มีเมนูในฝ่าย/งานนี้</p>
									</div>
								{/each}
							</MenuGroupContainer>
						{/each}
					</div>
				{/if}
			</Tabs.Content>

			<Tabs.Content value="groups" class="space-y-4">
				{#if canCreateMenu}
					<div class="flex justify-end">
						<Button
							onclick={() => {
								editingGroup = null;
								groupDialogOpen = true;
							}}>สร้างฝ่าย/งาน</Button
						>
					</div>
				{/if}

				{#if loading}
					<PageSkeleton variant="cards" rows={3} />
				{:else}
					<div class="space-y-6">
						{#each groupedWorkspaces as entry (entry.workspace.id)}
							{@const WorkspaceIcon = getIconComponent(entry.workspace.icon)}
							<section class="space-y-3">
								<div class="flex items-center gap-2">
									<WorkspaceIcon class="h-5 w-5 text-primary" />
									<h2 class="font-semibold">{entry.workspace.name}</h2>
									<Badge variant="secondary">{entry.groups.length} ฝ่าย/งาน</Badge>
								</div>

								{#if entry.groups.length === 0}
									<div class="rounded-xl border border-dashed p-5 text-sm text-muted-foreground">
										ยังไม่มีฝ่าย/งานในกลุ่มบริหารนี้
									</div>
								{:else}
									<div class="grid gap-3">
										{#each entry.groups as group (group.id)}
											<div
												role="listitem"
												draggable={canUpdateMenu}
												ondragstart={(event) => handleGroupDragStart(event, group)}
												ondragenter={(event) => handleGroupDragEnter(event, group)}
												ondragend={handleDragEnd}
												class={canUpdateMenu ? 'cursor-grab active:cursor-grabbing' : ''}
											>
												<Card class="p-4">
													<div class="flex items-center gap-3">
														{#if canUpdateMenu}
															<GripVertical class="h-5 w-5 text-muted-foreground" />
														{/if}
														<div class="min-w-0 flex-1">
															<div class="flex flex-wrap items-center gap-2">
																<h3 class="font-semibold">{group.name}</h3>
																{#if !group.is_active}
																	<Badge variant="secondary">ปิดใช้งาน</Badge>
																{/if}
															</div>
															<code class="text-xs text-muted-foreground">{group.code}</code>
														</div>
														{#if canUpdateMenu}
															<Button
																size="sm"
																variant="outline"
																onclick={() => {
																	editingGroup = group;
																	groupDialogOpen = true;
																}}
															>
																<Pencil class="h-4 w-4" />
																แก้ไข
															</Button>
														{/if}
													</div>
												</Card>
											</div>
										{/each}
									</div>
								{/if}
							</section>
						{/each}
					</div>
				{/if}
			</Tabs.Content>

			<Tabs.Content value="workspaces" class="space-y-4">
				<div class="flex items-center justify-between gap-4">
					<p class="text-sm text-muted-foreground">
						ลากเพื่อกำหนดลำดับหมวดระดับบนสุดในหน้าหลักและ Sidebar
					</p>
					{#if canCreateMenu}
						<Button
							onclick={() => {
								editingWorkspace = null;
								workspaceDialogOpen = true;
							}}>สร้างกลุ่มบริหาร</Button
						>
					{/if}
				</div>

				{#if loading}
					<PageSkeleton variant="cards" rows={3} />
				{:else if workspaces.length === 0}
					<PageState
						title="ยังไม่มีกลุ่มบริหาร"
						description="สร้างกลุ่มบริหารเพื่อเริ่มจัดหมวดบริการของโรงเรียน"
					/>
				{:else}
					<div class="grid gap-3">
						{#each workspaces as workspace (workspace.id)}
							{@const WorkspaceIcon = getIconComponent(workspace.icon)}
							<div
								role="listitem"
								draggable={canUpdateMenu}
								ondragstart={(event) => handleWorkspaceDragStart(event, workspace)}
								ondragenter={(event) => handleWorkspaceDragEnter(event, workspace)}
								ondragend={handleDragEnd}
								class={canUpdateMenu ? 'cursor-grab active:cursor-grabbing' : ''}
							>
								<Card class="p-4">
									<div class="flex items-center gap-3">
										{#if canUpdateMenu}
											<GripVertical class="h-5 w-5 text-muted-foreground" />
										{/if}
										<div
											class="flex h-10 w-10 items-center justify-center rounded-lg bg-primary/10"
										>
											<WorkspaceIcon class="h-5 w-5 text-primary" />
										</div>
										<div class="min-w-0 flex-1">
											<div class="flex flex-wrap items-center gap-2">
												<h3 class="font-semibold">{workspace.name}</h3>
												{#if !workspace.is_active}
													<Badge variant="secondary">ปิดใช้งาน</Badge>
												{/if}
											</div>
											<div class="flex items-center gap-2 text-xs text-muted-foreground">
												<code>{workspace.code}</code>
												<span>•</span>
												<span>
													{groups.filter((group) => group.workspace_code === workspace.code).length}
													ฝ่าย/งาน
												</span>
											</div>
										</div>
										{#if canUpdateMenu}
											<Button
												size="sm"
												variant="outline"
												onclick={() => {
													editingWorkspace = workspace;
													workspaceDialogOpen = true;
												}}
											>
												<Pencil class="h-4 w-4" />
												แก้ไข
											</Button>
										{/if}
									</div>
								</Card>
							</div>
						{/each}
					</div>
				{/if}
			</Tabs.Content>
		</Tabs.Root>
	{/if}
</PageShell>

<GroupManagementDialog
	bind:open={groupDialogOpen}
	group={editingGroup}
	{workspaces}
	canCreate={canCreateMenu}
	canUpdate={canUpdateMenu}
	canDelete={canDeleteMenu}
	onSuccess={handleGroupMutation}
	onOpenChange={(open) => (groupDialogOpen = open)}
/>

<WorkspaceManagementDialog
	bind:open={workspaceDialogOpen}
	workspace={editingWorkspace}
	canCreate={canCreateMenu}
	canUpdate={canUpdateMenu}
	canDelete={canDeleteMenu}
	onSuccess={handleWorkspaceMutation}
	onOpenChange={(open) => (workspaceDialogOpen = open)}
/>

<MenuItemManagementDialog
	bind:open={itemDialogOpen}
	item={editingItem}
	{groups}
	{workspaces}
	canUpdate={canUpdateMenu}
	onSuccess={replaceMenuItem}
	onOpenChange={(open) => (itemDialogOpen = open)}
/>
