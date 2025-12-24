<script lang="ts">
	import { onMount } from 'svelte';
	import {
		listMenuGroups,
		listMenuItems,
		createMenuItem,
		updateMenuItem,
		deleteMenuItem,
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
	import { LoaderCircle, Plus, Pencil, Trash2, Menu, Eye, EyeOff } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';

	let groups: MenuGroup[] = [];
	let items: MenuItem[] = [];
	let loading = true;
	let selectedGroupId: string | null = null;

	// Dialog states
	let createDialogOpen = false;
	let editDialogOpen = false;
	let editingItem: MenuItem | null = null;

	// Form data
	let formData: Partial<CreateMenuItemRequest> = {
		code: '',
		name: '',
		name_en: '',
		path: '',
		icon: '',
		required_permission: '',
		display_order: 0
	};

	onMount(async () => {
		await loadData();
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

	// Filter items by selected group
	$: filteredItems = selectedGroupId
		? items.filter((item) => item.group_id === selectedGroupId)
		: items;

	// Group items by group
	$: itemsByGroup = items.reduce(
		(acc, item) => {
			if (!acc[item.group_id]) {
				acc[item.group_id] = [];
			}
			acc[item.group_id].push(item);
			return acc;
		},
		{} as Record<string, MenuItem[]>
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
			<p class="text-muted-foreground mt-1">จัดการโครงสร้างเมนูและการเข้าถึง</p>
		</div>
		<div class="flex gap-2">
			<Button onclick={loadData} variant="outline" disabled={loading}>
				{#if loading}
					<LoaderCircle class="mr-2 h-4 w-4 animate-spin" />
				{/if}
				รีเฟรช
			</Button>
			<Button onclick={openCreateDialog}>
				<Plus class="mr-2 h-4 w-4" />
				เพิ่มเมนู
			</Button>
		</div>
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
		<!-- Menu Items Table -->
		<Card>
			<div class="overflow-x-auto">
				<table class="w-full">
					<thead class="border-b">
						<tr class="text-left">
							<th class="p-4 font-semibold">ชื่อเมนู</th>
							<th class="p-4 font-semibold">Path</th>
							<th class="p-4 font-semibold">Module</th>
							<th class="p-4 font-semibold">Group</th>
							<th class="p-4 font-semibold">ลำดับ</th>
							<th class="p-4 font-semibold">สถานะ</th>
							<th class="p-4 font-semibold text-right">จัดการ</th>
						</tr>
					</thead>
					<tbody>
						{#each filteredItems as item (item.id)}
							<tr class="border-b hover:bg-muted/50">
								<td class="p-4">
									<div>
										<p class="font-medium">{item.name}</p>
										{#if item.name_en}
											<p class="text-sm text-muted-foreground">{item.name_en}</p>
										{/if}
									</div>
								</td>
								<td class="p-4">
									<code class="text-sm bg-muted px-2 py-1 rounded">{item.path}</code>
								</td>
								<td class="p-4">
									{#if item.required_permission}
										<Badge variant="secondary">{item.required_permission}</Badge>
									{:else}
										<span class="text-muted-foreground text-sm">-</span>
									{/if}
								</td>
								<td class="p-4">
									<span class="text-sm">
										{groups.find((g) => g.id === item.group_id)?.name || '-'}
									</span>
								</td>
								<td class="p-4">
									<span class="text-sm text-muted-foreground">{item.display_order}</span>
								</td>
								<td class="p-4">
									{#if item.is_active}
										<Badge variant="default">
											<Eye class="h-3 w-3 mr-1" />
											แสดง
										</Badge>
									{:else}
										<Badge variant="secondary">
											<EyeOff class="h-3 w-3 mr-1" />
											ซ่อน
										</Badge>
									{/if}
								</td>
								<td class="p-4">
									<div class="flex justify-end gap-2">
										<Button size="sm" variant="outline" onclick={() => openEditDialog(item)}>
											<Pencil class="h-4 w-4" />
										</Button>
										<Button size="sm" variant="destructive" onclick={() => handleDelete(item)}>
											<Trash2 class="h-4 w-4" />
										</Button>
									</div>
								</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		</Card>
	{/if}
</div>

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
