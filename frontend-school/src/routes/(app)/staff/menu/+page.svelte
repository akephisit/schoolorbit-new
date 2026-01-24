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
	import { LoaderCircle, FolderOpen, GripVertical } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';
	
    // Remove dnd-kit imports
	import SortableItem from '$lib/components/menu/SortableItem.svelte';
	import MenuGroupContainer from '$lib/components/menu/MenuGroupContainer.svelte';
	import GroupManagementDialog from '$lib/components/menu/GroupManagementDialog.svelte';

	// Container structure
	type GroupContainer = {
		data: MenuGroup;
		nesteds: MenuItem[];
	};

	let groups = $state<MenuGroup[]>([]);
	let items = $state<MenuItem[]>([]);
	let loading = $state(true);
	let activeTab = $state('items'); // 'items' or 'groups'

	// Dialog states
	let groupDialogOpen = $state(false);
	let editingGroup = $state<MenuGroup | null>(null);

	// Filter state
	let userTypeFilter = $state<'all' | 'staff' | 'student' | 'parent'>('all');

	// Local State for Drag & Drop
	// ----------------------------------------------------
    let containers = $state<GroupContainer[]>([]);
	
    // We keep track of the Item or Group being dragged
    let draggedItem = $state<MenuItem | null>(null);
    let draggedGroup = $state<MenuGroup | null>(null);
    let dragType = $state<'item' | 'group' | null>(null);

	// Load data on mount
	$effect(() => {
		loadData();
	});

	async function loadData() {
		try {
			loading = true;
			[groups, items] = await Promise.all([listMenuGroups(), listMenuItems()]);
            rebuildContainers();
		} catch (error) {
			const message = error instanceof Error ? error.message : 'ไม่สามารถโหลดข้อมูลได้';
			toast.error(message);
		} finally {
			loading = false;
		}
	}
    
    function rebuildContainers() {
        // Sort items by display_order
        const sortedItems = [...items].sort((a,b) => a.display_order - b.display_order);
        
        // Sort groups by display_order
        const sortedGroups = [...groups].sort((a,b) => a.display_order - b.display_order);

        containers = sortedGroups.map((group) => ({
            data: group,
            nesteds: sortedItems.filter((item) => item.group_id === group.id)
        }));
    }

	// Filtered for display
    // Note: Mutating 'containers' directly maps to view. 
    // If filtering is active, we might limit interaction scope.
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
    
    // ===========================================
    // NATIVE DRAG & DROP LOGIC
    // ===========================================
    
    // --- 1. MENU ITEMS REORDERING ---

    function handleItemDragStart(e: DragEvent, item: MenuItem) {
        if(activeTab !== 'items') return;
        
        e.dataTransfer!.effectAllowed = 'move';
        // Provide data for potential other drops, though we use local state mostly
        e.dataTransfer!.setData('application/json', JSON.stringify(item));
        
        draggedItem = item;
        dragType = 'item';
    }

    function handleItemDragEnter(e: DragEvent, targetItem: MenuItem) {
        if(dragType !== 'item' || !draggedItem) return;
        if(draggedItem.id === targetItem.id) return;

        // Perform Swap (Live Sorting)
        // 1. Find source container and index
        let sourceGroupIndex = containers.findIndex(c => c.nesteds.find(i => i.id === draggedItem!.id));
        if(sourceGroupIndex === -1) return;
        
        let targetGroupIndex = containers.findIndex(c => c.nesteds.find(i => i.id === targetItem.id));
        if(targetGroupIndex === -1) return;

        const sourceList = containers[sourceGroupIndex].nesteds;
        const targetList = containers[targetGroupIndex].nesteds;

        const oldIndex = sourceList.findIndex(i => i.id === draggedItem!.id);
        const newIndex = targetList.findIndex(i => i.id === targetItem.id);
        
        // Optimistic Update
        if (sourceGroupIndex === targetGroupIndex) {
            // Same Group Swap
            const newList = [...sourceList];
            const [removed] = newList.splice(oldIndex, 1);
            newList.splice(newIndex, 0, removed);
            containers[sourceGroupIndex].nesteds = newList;
        } else {
            // Cross Group Move (Drag over item in another group)
            // Remove from source
            const [removed] = sourceList.splice(oldIndex, 1);
            // Update group_id locally
            removed.group_id = containers[targetGroupIndex].data.id;
            // Add to target
            targetList.splice(newIndex, 0, removed);
            
            // Trigger reactivity
            containers[sourceGroupIndex].nesteds = sourceList;
            containers[targetGroupIndex].nesteds = targetList;
        }
    }
    
    // Handle dropping on an empty group or specific group area
    function handleGroupDragOver(e: DragEvent) {
        // Allow dropping items into group
        if(dragType === 'item') {
            e.preventDefault(); // Necessary to allow drop
            e.dataTransfer!.dropEffect = 'move';
        }
    }
    
    function handleGroupDrop(e: DragEvent, targetGroup: MenuGroup) {
        if(dragType === 'item' && draggedItem) {
             e.preventDefault();
             // Check if item is already in this group (handled by dragEnter on items usually)
             // If group is empty, we must handle the move here because there are no items to drag enter
             
             const sourceGroupIndex = containers.findIndex(c => c.nesteds.find(i => i.id === draggedItem!.id));
             if(sourceGroupIndex === -1) return; // Should not happen
             
             const targetGroupIndex = containers.findIndex(c => c.data.id === targetGroup.id);
             
             if(sourceGroupIndex !== targetGroupIndex) {
                 const sourceList = containers[sourceGroupIndex].nesteds;
                 const targetList = containers[targetGroupIndex].nesteds;
                 const oldIndex = sourceList.findIndex(i => i.id === draggedItem!.id);
                 
                 // Remove
                 const [removed] = sourceList.splice(oldIndex, 1);
                 removed.group_id = targetGroup.id;
                 
                 // Append to end of target
                 targetList.push(removed);
                 
                 // Update state
                 containers[sourceGroupIndex].nesteds = sourceList;
                 containers[targetGroupIndex].nesteds = targetList;
             }
             
             // handleDragEnd(e); // Removed: let the source's natural dragend event handle the commit
        } else if (dragType === 'group' && draggedGroup) {
            // Group reordering drop (optional, handled via dragEnter usually)
        }
    }

    async function handleDragEnd(e: DragEvent) {
        e.preventDefault();
        
        if (dragType === 'item') {
            await commitItemReorder();
        } else if (dragType === 'group') {
            await commitGroupReorder();
        }
        
        draggedItem = null;
        draggedGroup = null;
        dragType = null;
    }
    
    async function commitItemReorder() {
        // Save current state to backend
        try {
            // Flatten all items with updated display_order
            let allUpdates: { id: string, display_order: number, group_id: string }[] = [];
            
            for(const container of containers) {
                container.nesteds.forEach((item, index) => {
                    allUpdates.push({
                        id: item.id,
                        group_id: container.data.id, 
                        display_order: index + 1
                    });
                });
            }
            
            // We need to efficiently call API
            // Current API: moveItemToGroup (one by one) and reorderMenuItems (batch order)
            // It's safer to just re-save everything or identify changes.
            // For simplicity in this demo, we assume the backend reorder API can handle minimal updates or we send all.
            // But verify: reorderMenuItems takes {id, display_order}. It DOES NOT update group_id usually.
            
            // 1. Identify Group Moves first? 
            // The user API `moveItemToGroup` moves one item.
            // The user API `reorderMenuItems` reorders.
            
            // This is complex to batch properly without a dedicated batch endpoint.
            // We'll iterate and find diffs.
            
            const promises: Promise<any>[] = [];
            
            // Check for Group changes
            // We need to compare with 'items' (original state)
            const changes = [];
            
            // To properly track, we should just send a "Reorder All" if supported,
            // but since we only have `reorderMenuItems` and `moveItemToGroup`:
            
            // Step 1: Detect Group Changes
            for(const container of containers) {
               for(const item of container.nesteds) {
                   const original = items.find(i => i.id === item.id);
                   if(original && original.group_id !== container.data.id) {
                       await moveItemToGroup(item.id, container.data.id);
                       // Update local original reference to avoid re-moving if we fail later
                       original.group_id = container.data.id; 
                   }
               }
            }
            
            // Step 2: Reorder within groups (globally unique display_order or per group?)
            // Usually reorderMenuItems updates display_order for specified IDs.
            // We just dump the new orders.
            const reorderPayload: {id: string, display_order: number}[] = [];
            for(const container of containers) {
                container.nesteds.forEach((item, idx) => {
                     reorderPayload.push({
                         id: item.id,
                         display_order: idx + 1
                     });
                });
            }
            
            if(reorderPayload.length > 0) {
                await reorderMenuItems(reorderPayload);
            }
            
            toast.success('บันทึกลำดับสำเร็จ');
            // Reload raw data to ensure sync
            // await loadData(); 
            // (Skip reload to prevent jump, assuming optimistic worked)
            
        } catch (e) {
            console.error(e);
            toast.error('บันทึกไม่สำเร็จ');
            await loadData(); // Revert
        }
    }
    
    // --- 2. MENU GROUPS REORDERING ---
    
    function handleGroupDragStart(e: DragEvent, group: MenuGroup) {
        if(activeTab !== 'groups') return;
        
        e.dataTransfer!.effectAllowed = 'move';
        draggedGroup = group;
        dragType = 'group';
    }
    
    function handleGroupDragEnter(e: DragEvent, targetGroup: MenuGroup) {
        if(dragType !== 'group' || !draggedGroup) return;
        if(draggedGroup.id === targetGroup.id) return;
        
        // Swap groups in 'containers' (view in Items tab) AND 'groups' (view in Groups tab)
        // But activeTab handles which view we are in.
        
        // Updating 'groups' array
        const oldIndex = groups.findIndex(g => g.id === draggedGroup!.id);
        const newIndex = groups.findIndex(g => g.id === targetGroup.id);
        
        if (oldIndex !== newIndex) {
            const newGroups = [...groups];
            const [removed] = newGroups.splice(oldIndex, 1);
            newGroups.splice(newIndex, 0, removed);
            groups = newGroups;
            rebuildContainers(); // Sync containers
        }
    }
    
    async function commitGroupReorder() {
        try {
            const payload = groups.map((g, i) => ({
                id: g.id,
                display_order: i + 1
            }));
            await reorderMenuGroups(payload);
            toast.success('เรียงกลุ่มสำเร็จ');
        } catch (e) {
            toast.error('เรียงกลุ่มไม่สำเร็จ');
            loadData();
        }
    }

	function openEditDialog(item: MenuItem) {
		// Placeholder
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
			<p class="text-muted-foreground mt-1">จัดการโครงสร้างเมนูและกลุ่มเมนู (Native Drag & Drop)</p>
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
			{:else if displayContainers.length === 0}
				<Card class="p-12 text-center">
					<FolderOpen class="h-16 w-16 mx-auto mb-4 opacity-20" />
					<p class="text-lg">ไม่พบกลุ่มเมนู</p>
				</Card>
			{:else}
				<div class="space-y-6 pb-20">
					{#each displayContainers as { data, nesteds } (data.id)}
						<MenuGroupContainer
							{data}
							itemCount={nesteds.length}
							onDragOver={handleGroupDragOver}
							onDrop={handleGroupDrop}
						>
							{#each nesteds as item (item.id)}
								<SortableItem
									{item}
									onEdit={openEditDialog}
									onDelete={handleDelete}
									onDragStart={handleItemDragStart}
									onDragEnter={handleItemDragEnter}
									onDragEnd={handleDragEnd}
								/>
							{:else}
								<div
									class="text-center py-8 text-muted-foreground bg-muted/20 border-2 border-dashed rounded-lg"
								>
									<p class="text-sm">ไม่มีรายการในกลุ่มนี้</p>
									<p class="text-xs mt-1">ลากเมนูมาวางที่นี่</p>
								</div>
							{/each}
						</MenuGroupContainer>
					{/each}
				</div>
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
						<div
							draggable={true}
							ondragstart={(e) => handleGroupDragStart(e, group)}
							ondragenter={(e) => handleGroupDragEnter(e, group)}
							ondragend={handleDragEnd}
							role="listitem"
							class="cursor-grab active:cursor-grabbing"
						>
							<Card class="p-4 hover:shadow-md transition-all">
								<div class="flex items-center gap-3">
									<GripVertical class="h-5 w-5 text-muted-foreground" />
									<div class="flex-1">
										<div class="flex items-center gap-2">
											<h3 class="font-semibold">{group.name}</h3>
											{#if group.name_en}
												<span class="text-sm text-muted-foreground">({group.name_en})</span>
											{/if}
										</div>
										<div class="flex items-center gap-2 mt-1">
											<code class="text-xs bg-muted px-2 py-0.5 rounded">{group.code}</code>
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
						</div>
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
