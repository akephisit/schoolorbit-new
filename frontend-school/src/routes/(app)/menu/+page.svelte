<script lang="ts">
	import {
		listMenuGroups,
		listMenuItems,
		createMenuItem,
		updateMenuItem,
		deleteMenuItem,
		reorderMenuItems,
		type MenuGroup,
		type MenuItem,
		type CreateMenuItemRequest,
		type UpdateMenuItemRequest
	} from '$lib/api/menu-admin';
	import { Button } from '$lib/components/ui/button';
	import { Card } from '$lib/components/ui/card';
	import { Badge } from '$lib/components/ui/badge';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Tabs from '$lib/components/ui/tabs';
	import { LoaderCircle, Plus, Pencil, Trash2, Menu, Eye, EyeOff, FolderOpen, AlertCircle } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';
	import SortableMenuItems from '$lib/components/menu/SortableMenuItems.svelte';
	import GroupManagementDialog from '$lib/components/menu/GroupManagementDialog.svelte';

	let groups = $state<MenuGroup[]>([]);
	let items = $state<MenuItem[]>([]);
	let loading = $state(true);
	let selectedGroupId = $state<string | null>(null);
	let activeTab = $state('items'); // 'items' or 'groups'

	// Dialog states
	let createDialogOpen = $state(false);
	let editDialogOpen = $state(false);
	let editingItem = $state<MenuItem | null>(null);
	let groupDialogOpen = $state(false);
	let editingGroup = $state<MenuGroup | null>(null);

	// Form data
	let formData = $state<Partial<CreateMenuItemRequest>>({
		code: '',
		name: '',
		name_en: '',
		path: '',
		icon: '',
		required_permission: '',
		display_order: 0
	});

	// Load data on mount
	$effect(() => {
		loadData();
	});

	async function loadData() {
		try {
			loading = true;
			[groups, items] = await Promise.all([listMenuGroups(), listMenuItems()]);
		} catch (error) {
			const message = error instanceof Error ? error.message : 'ไม่สามารถโหลดข้อมูลได้';
			toast.error(message);
		} finally {
			loading = false;
		}
	}

	async function handleCreate() {
		if (!formData.code || !formData.name || !formData.path || !formData.group_id) {
			toast.error('กรุณากรอกข้อมูลที่จำเป็น');
			return;
		}

		try {
			await createMenuItem(formData as CreateMenuItemRequest);
			toast.success('สร้างเมนูสำเร็จ');
			createDialogOpen = false;
			resetForm();
			await loadData();
		} catch (error) {
			const message = error instanceof Error ? error.message : 'ไม่สามารถสร้างเมนูได้';
			toast.error(message);
		}
	}

	async function handleUpdate() {
		if (!editingItem) return;

		try {
			const updates: UpdateMenuItemRequest = {};
			if (formData.name !== editingItem.name) updates.name = formData.name;
			if (formData.name_en !== editingItem.name_en) updates.name_en = formData.name_en;
			if (formData.path !== editingItem.path) updates.path = formData.path;
			if (formData.icon !== editingItem.icon) updates.icon = formData.icon;
			if (formData.required_permission !== editingItem.required_permission)
				updates.required_permission = formData.required_permission;
			if (formData.display_order !== editingItem.display_order)
				updates.display_order = formData.display_order;

			await updateMenuItem(editingItem.id, updates);
			toast.success('แก้ไขเมนูสำเร็จ');
			editDialogOpen = false;
			editingItem = null;
			resetForm();
			await loadData();
		} catch (error) {
			const message = error instanceof Error ? error.message : 'ไม่สามารถแก้ไขเมนูได้';
			toast.error(message);
		}
	}

	async function handleDelete(item: MenuItem) {
		if (!confirm(`ต้องการลบเมนู "${item.name}" หรือไม่?`)) return;

		try {
			await deleteMenuItem(item.id);
			toast.success('ลบเมนูสำเร็จ');
			await loadData();
		} catch (error) {
			const message = error instanceof Error ? error.message : 'ไม่สามารถลบเมนูได้';
			toast.error(message);
		}
	}

	function openCreateDialog() {
		resetForm();
		createDialogOpen = true;
	}

	function openEditDialog(item: MenuItem) {
		editingItem = item;
		formData = {
			code: item.code,
			name: item.name,
			name_en: item.name_en || '',
			path: item.path,
			icon: item.icon || '',
			required_permission: item.required_permission || '',
			display_order: item.display_order,
			group_id: item.group_id
		};
		editDialogOpen = true;
	}

	function resetForm() {
		formData = {
			code: '',
			name: '',
			name_en: '',
			path: '',
			icon: '',
			required_permission: '',
			display_order: 0
		};
	}

	async function handleReorder(groupItems: MenuItem[]) {
		try {
			const reorderData = groupItems.map((item) => ({
				id: item.id,
				display_order: item.display_order
			}));

			await reorderMenuItems(reorderData);
			toast.success('เรียงลำดับเมนูสำเร็จ');
			await loadData(); // Reload to sync
		} catch (error) {
			const message = error instanceof Error ? error.message : 'ไม่สามารถเรียงลำดับเมนูได้';
			toast.error(message);
		}
	}

	// Derived states
	const filteredItems = $derived(
		selectedGroupId ? items.filter((item) => item.group_id === selectedGroupId) : items
	);

	const itemsByGroup = $derived(
		items.reduce(
			(acc, item) => {
				if (!acc[item.group_id]) {
					acc[item.group_id] = [];
				}
				acc[item.group_id].push(item);
				return acc;
			},
			{} as Record<string, MenuItem[]>
		)
	);
