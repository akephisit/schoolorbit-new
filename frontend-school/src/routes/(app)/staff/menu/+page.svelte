<script lang="ts">
	import {
		listMenuGroups,
		listMenuItems,
		deleteMenuItem,
		reorderMenuItems,
		reorderMenuGroups,
		moveItemToGroup,
		type MenuGroup,
		type MenuItem
	} from '$lib/api/menu-admin';
	import { Button } from '$lib/components/ui/button';
	import { Card } from '$lib/components/ui/card';
	import * as Tabs from '$lib/components/ui/tabs';
	import { LoaderCircle, FolderOpen } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';
	import {
		DndContext,
		DragOverlay,
		PointerSensor,
		KeyboardSensor,
		closestCenter,
		type DragStartEvent,
		type DragEndEvent
	} from '@dnd-kit-svelte/core';
	import { SortableContext, arrayMove } from '@dnd-kit-svelte/sortable';
	import SortableItem from '$lib/components/menu/SortableItem.svelte';
	import MenuGroupContainer from '$lib/components/menu/MenuGroupContainer.svelte';
	import Droppable from '$lib/components/menu/Droppable.svelte';
	import GroupManagementDialog from '$lib/components/menu/GroupManagementDialog.svelte';

	// Container structure matching dnd-kit example
	type GroupContainer = {
		data: MenuGroup;
		nesteds: MenuItem[];
	};

	let groups = $state<MenuGroup[]>([]);
	let items = $state<MenuItem[]>([]);
	let loading = $state(true);
	let activeTab = $state('items'); // 'items' or 'groups'

	// Drag state
	let activeItem = $state<MenuItem | GroupContainer | null>(null);
	let activeType = $state<'group' | 'item' | null>(null);
	let originalGroupId = $state<string | null>(null); // Track original group before onDragOver

	// Dialog states
	let groupDialogOpen = $state(false);
	let editingGroup = $state<MenuGroup | null>(null);
	
	// Filter state
	let userTypeFilter = $state<'all' | 'staff' | 'student' | 'parent'>('all');

	// Use state instead of derived to allow direct mutation in onDragOver
	let containers = $state<GroupContainer[]>([]);

	// Load data on mount
	$effect(() => {
		loadData();
	});

	async function loadData() {
		try {
			loading = true;
			[groups, items] = await Promise.all([listMenuGroups(), listMenuItems()]);

			// Rebuild containers from loaded data
			containers = groups.map((group) => ({
				data: group,
				nesteds: items.filter((item) => item.group_id === group.id)
			}));
		} catch (error) {
			const message = error instanceof Error ? error.message : 'ไม่สามารถโหลดข้อมูลได้';
			toast.error(message);
		} finally {
			loading = false;
		}
	}
	
	// Filtered containers based on user_type filter
	const filteredContainers = $derived(
		userTypeFilter === 'all'
			? containers
			: containers.map(container => ({
				...container,
				nesteds: container.nesteds.filter(item => item.user_type === userTypeFilter)
			})).filter(container => container.nesteds.length > 0)
	);

	// Helper functions
	function isContainerItem(item: MenuItem | GroupContainer | null): item is GroupContainer {
		return item !== null && 'nesteds' in item;
	}

	function findContainer(id: string): GroupContainer | null {
		return (
			containers.find(
				(container) => container.data.id === id || container.nesteds.some((item) => item.id === id)
			) || null
		);
	}

	// DnD sensors
	const sensors = [
		{ sensor: PointerSensor, options: {} },
		{ sensor: KeyboardSensor, options: {} }
	];

	// Drag handlers
	function handleDragStart({ active }: DragStartEvent) {
		const type = active.data?.type as 'group' | 'item';
		activeType = type;

		if (type === 'group') {
			activeItem = containers.find((c) => c.data.id === active.id) || null;
			originalGroupId = null;
		} else {
			const container = findContainer(active.id as string);
			activeItem = container?.nesteds.find((item) => item.id === active.id) || null;
			// Store original group_id BEFORE onDragOver changes it
			originalGroupId = container?.data.id || null;
		}
	}

	async function handleDragEnd({ active, over }: DragEndEvent) {
		if (!over) return;

		const activeType = active.data?.type as 'group' | 'item';
		const overType = over.data?.type as 'group' | 'item' | undefined;
		const acceptsItem = over.data?.accepts?.includes('item') ?? false;

		// Case 1: Reorder groups
		if (activeType === 'group' && (overType === 'group' || over.data?.accepts?.includes('group'))) {
			const oldIndex = groups.findIndex((g) => g.id === active.id);
			const newIndex = groups.findIndex((g) => g.id === over.id);

			if (oldIndex !== newIndex) {
				groups = arrayMove(groups, oldIndex, newIndex);

				try {
					await reorderMenuGroups(groups.map((g, i) => ({ id: g.id, display_order: i + 1 })));
					toast.success('เรียงกลุ่มสำเร็จ');
				} catch {
					toast.error('ไม่สามารถเรียงกลุ่มได้');
					await loadData();
				}
			}
			return;
		}

		// Case 2: Move/reorder items
		if (activeType === 'item' && (overType === 'item' || acceptsItem)) {
			const overContainer = findContainer(over.id as string);
			if (!overContainer) return;

			// Check if moved to different group using originalGroupId
			if (originalGroupId && originalGroupId !== overContainer.data.id) {
				// Cross-group move
				try {
					await moveItemToGroup(active.id as string, overContainer.data.id);
					toast.success('ย้ายเมนูสำเร็จ');
					await loadData();
				} catch {
					toast.error('ไม่สามารถย้ายเมนูได้');
					await loadData();
				}
			} else {
				// Same-group reorder
				const groupItems = overContainer.nesteds;
				const oldIndex = groupItems.findIndex((item) => item.id === active.id);
				const newIndex = groupItems.findIndex((item) => item.id === over.id);

				if (oldIndex !== -1 && newIndex !== -1 && oldIndex !== newIndex) {
					const reordered = arrayMove(groupItems, oldIndex, newIndex);
					const withOrder = reordered.map((item, index) => ({
						...item,
						display_order: index + 1
					}));

					try {
						await reorderMenuItems(
							withOrder.map((i) => ({ id: i.id, display_order: i.display_order }))
						);
						toast.success('เรียงลำดับสำเร็จ');
						await loadData();
					} catch {
						toast.error('ไม่สามารถเรียงลำดับได้');
						await loadData();
					}
				}
			}
		}
	}

	function handleDragOver() {
		// CONFIRMED: Cannot implement real-time preview in Svelte 5
		// Even with untrack(), mutations trigger infinite loops
		// This is a fundamental difference from vanilla JS/React
		// DragOverlay provides sufficient visual feedback
	}

	function openEditDialog(item: MenuItem) {
		// TODO: implement if needed
		console.log('Edit:', item);
	}

	async function handleDelete(item: MenuItem) {
		if (!confirm(`ต้องการลบเมนู "${item.name}" ใช่หรือไม่?`)) return;

		try {
			await deleteMenuItem(item.id);
			toast.success('ลบเมนูสำเร็จ');
			await loadData();
		} catch {
			toast.error('ไม่สามารถลบเมนูได้');
		}
	}
