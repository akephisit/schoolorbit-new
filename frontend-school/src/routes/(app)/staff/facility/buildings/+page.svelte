<script lang="ts">
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';

	let { data } = $props();

	import {
		type Building,
		type Room,
		listBuildings,
		listRooms,
		createBuilding,
		updateBuilding,
		deleteBuilding,
		createRoom,
		updateRoom,
		deleteRoom
	} from '$lib/api/facility';

	import * as Card from '$lib/components/ui/card';
	import * as Table from '$lib/components/ui/table';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Badge } from '$lib/components/ui/badge';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Select from '$lib/components/ui/select';
	import * as Tabs from '$lib/components/ui/tabs';

	import {
		Building as BuildingIcon,
		DoorOpen,
		Plus,
		Search,
		Settings,
		Trash2,
		Loader2,
		School
	} from 'lucide-svelte';

	// Constants
	const ROOM_TYPES = [
		{ value: 'GENERAL', label: 'ห้องเรียนทั่วไป' },
		{ value: 'LAB', label: 'ห้องปฏิบัติการ' },
		{ value: 'AUDITORIUM', label: 'หอประชุม' },
		{ value: 'GYM', label: 'โรงยิม' },
		{ value: 'LIBRARY', label: 'ห้องสมุด' },
		{ value: 'OFFICE', label: 'สำนักงาน' },
		{ value: 'OTHER', label: 'อื่นๆ' }
	];

	// State
	let loading = $state(true);
	let buildings = $state<Building[]>([]);
	let rooms = $state<Room[]>([]);
	let activeTab = $state('buildings');

	// Filters
	let searchTerm = $state('');
	let selectedBuildingFilter = $state('all');

	// Dialogs
	let showBuildingDialog = $state(false);
	let showRoomDialog = $state(false);
	let showDeleteDialog = $state(false);
	let submitting = $state(false);

	// Editing
	let editingItem = $state<any>(null); // Building or Room
	let deleteTarget = $state<{ type: 'building' | 'room'; id: string; name: string } | null>(null);

	// Initial Data
	async function loadData() {
		try {
			loading = true;
			const [bRes, rRes] = await Promise.all([
				listBuildings(),
				listRooms({
					building_id: selectedBuildingFilter === 'all' ? undefined : selectedBuildingFilter
				}) // Preload active rooms
			]);
			buildings = bRes.data;
			rooms = rRes.data;
		} catch (e) {
			toast.error('โหลดข้อมูลไม่สำเร็จ');
		} finally {
			loading = false;
		}
	}

	async function refreshRooms() {
		try {
			const res = await listRooms({
				building_id: selectedBuildingFilter === 'all' ? undefined : selectedBuildingFilter,
				search: searchTerm || undefined
			});
			rooms = res.data;
		} catch (e) {
			console.error(e);
		}
	}

	// Actions: Buildings
	async function handleSaveBuilding(e: SubmitEvent) {
		e.preventDefault();
		const form = e.target as HTMLFormElement;
		const formData = new FormData(form);
		const payload = {
			name_th: formData.get('name_th') as string,
			name_en: formData.get('name_en') as string,
			code: formData.get('code') as string,
			description: formData.get('description') as string
		};

		submitting = true;
		try {
			if (editingItem) {
				await updateBuilding(editingItem.id, payload);
				toast.success('บันทึกข้อมูลอาคารสำเร็จ');
			} else {
				await createBuilding(payload);
				toast.success('เพิ่มอาคารสำเร็จ');
			}
			showBuildingDialog = false;
			loadData();
		} catch (e) {
			toast.error('บันทึกไม่สำเร็จ');
		} finally {
			submitting = false;
		}
	}

	// Actions: Rooms
	async function handleSaveRoom(e: SubmitEvent) {
		e.preventDefault();
		const form = e.target as HTMLFormElement;
		const formData = new FormData(form);

		const payload = {
			name_th: formData.get('name_th') as string,
			name_en: formData.get('name_en') as string,
			code: formData.get('code') as string,
			room_type: formData.get('room_type') as string,
			building_id: (formData.get('building_id') as string) || undefined,
			capacity: parseInt(formData.get('capacity') as string) || 40,
			floor: parseInt(formData.get('floor') as string) || undefined,
			description: formData.get('description') as string
		};

		submitting = true;
		try {
			if (editingItem) {
				await updateRoom(editingItem.id, payload);
				toast.success('บันทึกข้อมูลห้องสำเร็จ');
			} else {
				await createRoom(payload);
				toast.success('เพิ่มห้องสำเร็จ');
			}
			showRoomDialog = false;
			refreshRooms();
		} catch (e) {
			toast.error('บันทึกไม่สำเร็จ');
		} finally {
			submitting = false;
		}
	}

	async function handleDelete() {
		if (!deleteTarget) return;
		submitting = true;
		try {
			if (deleteTarget.type === 'building') {
				await deleteBuilding(deleteTarget.id);
				toast.success('ลบอาคารสำเร็จ');
				loadData();
			} else {
				await deleteRoom(deleteTarget.id);
				toast.success('ลบห้องสำเร็จ');
				refreshRooms();
			}
			showDeleteDialog = false;
		} catch (e) {
			toast.error('ลบไม่สำเร็จ (อาจมีข้อมูลเชื่อมโยง)');
		} finally {
			submitting = false;
		}
	}

	// Helpers
	function openAddBuilding() {
		editingItem = null;
		showBuildingDialog = true;
	}
	function openEditBuilding(b: Building) {
		editingItem = b;
		showBuildingDialog = true;
	}
	function openAddRoom() {
		editingItem = null;
		showRoomDialog = true;
	}
	function openEditRoom(r: Room) {
		editingItem = r;
		showRoomDialog = true;
	}
	function confirmDelete(type: 'building' | 'room', item: any) {
		deleteTarget = {
			type,
			id: item.id,
			name: item.name_th
		};
		showDeleteDialog = true;
	}

	// Fix for select inputs logic
	let formBuildingId = $state('');
	let formRoomType = $state('GENERAL');

	$effect(() => {
		if (showRoomDialog) {
			if (editingItem) {
				formBuildingId = editingItem.building_id || '';
				formRoomType = editingItem.room_type || 'GENERAL';
			} else {
				formBuildingId = (selectedBuildingFilter !== 'all' ? selectedBuildingFilter : '') || '';
				formRoomType = 'GENERAL';
			}
		}
	});

	onMount(loadData);
