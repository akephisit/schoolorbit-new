<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import type { PageProps } from './$types';
	import { appIdentityKey } from '$lib/auth/settled-user';
	import { authStore } from '$lib/stores/auth';
	import { LatestRequest } from '$lib/async/latest-request';
	import { captureRouteLoad } from '$lib/navigation/route-load';
	import { getAppMenuRegion } from '$lib/navigation/app-menu.svelte';
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
	import { GripVertical, Pencil } from '@lucide/svelte';
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
	let { data }: PageProps = $props();
	const workspaceSource = $derived(data.workspaces),
		groupSource = $derived(data.groups),
		itemSource = $derived(data.items);
	const workspaceRequest = new LatestRequest(),
		groupRequest = new LatestRequest(),
		itemRequest = new LatestRequest();
	let workspaceState = $state({ loading: true, loaded: false, error: '' }),
		groupState = $state({ loading: true, loaded: false, error: '' }),
		itemState = $state({ loading: true, loaded: false, error: '' });
	let owner = $state(''),
		ownerEpoch = $state(0),
		disposed = false,
		reordering = $state(false);
	const appMenu = getAppMenuRegion();
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

	$effect.pre(() => {
		const identity = `${$authStore.user?.id ?? ''}|${canReadMenu}|${canCreateMenu}|${canUpdateMenu}|${canDeleteMenu}`;
		untrack(() => {
			if (owner === identity) return;
			owner = identity;
			ownerEpoch++;
			reordering = false;
			workspaces = [];
			groups = [];
			items = [];
			containers = [];
			workspaceState.loaded = false;
			workspaceState.loading = true;
			workspaceState.error = '';
			groupState.loaded = false;
			groupState.loading = true;
			groupState.error = '';
			itemState.loaded = false;
			itemState.loading = true;
			itemState.error = '';
			workspaceRequest.abort();
			groupRequest.abort();
			itemRequest.abort();
			groupDialogOpen = false;
			workspaceDialogOpen = false;
			itemDialogOpen = false;
			academicTemplateDialogOpen = false;
			resetDragState();
		});
	});
	$effect.pre(() => {
		const operation = workspaceSource;
		untrack(() => {
			const t = workspaceRequest.begin();
			workspaceState.loading = true;
			workspaceState.error = '';
			void operation.then((r) => applyWorkspaces(r, t.revision));
		});
		return () => workspaceRequest.abort();
	});
	$effect.pre(() => {
		const operation = groupSource;
		untrack(() => {
			const t = groupRequest.begin();
			groupState.loading = true;
			groupState.error = '';
			void operation.then((r) => applyGroups(r, t.revision));
		});
		return () => groupRequest.abort();
	});
	$effect.pre(() => {
		const operation = itemSource;
		untrack(() => {
			const t = itemRequest.begin();
			itemState.loading = true;
			itemState.error = '';
			void operation.then((r) => applyItems(r, t.revision));
		});
		return () => itemRequest.abort();
	});
	onDestroy(() => {
		disposed = true;
		ownerEpoch++;
		workspaceRequest.abort();
		groupRequest.abort();
		itemRequest.abort();
	});
	function applyWorkspaces(r: Awaited<typeof data.workspaces>, revision: number) {
		if (!workspaceRequest.isCurrent(revision)) return;
		workspaceState.loading = false;
		if (!r.ok) {
			workspaceState.error = r.error;
			return;
		}
		if (r.data.identityKey !== appIdentityKey()) return;
		workspaces = r.data.records ?? [];
		workspaceState.loaded = true;
		sortAdministrationData();
		rebuildContainers();
	}
	function applyGroups(r: Awaited<typeof data.groups>, revision: number) {
		if (!groupRequest.isCurrent(revision)) return;
		groupState.loading = false;
		if (!r.ok) {
			groupState.error = r.error;
			return;
		}
		if (r.data.identityKey !== appIdentityKey()) return;
		groups = r.data.records ?? [];
		groupState.loaded = true;
		sortAdministrationData();
		rebuildContainers();
	}
	function applyItems(r: Awaited<typeof data.items>, revision: number) {
		if (!itemRequest.isCurrent(revision)) return;
		itemState.loading = false;
		if (!r.ok) {
			itemState.error = r.error;
			return;
		}
		if (r.data.identityKey !== appIdentityKey()) return;
		items = r.data.records ?? [];
		itemState.loaded = true;
		rebuildContainers();
	}
	async function refreshWorkspaces() {
		if (disposed || !canReadMenu) return;
		const identityKey = appIdentityKey(),
			t = workspaceRequest.begin();
		workspaceState.loading = true;
		workspaceState.error = '';
		applyWorkspaces(
			await captureRouteLoad(
				listMenuWorkspaces({ signal: t.signal }).then((records) => ({ identityKey, records })),
				'โหลดกลุ่มบริหารไม่สำเร็จ'
			),
			t.revision
		);
	}
	async function refreshGroups() {
		if (disposed || !canReadMenu) return;
		const identityKey = appIdentityKey(),
			t = groupRequest.begin();
		groupState.loading = true;
		groupState.error = '';
		applyGroups(
			await captureRouteLoad(
				listMenuGroups({ signal: t.signal }).then((records) => ({ identityKey, records })),
				'โหลดฝ่าย/งานไม่สำเร็จ'
			),
			t.revision
		);
	}
	async function refreshItems() {
		if (disposed || !canReadMenu) return;
		const identityKey = appIdentityKey(),
			t = itemRequest.begin();
		itemState.loading = true;
		itemState.error = '';
		applyItems(
			await captureRouteLoad(
				listMenuItems(undefined, { signal: t.signal }).then((records) => ({
					identityKey,
					records
				})),
				'โหลดเมนูบริการไม่สำเร็จ'
			),
			t.revision
		);
	}
	async function handleTemplateApplied() {
		if (disposed || !canReadMenu) return;
		await Promise.all([refreshWorkspaces(), refreshGroups(), refreshItems(), appMenu.retry()]);
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
		if (disposed || !canReadMenu) return;
		groupRequest.abort();
		groupState.loading = false;
		groupState.error = '';
		groups = groups.some((current) => current.id === group.id)
			? groups.map((current) => (current.id === group.id ? group : current))
			: [...groups, group];
		sortAdministrationData();
		rebuildContainers();
	}

	function replaceMenuItem(item: MenuItem) {
		if (disposed || !canReadMenu) return;
		itemRequest.abort();
		itemState.loading = false;
		itemState.error = '';
		items = items.map((current) => (current.id === item.id ? item : current));
		rebuildContainers();
	}

	function handleGroupMutation(
		result: { type: 'upsert'; group: MenuGroup } | { type: 'delete'; groupId: string }
	) {
		if (disposed || !canReadMenu) return;
		if (result.type === 'upsert') {
			replaceMenuGroup(result.group);
			void appMenu.retry();
			return;
		}
		groupRequest.abort();
		groupState.loading = false;
		groups = groups.filter((g) => g.id !== result.groupId);
		rebuildContainers();
		void refreshItems();
		void appMenu.retry();
	}

	function handleWorkspaceMutation(
		result: { type: 'upsert'; workspace: MenuWorkspace } | { type: 'delete'; workspaceId: string }
	) {
		if (disposed || !canReadMenu) return;
		workspaceRequest.abort();
		workspaceState.loading = false;
		workspaceState.error = '';
		if (result.type === 'upsert') {
			workspaces = workspaces.some((current) => current.id === result.workspace.id)
				? workspaces.map((current) =>
						current.id === result.workspace.id ? result.workspace : current
					)
				: [...workspaces, result.workspace];
			sortAdministrationData();
			rebuildContainers();
			void appMenu.retry();
			return;
		}
		workspaces = workspaces.filter((w) => w.id !== result.workspaceId);
		sortAdministrationData();
		rebuildContainers();
		void refreshGroups();
		void appMenu.retry();
	}

	function handleItemDragStart(event: DragEvent, item: MenuItem) {
		if (!canUpdateMenu || reordering || activeTab !== 'items') return;
		event.dataTransfer?.setData('text/plain', item.id);
		if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move';
		draggedItem = item;
		dragType = 'item';
	}

	function handleItemDragEnter(_event: DragEvent, targetItem: MenuItem) {
		if (!canUpdateMenu || reordering || dragType !== 'item' || !draggedItem) return;
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
		if (!canUpdateMenu || reordering || dragType !== 'item' || !draggedItem) return;
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

		const snapshot = containers.flatMap((container) =>
			container.nesteds.map((item, index) => ({
				...item,
				display_order: index + 1,
				group_id: container.data.id
			}))
		);
		await reorderMenuItems(payload);
		return snapshot;
	}

	function handleGroupDragStart(event: DragEvent, group: MenuGroup) {
		if (!canUpdateMenu || reordering || activeTab !== 'groups') return;
		event.dataTransfer?.setData('text/plain', group.id);
		if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move';
		draggedGroup = group;
		dragType = 'group';
	}

	function handleGroupDragEnter(_event: DragEvent, targetGroup: MenuGroup) {
		if (!canUpdateMenu || reordering || dragType !== 'group' || !draggedGroup) return;
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
		const snapshot = [...groups];
		await reorderMenuGroups(payload);
		const orderById = new Map(payload.map((entry) => [entry.id, entry.display_order]));
		return snapshot.map((group) => ({
			...group,
			display_order: orderById.get(group.id) ?? group.display_order
		}));
	}

	function handleWorkspaceDragStart(event: DragEvent, workspace: MenuWorkspace) {
		if (!canUpdateMenu || reordering || activeTab !== 'workspaces') return;
		event.dataTransfer?.setData('text/plain', workspace.id);
		if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move';
		draggedWorkspace = workspace;
		dragType = 'workspace';
	}

	function handleWorkspaceDragEnter(_event: DragEvent, targetWorkspace: MenuWorkspace) {
		if (!canUpdateMenu || reordering || dragType !== 'workspace' || !draggedWorkspace) return;
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
		const snapshot = [...workspaces];
		await reorderMenuWorkspaces(payload);
		return snapshot.map((workspace, index) => ({
			...workspace,
			display_order: index + 1
		}));
	}

	async function handleDragEnd(event: DragEvent) {
		event.preventDefault();
		if (!canUpdateMenu || reordering || !dragType) {
			resetDragState();
			return;
		}

		const completedType = dragType,
			epoch = ownerEpoch;
		reordering = true;
		const current = () => !disposed && epoch === ownerEpoch && canUpdateMenu;
		const affected =
			completedType === 'item'
				? itemRequest
				: completedType === 'group'
					? groupRequest
					: workspaceRequest;
		const affectedState =
			completedType === 'item'
				? itemState
				: completedType === 'group'
					? groupState
					: workspaceState;
		affected.abort();
		affectedState.loading = false;
		try {
			if (completedType === 'item') {
				const saved = await commitItemReorder();
				if (!current()) return;
				if (saved) items = saved;
			}
			if (completedType === 'group') {
				const saved = await commitGroupReorder();
				if (!current()) return;
				groups = saved;
			}
			if (completedType === 'workspace') {
				const saved = await commitWorkspaceReorder();
				if (!current()) return;
				workspaces = saved;
			}
			affected.abort();
			affectedState.loading = false;
			affectedState.error = '';
			sortAdministrationData();
			rebuildContainers();
			void appMenu.retry();
			toast.success('บันทึกลำดับสำเร็จ');
		} catch {
			if (!current()) return;
			toast.error('บันทึกลำดับไม่สำเร็จ');
			if (completedType === 'item') await refreshItems();
			if (completedType === 'group') await refreshGroups();
			if (completedType === 'workspace') await refreshWorkspaces();
		} finally {
			if (current()) {
				reordering = false;
				resetDragState();
			}
		}
	}

	function openItemDialog(item: MenuItem) {
		if (!canUpdateMenu || reordering || !groupState.loaded || !workspaceState.loaded) return;
		editingItem = item;
		itemDialogOpen = true;
	}

	async function handleDeleteItem(item: MenuItem) {
		if (
			disposed ||
			reordering ||
			!canDeleteMenu ||
			!confirm(`ต้องการลบเมนู "${item.name}" ใช่หรือไม่?`)
		)
			return;
		const epoch = ownerEpoch;
		const current = () => !disposed && epoch === ownerEpoch && canDeleteMenu;
		try {
			await deleteMenuItem(item.id);
			if (!current()) return;
			itemRequest.abort();
			itemState.loading = false;
			itemState.error = '';
			items = items.filter((current) => current.id !== item.id);
			rebuildContainers();
			void appMenu.retry();
			toast.success('ลบเมนูสำเร็จ');
		} catch (error) {
			if (current()) toast.error(error instanceof Error ? error.message : 'ไม่สามารถลบเมนูได้');
		}
	}