</script>

<svelte:head>
	<title>จัดการเมนู - Menu Management</title>
</svelte:head>

<div class="space-y-6">
	<!-- Header -->
	<div class="flex items-center justify-between">
		<div>
			<h1 class="text-3xl font-bold">จัดการเมนู</h1>
			<p class="text-muted-foreground mt-1">จัดการโครงสร้างเมนูและกลุ่มเมนู</p>
		</div>
	</div>

	<!-- Filter -->
	<div class="flex items-center gap-3">
		<label for="user-type-filter" class="text-sm font-medium">กรองตาม User Type:</label>
		<select
			id="user-type-filter"
			bind:value={userTypeFilter}
			class="px-3 py-2 border rounded-md bg-background text-sm"
		>
			<option value="all">ทั้งหมด</option>
			<option value="staff">👔 Staff</option>
			<option value="student">🎓 Student</option>
			<option value="parent">👨‍👩‍👧 Parent</option>
		</select>
	</div>


	<!-- Tabs -->
	<Tabs.Root bind:value={activeTab}>
		<Tabs.List class="grid w-full grid-cols-2 max-w-md">
			<Tabs.Trigger value="items">รายการเมนู</Tabs.Trigger>
			<Tabs.Trigger value="groups">กลุ่มเมนู</Tabs.Trigger>
		</Tabs.List>

		<!-- ITEMS TAB -->
		<Tabs.Content value="items" class="space-y-4">
			{#if loading}
				<div class="flex justify-center items-center py-20">
					<LoaderCircle class="h-8 w-8 animate-spin text-primary" />
				</div>
			{:else if filteredContainers.length === 0}
				<Card class="p-12 text-center">
					<FolderOpen class="h-16 w-16 mx-auto mb-4 opacity-20" />
					<p class="text-lg">ไม่พบกลุ่มเมนู</p>
				</Card>
			{:else}
				<DndContext
					{sensors}
					collisionDetection={closestCenter}
					onDragStart={handleDragStart}
					onDragEnd={handleDragEnd}
					onDragOver={handleDragOver}
				>
					<SortableContext items={filteredContainers.map((c) => c.data.id)}>
						<Droppable id="groups-container" data={{ accepts: ['group'] }}>
							<div class="space-y-6">
								{#each filteredContainers as { data, nesteds } (data.id)}
									<MenuGroupContainer
										{data}
										type="group"
										accepts={['item']}
										itemCount={nesteds.length}
									>
										<SortableContext items={nesteds.map((item) => item.id)}>
											<div class="space-y-2">
												{#each nesteds as item (item.id)}
													<SortableItem {item} onEdit={openEditDialog} onDelete={handleDelete} />
												{:else}
													<p class="text-sm text-center text-muted-foreground py-4">ไม่มีรายการ</p>
												{/each}
											</div>
										</SortableContext>
									</MenuGroupContainer>
								{/each}
							</div>
						</Droppable>
					</SortableContext>

					<DragOverlay>
						{#if activeType === 'item' && activeItem && !isContainerItem(activeItem)}
							<SortableItem item={activeItem} onEdit={openEditDialog} onDelete={handleDelete} />
						{:else if activeType === 'group' && activeItem && isContainerItem(activeItem)}
							<MenuGroupContainer
								data={activeItem.data}
								type="group"
								accepts={['item']}
								itemCount={activeItem.nesteds.length}
								class="shadow-xl"
							>
								<div class="space-y-2">
									{#each activeItem.nesteds as item (item.id)}
										<SortableItem {item} onEdit={openEditDialog} onDelete={handleDelete} />
									{/each}
								</div>
							</MenuGroupContainer>
						{/if}
					</DragOverlay>
				</DndContext>
			{/if}
		</Tabs.Content>

		<!-- GROUPS TAB -->
		<Tabs.Content value="groups" class="space-y-4">
			<div class="flex justify-end">
				<Button
					onclick={() => {
						editingGroup = null;
						groupDialogOpen = true;
					}}
				>
					สร้างกลุ่มใหม่
				</Button>
			</div>

			{#if loading}
				<div class="flex justify-center py-12">
					<LoaderCircle class="h-8 w-8 animate-spin" />
				</div>
			{:else}
				<div class="grid gap-3">
					{#each groups as group (group.id)}
						{@const itemCount = items.filter((i) => i.group_id === group.id).length}
						<Card class="p-4">
							<div class="flex items-center gap-3">
								<div class="flex-1">
									<div class="flex items-center gap-2">
										<h3 class="font-semibold">{group.name}</h3>
										{#if group.name_en}
											<span class="text-sm text-muted-foreground">({group.name_en})</span>
										{/if}
									</div>
									<div class="flex items-center gap-2 mt-1">
										<code class="text-xs bg-muted px-2 py-0.5 rounded">{group.code}</code>
										<span class="text-sm text-muted-foreground">{itemCount} รายการ</span>
									</div>
								</div>
								<Button
									size="sm"
									variant="outline"
									onclick={() => {
										editingGroup = group;
										groupDialogOpen = true;
									}}
								>
									แก้ไข
								</Button>
							</div>
						</Card>
					{/each}
				</div>
			{/if}
		</Tabs.Content>
	</Tabs.Root>
</div>

<!-- Group Management Dialog -->
<GroupManagementDialog
	bind:open={groupDialogOpen}
	group={editingGroup}
	onSuccess={loadData}
	onOpenChange={(open) => (groupDialogOpen = open)}
/>