</script>

<svelte:head>
	<title>จัดการเมนู - Menu Management</title>
</svelte:head>

<div class="container mx-auto p-6 space-y-6">
	<!-- Header -->
	<div class="flex items-center justify-between">
		<div>
			<h1 class="text-3xl font-bold">จัดการเมนู</h1>
			<p class="text-muted-foreground mt-1">จัดการโครงสร้างเมนูและกลุ่มเมนู</p>
		</div>
	</div>

	<!-- Tabs -->
	<Tabs.Root bind:value={activeTab}>
		<Tabs.List class="grid w-full grid-cols-2 max-w-md">
			<Tabs.Trigger value="items">รายการเมนู</Tabs.Trigger>
			<Tabs.Trigger value="groups">กลุ่มเมนู</Tabs.Trigger>
		</Tabs.List>

		<!-- ITEMS TAB -->
		<Tabs.Content value="items" class="space-y-4">
			<div class="flex justify-end">
				<Button onclick={() => (createDialogOpen = true)}>
					<Plus class="h-4 w-4 mr-2" />
					เพิ่มเมนู
				</Button>
			</div>

			<!-- Group Filter -->
			{#if groups.length > 0}
				<div class="flex gap-2">
					<Button
						variant={selectedGroupId === null ? 'default' : 'outline'}
						onclick={() => (selectedGroupId = null)}
					>
						ทั้งหมด ({items.length})
					</Button>
					{#each groups as group}
						<Button
							variant={selectedGroupId === group.id ? 'default' : 'outline'}
							onclick={() => (selectedGroupId = group.id)}
						>
							{group.name} ({itemsByGroup[group.id]?.length || 0})
						</Button>
					{/each}
				</div>
			{/if}

			<!-- Loading State -->
			{#if loading}
				<div class="flex justify-center items-center py-20">
					<LoaderCircle class="h-8 w-8 animate-spin text-primary" />
				</div>
			{:else if filteredItems.length === 0}
				<!-- Empty State -->
				<Card class="p-12">
					<div class="text-center text-muted-foreground">
						<Menu class="h-16 w-16 mx-auto mb-4 opacity-20" />
						<p class="text-lg">ไม่พบเมนูที่คุณสามารถจัดการได้</p>
						<p class="text-sm mt-2">คุณสามารถจัดการได้เฉพาะเมนูที่มีสิทธิ์ในโมดูลนั้นๆ</p>
					</div>
				</Card>
			{:else}
				<!-- Menu Items Sortable Lists by Group -->
				{#if selectedGroupId}
					<!-- Single group view -->
					{@const group = groups.find((g) => g.id === selectedGroupId)}
					{#if group}
						<SortableMenuItems
							items={filteredItems}
							groupName={group.name}
							onReorder={handleReorder}
							onEdit={openEditDialog}
							onDelete={handleDelete}
						/>
					{/if}
				{:else}
					<!-- All groups view -->
					<div class="space-y-6">
						{#each groups as group (group.id)}
							{@const groupItems = itemsByGroup[group.id] || []}
							{#if groupItems.length > 0}
								<SortableMenuItems
									items={groupItems}
									groupName={group.name}
									onReorder={handleReorder}
									onEdit={openEditDialog}
									onDelete={handleDelete}
								/>
							{/if}
						{/each}
					</div>
				{/if}
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
					<Plus class="h-4 w-4 mr-2" />
					สร้างกลุ่มใหม่
				</Button>
			</div>

			{#if loading}
				<div class="flex justify-center py-12">
					<LoaderCircle class="h-8 w-8 animate-spin" />
				</div>
			{:else if groups.length === 0}
				<Card class="p-12 text-center">
					<FolderOpen class="h-16 w-16 mx-auto mb-4 opacity-20" />
					<p class="text-lg">ไม่พบกลุ่มเมนู</p>
				</Card>
			{:else}
				<div class="grid gap-3">
					{#each groups as group (group.id)}
						{@const itemCount = itemsByGroup[group.id]?.length || 0}
						<Card class="p-4">
							<div class="flex items-center gap-3">
								<div class="flex-1">
									<div class="flex items-center gap-2">
										<h3 class="font-semibold">{group.name}</h3>
										{#if group.name_en}
											<span class="text-sm text-muted-foreground">({group.name_en})</span>
										{/if}
										{#if group.code === 'other' && itemCount > 0}
											<Badge variant="destructive" class="gap-1">
												<AlertCircle class="h-3 w-3" />
												ต้องจัดกลุ่ม
											</Badge>
										{/if}
									</div>
									<div class="flex items-center gap-2 mt-1">
										<code class="text-xs bg-muted px-2 py-0.5 rounded">{group.code}</code>
										<span class="text-sm text-muted-foreground">{itemCount} รายการ</span>
									</div>
								</div>
								<div class="flex gap-2">
									<Button
										size="sm"
										variant="outline"
										onclick={() => {
											editingGroup = group;
											groupDialogOpen = true;
										}}
									>
										<Pencil class="h-4 w-4" />
									</Button>
								</div>
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

<!-- Create Dialog -->

<Dialog.Root bind:open={createDialogOpen}>
	<Dialog.Content class="max-w-md">
		<Dialog.Header>
			<Dialog.Title>เพิ่มเมนูใหม่</Dialog.Title>
			<Dialog.Description>สร้างเมนูใหม่ในระบบ</Dialog.Description>
		</Dialog.Header>
		<div class="space-y-4">
			<div>
				<Label for="code">รหัสเมนู *</Label>
				<Input id="code" bind:value={formData.code} placeholder="staff" />
			</div>
			<div>
				<Label for="name">ชื่อเมนู (ไทย) *</Label>
				<Input id="name" bind:value={formData.name} placeholder="บุคลากร" />
			</div>
			<div>
				<Label for="name_en">ชื่อเมนู (อังกฤษ)</Label>
				<Input id="name_en" bind:value={formData.name_en} placeholder="Staff" />
			</div>
			<div>
				<Label for="path">Path *</Label>
				<Input id="path" bind:value={formData.path} placeholder="/staff" />
			</div>
			<div>
				<Label for="icon">ไอคอน</Label>
				<Input id="icon" bind:value={formData.icon} placeholder="Users" />
			</div>
			<div>
				<Label for="module">Module (required_permission)</Label>
				<Input id="module" bind:value={formData.required_permission} placeholder="staff" />
			</div>
			<div>
				<Label for="group">กลุ่มเมนู * (Group ID)</Label>
				<Input id="group" bind:value={formData.group_id} placeholder="กรอก Group ID" />
				<div class="mt-2 max-h-32 overflow-y-auto">
					{#each groups as group}
						<button
							type="button"
							class="block w-full text-left text-sm px-2 py-1 hover:bg-muted rounded"
							onclick={() => (formData.group_id = group.id)}
						>
							{group.name}
						</button>
					{/each}
				</div>
			</div>
			<div>
				<Label for="order">ลำดับการแสดง</Label>
				<Input id="order" type="number" bind:value={formData.display_order} />
			</div>
		</div>
		<Dialog.Footer>
			<Button variant="outline" onclick={() => (createDialogOpen = false)}>ยกเลิก</Button>
			<Button onclick={handleCreate}>สร้าง</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<!-- Edit Dialog -->
<Dialog.Root bind:open={editDialogOpen}>
	<Dialog.Content class="max-w-md">
		<Dialog.Header>
			<Dialog.Title>แก้ไขเมนู</Dialog.Title>
			<Dialog.Description>แก้ไขข้อมูลเมนู</Dialog.Description>
		</Dialog.Header>
		<div class="space-y-4">
			<div>
				<Label for="edit-name">ชื่อเมนู (ไทย) *</Label>
				<Input id="edit-name" bind:value={formData.name} />
			</div>
			<div>
				<Label for="edit-name_en">ชื่อเมนู (อังกฤษ)</Label>
				<Input id="edit-name_en" bind:value={formData.name_en} />
			</div>
			<div>
				<Label for="edit-path">Path *</Label>
				<Input id="edit-path" bind:value={formData.path} />
			</div>
			<div>
				<Label for="edit-icon">ไอคอน</Label>
				<Input id="edit-icon" bind:value={formData.icon} />
			</div>
			<div>
				<Label for="edit-module">Module</Label>
				<Input id="edit-module" bind:value={formData.required_permission} />
			</div>
			<div>
				<Label for="edit-order">ลำดับการแสดง</Label>
				<Input id="edit-order" type="number" bind:value={formData.display_order} />
			</div>
		</div>
		<Dialog.Footer>
			<Button variant="outline" onclick={() => (editDialogOpen = false)}>ยกเลิก</Button>
			<Button onclick={handleUpdate}>บันทึก</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