</script>

{#snippet catalogStatus(
	label: string,
	state: { loading: boolean; loaded: boolean; error: string },
	retry: () => Promise<void>
)}
	{#if state.loading}{#if state.loaded}<p role="status">กำลังอัปเดต{label}</p>{:else}<div
				role="status"
				aria-label={`กำลังโหลด${label}`}
			>
				<PageSkeleton variant="cards" rows={2} />
			</div>{/if}{/if}
	{#if state.error}<PageState
			variant="error"
			title={`โหลด${label}ไม่สำเร็จ`}
			description={state.error}
			actionLabel={`ลองโหลด${label}อีกครั้ง`}
			onaction={() => void retry()}
		/>{/if}
{/snippet}
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
				onApplied={handleTemplateApplied}
				canPreview={canReadMenu}
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

				<section data-testid="menu-items">
					{#if !itemState.loaded || itemState.error || itemState.loading}{@render catalogStatus(
							'เมนูบริการ',
							itemState,
							refreshItems
						)}{/if}
					{#if !groupState.loaded || groupState.error || groupState.loading}{@render catalogStatus(
							'ฝ่าย/งาน',
							groupState,
							refreshGroups
						)}{/if}
					{#if !workspaceState.loaded || workspaceState.error || workspaceState.loading}{@render catalogStatus(
							'กลุ่มบริหาร',
							workspaceState,
							refreshWorkspaces
						)}{/if}
					{#if itemState.loaded && groupState.loaded}
						{#if displayContainers.length === 0}
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
											<span
												>{workspaceNameByCode.get(data.workspace_code) ?? data.workspace_code}</span
											>
											<span aria-hidden="true">/</span>
											<span>{data.name}</span>
										</div>
										{#each nesteds as item (item.id)}
											<SortableItem
												{item}
												onEdit={openItemDialog}
												onDelete={handleDeleteItem}
												canUpdate={canUpdateMenu && !reordering}
												canDelete={canDeleteMenu && !reordering}
												canReorder={canUpdateMenu && !reordering}
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
					{/if}
				</section></Tabs.Content
			>

			<Tabs.Content value="groups" class="space-y-4">
				{#if canCreateMenu}
					<div class="flex justify-end">
						<Button
							disabled={!workspaceState.loaded ||
								!groupState.loaded ||
								Boolean(workspaceState.error) ||
								reordering}
							onclick={() => {
								editingGroup = null;
								groupDialogOpen = true;
							}}>สร้างฝ่าย/งาน</Button
						>
					</div>
				{/if}

				<section data-testid="menu-groups">
					{#if !groupState.loaded || groupState.error || groupState.loading}{@render catalogStatus(
							'ฝ่าย/งาน',
							groupState,
							refreshGroups
						)}{/if}
					{#if !workspaceState.loaded || workspaceState.error || workspaceState.loading}{@render catalogStatus(
							'กลุ่มบริหาร',
							workspaceState,
							refreshWorkspaces
						)}{/if}
					{#if groupState.loaded && workspaceState.loaded}
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
													draggable={canUpdateMenu && !reordering}
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
				</section></Tabs.Content
			>

			<Tabs.Content value="workspaces" class="space-y-4">
				<div class="flex items-center justify-between gap-4">
					<p class="text-sm text-muted-foreground">
						ลากเพื่อกำหนดลำดับหมวดระดับบนสุดในหน้าหลักและ Sidebar
					</p>
					{#if canCreateMenu}
						<Button
							disabled={!workspaceState.loaded || reordering}
							onclick={() => {
								editingWorkspace = null;
								workspaceDialogOpen = true;
							}}>สร้างกลุ่มบริหาร</Button
						>
					{/if}
				</div>

				<section data-testid="menu-workspaces">
					{#if !workspaceState.loaded || workspaceState.error || workspaceState.loading}{@render catalogStatus(
							'กลุ่มบริหาร',
							workspaceState,
							refreshWorkspaces
						)}{/if}
					{#if workspaceState.loaded}
						{#if workspaces.length === 0}
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
										draggable={canUpdateMenu && !reordering}
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
															{groupState.loaded
																? groups.filter((group) => group.workspace_code === workspace.code)
																		.length
																: '…'}
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
					{/if}
				</section></Tabs.Content
			>
		</Tabs.Root>
	{/if}
</PageShell>

{#key owner}
	{@const dialogOwner = ownerEpoch}
	{#if groupDialogOpen && canReadMenu && workspaceState.loaded && !workspaceState.error && !reordering}
		<GroupManagementDialog
			bind:open={groupDialogOpen}
			group={editingGroup}
			{workspaces}
			canCreate={canCreateMenu}
			canUpdate={canUpdateMenu && !reordering}
			canDelete={canDeleteMenu && !reordering}
			onSuccess={(result) => {
				if (dialogOwner === ownerEpoch) handleGroupMutation(result);
			}}
			onOpenChange={(open) => (groupDialogOpen = open)}
		/>
	{/if}
	{#if workspaceDialogOpen && canReadMenu && !reordering}
		<WorkspaceManagementDialog
			bind:open={workspaceDialogOpen}
			workspace={editingWorkspace}
			canCreate={canCreateMenu}
			canUpdate={canUpdateMenu && !reordering}
			canDelete={canDeleteMenu && !reordering}
			onSuccess={(result) => {
				if (dialogOwner === ownerEpoch) handleWorkspaceMutation(result);
			}}
			onOpenChange={(open) => (workspaceDialogOpen = open)}
		/>
	{/if}
	{#if itemDialogOpen && canReadMenu && groupState.loaded && workspaceState.loaded && !reordering}
		<MenuItemManagementDialog
			bind:open={itemDialogOpen}
			item={editingItem}
			{groups}
			{workspaces}
			canUpdate={canUpdateMenu && !reordering}
			onSuccess={(item) => {
				if (dialogOwner !== ownerEpoch) return;
				replaceMenuItem(item);
				if (!disposed) void appMenu.retry();
			}}
			onOpenChange={(open) => (itemDialogOpen = open)}
		/>
	{/if}
{/key}