</script>

<svelte:head>
	<title>{data.title} - SchoolOrbit</title>
</svelte:head>

<div class="space-y-6">
	<div class="flex flex-col gap-2">
		<h2 class="text-3xl font-bold flex items-center gap-2">
			<School class="w-8 h-8" />
			จัดการอาคารสถานที่
		</h2>
		<p class="text-muted-foreground">
			ข้อมูลอาคาร (Buildings) และห้องเรียน (Rooms) สำหรับใช้งานในระบบ
		</p>
	</div>

	<Tabs.Root value={activeTab} onValueChange={(v) => (activeTab = v)}>
		<Tabs.List>
			<Tabs.Trigger value="buildings" class="flex gap-2">
				<BuildingIcon class="w-4 h-4" /> อาคาร
			</Tabs.Trigger>
			<Tabs.Trigger value="rooms" class="flex gap-2">
				<DoorOpen class="w-4 h-4" /> ห้องเรียน/ห้องปฏิบัติการ
			</Tabs.Trigger>
		</Tabs.List>

		<!-- Buildings Tab -->
		<Tabs.Content value="buildings" class="space-y-4 pt-4">
			<div class="flex justify-end">
				<Button onclick={openAddBuilding}>
					<Plus class="w-4 h-4 mr-2" /> เพิ่มอาคาร
				</Button>
			</div>

			<Card.Root>
				<Table.Root>
					<Table.Header>
						<Table.Row>
							<Table.Head class="w-[100px]">รหัส</Table.Head>
							<Table.Head>ชื่ออาคาร</Table.Head>
							<Table.Head>รายละเอียด</Table.Head>
							<Table.Head class="text-right">จัดการ</Table.Head>
						</Table.Row>
					</Table.Header>
					<Table.Body>
						{#if loading}
							<Table.Row
								><Table.Cell colspan={4} class="h-24 text-center"
									><Loader2 class="animate-spin mx-auto" /></Table.Cell
								></Table.Row
							>
						{:else if buildings.length === 0}
							<Table.Row
								><Table.Cell colspan={4} class="h-24 text-center text-muted-foreground"
									>ไม่พบข้อมูล</Table.Cell
								></Table.Row
							>
						{:else}
							{#each buildings as b}
								<Table.Row>
									<Table.Cell class="font-mono text-xs">{b.code || '-'}</Table.Cell>
									<Table.Cell>
										<div class="font-medium">{b.name_th}</div>
										{#if b.name_en}<div class="text-xs text-muted-foreground">{b.name_en}</div>{/if}
									</Table.Cell>
									<Table.Cell class="text-muted-foreground text-sm"
										>{b.description || '-'}</Table.Cell
									>
									<Table.Cell class="text-right">
										<Button variant="ghost" size="icon" onclick={() => openEditBuilding(b)}>
											<Settings class="w-4 h-4" />
										</Button>
										<Button
											variant="ghost"
											size="icon"
											class="text-destructive"
											onclick={() => confirmDelete('building', b)}
										>
											<Trash2 class="w-4 h-4" />
										</Button>
									</Table.Cell>
								</Table.Row>
							{/each}
						{/if}
					</Table.Body>
				</Table.Root>
			</Card.Root>
		</Tabs.Content>

		<!-- Rooms Tab -->
		<Tabs.Content value="rooms" class="space-y-4 pt-4">
			<div class="flex gap-4 items-center flex-wrap">
				<div class="w-[200px]">
					<Select.Root
						type="single"
						bind:value={selectedBuildingFilter}
						onValueChange={refreshRooms}
					>
						<Select.Trigger class="w-full">
							{selectedBuildingFilter === 'all'
								? 'ทุกอาคาร'
								: buildings.find((b) => b.id === selectedBuildingFilter)?.name_th || 'เลือกอาคาร'}
						</Select.Trigger>
						<Select.Content>
							<Select.Item value="all">ทุกอาคาร</Select.Item>
							{#each buildings as b}
								<Select.Item value={b.id}>{b.name_th}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
				<div class="relative w-[300px]">
					<Search class="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
					<Input
						placeholder="ค้นหาชื่อห้อง/รหัส..."
						class="pl-8"
						bind:value={searchTerm}
						oninput={refreshRooms}
					/>
				</div>
				<div class="ml-auto">
					<Button onclick={openAddRoom}>
						<Plus class="w-4 h-4 mr-2" /> เพิ่มห้อง
					</Button>
				</div>
			</div>

			<Card.Root>
				<Table.Root>
					<Table.Header>
						<Table.Row>
							<Table.Head class="w-[100px]">รหัสห้อง</Table.Head>
							<Table.Head>ชื่อห้อง</Table.Head>
							<Table.Head>ประเภท</Table.Head>
							<Table.Head>อาคาร/ชั้น</Table.Head>
							<Table.Head class="text-center">ความจุ</Table.Head>
							<Table.Head class="text-right">จัดการ</Table.Head>
						</Table.Row>
					</Table.Header>
					<Table.Body>
						{#if loading && rooms.length === 0}
							<Table.Row
								><Table.Cell colspan={6} class="h-24 text-center"
									><Loader2 class="animate-spin mx-auto" /></Table.Cell
								></Table.Row
							>
						{:else if rooms.length === 0}
							<Table.Row
								><Table.Cell colspan={6} class="h-24 text-center text-muted-foreground"
									>ไม่พบข้อมูล</Table.Cell
								></Table.Row
							>
						{:else}
							{#each rooms as r}
								<Table.Row>
									<Table.Cell class="font-bold">{r.code || '-'}</Table.Cell>
									<Table.Cell>
										<div class="font-medium">{r.name_th}</div>
										{#if r.name_en}<div class="text-xs text-muted-foreground">{r.name_en}</div>{/if}
									</Table.Cell>
									<Table.Cell>
										<Badge variant="outline">
											{ROOM_TYPES.find((t) => t.value === r.room_type)?.label || r.room_type}
										</Badge>
									</Table.Cell>
									<Table.Cell>
										<div class="text-sm">{r.building_name || '-'}</div>
										{#if r.floor}<div class="text-xs text-muted-foreground">
												ชั้น {r.floor}
											</div>{/if}
									</Table.Cell>
									<Table.Cell class="text-center">{r.capacity}</Table.Cell>
									<Table.Cell class="text-right">
										<Button variant="ghost" size="icon" onclick={() => openEditRoom(r)}>
											<Settings class="w-4 h-4" />
										</Button>
										<Button
											variant="ghost"
											size="icon"
											class="text-destructive"
											onclick={() => confirmDelete('room', r)}
										>
											<Trash2 class="w-4 h-4" />
										</Button>
									</Table.Cell>
								</Table.Row>
							{/each}
						{/if}
					</Table.Body>
				</Table.Root>
			</Card.Root>
		</Tabs.Content>
	</Tabs.Root>

	<!-- Building Dialog -->
	<Dialog.Root bind:open={showBuildingDialog}>
		<Dialog.Content>
			<Dialog.Header>
				<Dialog.Title>{editingItem ? 'แก้ไขอาคาร' : 'เพิ่มอาคารใหม่'}</Dialog.Title>
			</Dialog.Header>
			<form onsubmit={handleSaveBuilding} class="space-y-4 py-4">
				<div class="grid grid-cols-2 gap-4">
					<div class="space-y-2">
						<Label>รหัสอาคาร (Code)</Label>
						<Input name="code" value={editingItem?.code || ''} placeholder="เช่น BLD1" />
					</div>
				</div>
				<div class="space-y-2">
					<Label>ชื่ออาคาร (TH) <span class="text-red-500">*</span></Label>
					<Input
						name="name_th"
						value={editingItem?.name_th || ''}
						required
						placeholder="เช่น อาคารเฉลิมพระเกียรติ"
					/>
				</div>
				<div class="space-y-2">
					<Label>ชื่ออาคาร (EN)</Label>
					<Input name="name_en" value={editingItem?.name_en || ''} placeholder="Ex. Building 1" />
				</div>
				<div class="space-y-2">
					<Label>รายละเอียด</Label>
					<Input name="description" value={editingItem?.description || ''} />
				</div>

				<Dialog.Footer>
					<Button variant="outline" type="button" onclick={() => (showBuildingDialog = false)}
						>ยกเลิก</Button
					>
					<Button type="submit" disabled={submitting}>บันทึก</Button>
				</Dialog.Footer>
			</form>
		</Dialog.Content>
	</Dialog.Root>

	<!-- Room Dialog -->
	<Dialog.Root bind:open={showRoomDialog}>
		<Dialog.Content class="sm:max-w-[600px]">
			<Dialog.Header>
				<Dialog.Title>{editingItem ? 'แก้ไขห้อง' : 'เพิ่มห้องใหม่'}</Dialog.Title>
			</Dialog.Header>
			<form onsubmit={handleSaveRoom} class="space-y-4 py-4">
				<div class="grid grid-cols-2 gap-4">
					<div class="space-y-2">
						<Label>อาคาร <span class="text-red-500">*</span></Label>
						<Select.Root type="single" bind:value={formBuildingId}>
							<Select.Trigger class="w-full">
								{buildings.find((b) => b.id === formBuildingId)?.name_th || 'เลือกอาคาร'}
							</Select.Trigger>
							<Select.Content>
								{#each buildings as b}
									<Select.Item value={b.id}>{b.name_th}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
						<input type="hidden" name="building_id" value={formBuildingId} />
					</div>
					<div class="space-y-2">
						<Label>ประเภทห้อง</Label>
						<Select.Root type="single" bind:value={formRoomType}>
							<Select.Trigger class="w-full">
								{ROOM_TYPES.find((t) => t.value === formRoomType)?.label}
							</Select.Trigger>
							<Select.Content>
								{#each ROOM_TYPES as t}
									<Select.Item value={t.value}>{t.label}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
						<input type="hidden" name="room_type" value={formRoomType} />
					</div>
				</div>

				<div class="grid grid-cols-3 gap-4">
					<div class="col-span-1 space-y-2">
						<Label>รหัส/เลขห้อง <span class="text-red-500">*</span></Label>
						<Input name="code" value={editingItem?.code || ''} required placeholder="301" />
					</div>
					<div class="col-span-2 space-y-2">
						<Label>ชื่อห้อง (TH) <span class="text-red-500">*</span></Label>
						<Input
							name="name_th"
							value={editingItem?.name_th || ''}
							required
							placeholder="ห้องเรียน 301"
						/>
					</div>
				</div>

				<div class="grid grid-cols-2 gap-4">
					<div class="space-y-2">
						<Label>ชั้น</Label>
						<Input type="number" name="floor" value={editingItem?.floor?.toString() || ''} />
					</div>
					<div class="space-y-2">
						<Label>ความจุ (คน)</Label>
						<Input
							type="number"
							name="capacity"
							value={editingItem?.capacity?.toString() || '40'}
						/>
					</div>
				</div>

				<div class="space-y-2">
					<Label>รายละเอียดเพิ่มเติม</Label>
					<Input name="description" value={editingItem?.description || ''} />
				</div>

				<Dialog.Footer>
					<Button variant="outline" type="button" onclick={() => (showRoomDialog = false)}
						>ยกเลิก</Button
					>
					<Button type="submit" disabled={submitting}>บันทึก</Button>
				</Dialog.Footer>
			</form>
		</Dialog.Content>
	</Dialog.Root>

	<!-- Delete Confirm -->
	<Dialog.Root bind:open={showDeleteDialog}>
		<Dialog.Content>
			<Dialog.Header>
				<Dialog.Title>ยืนยันการลบ</Dialog.Title>
				<Dialog.Description>
					คุณต้องการลบ "{deleteTarget?.name}" ใช่หรือไม่? การกระทำนี้ไม่สามารถเรียกคืนได้
				</Dialog.Description>
			</Dialog.Header>
			<Dialog.Footer>
				<Button variant="outline" onclick={() => (showDeleteDialog = false)}>ยกเลิก</Button>
				<Button variant="destructive" onclick={handleDelete} disabled={submitting}>ยืนยันลบ</Button>
			</Dialog.Footer>
		</Dialog.Content>
	</Dialog.Root>
</div>
