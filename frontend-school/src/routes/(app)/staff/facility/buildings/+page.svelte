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
	import { PageShell } from '$lib/components/app-layout';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Badge } from '$lib/components/ui/badge';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Select from '$lib/components/ui/select';
	import * as Tabs from '$lib/components/ui/tabs';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';

	import {
		Building as BuildingIcon,
		DoorOpen,
		Plus,
		Search,
		Settings,
		Trash2
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
	const canReadFacility = $derived($can.has(PERMISSIONS.FACILITY_READ_ALL));
	const canCreateFacility = $derived($can.has(PERMISSIONS.FACILITY_CREATE_ALL));
	const canUpdateFacility = $derived($can.has(PERMISSIONS.FACILITY_UPDATE_ALL));
	const canDeleteFacility = $derived($can.has(PERMISSIONS.FACILITY_DELETE_ALL));
	const canMutateFacility = $derived(canUpdateFacility || canDeleteFacility);

	// Filters
	let searchTerm = $state('');
	let selectedBuildingFilter = $state('all');

	// Dialogs
	let showBuildingDialog = $state(false);
	let showRoomDialog = $state(false);
	let showDeleteDialog = $state(false);
	let submitting = $state(false);

	// Editing
	let editingItem = $state<Building | Room | null>(null);
	let deleteTarget = $state<{ type: 'building' | 'room'; id: string; name: string } | null>(null);

	// Initial Data
	async function loadData() {
		if (!canReadFacility) {
			buildings = [];
			rooms = [];
			loading = false;
			return;
		}
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
		} catch {
			toast.error('โหลดข้อมูลไม่สำเร็จ');
		} finally {
			loading = false;
		}
	}

	async function refreshRooms() {
		if (!canReadFacility) return;
		try {
			const res = await listRooms({
				building_id: selectedBuildingFilter === 'all' ? undefined : selectedBuildingFilter,
				search: searchTerm || undefined
			});
			rooms = res.data;
		} catch {
			toast.error('โหลดข้อมูลห้องไม่สำเร็จ');
		}
	}

	function replaceBuilding(building: Building) {
		buildings = buildings.some((item) => item.id === building.id)
			? buildings.map((item) => (item.id === building.id ? building : item))
			: [...buildings, building];
		buildings = buildings.sort((a, b) =>
			(a.code ?? a.name_th).localeCompare(b.code ?? b.name_th, 'th')
		);
		rooms = rooms.map((room) =>
			room.building_id === building.id ? { ...room, building_name: building.name_th } : room
		);
	}

	function removeBuilding(id: string) {
		buildings = buildings.filter((building) => building.id !== id);
		rooms = rooms.filter((room) => room.building_id !== id);
	}

	function roomMatchesCurrentFilters(room: Room) {
		if (selectedBuildingFilter !== 'all' && room.building_id !== selectedBuildingFilter)
			return false;
		const query = searchTerm.trim().toLowerCase();
		if (!query) return true;
		return [room.name_th, room.name_en, room.code, room.building_name]
			.filter(Boolean)
			.some((value) => value!.toLowerCase().includes(query));
	}

	function replaceRoom(room: Room) {
		if (!roomMatchesCurrentFilters(room)) {
			removeRoom(room.id);
			return;
		}

		rooms = rooms.some((item) => item.id === room.id)
			? rooms.map((item) => (item.id === room.id ? room : item))
			: [...rooms, room];
		rooms = rooms.sort((a, b) => (a.code ?? a.name_th).localeCompare(b.code ?? b.name_th, 'th'));
	}

	function removeRoom(id: string) {
		rooms = rooms.filter((room) => room.id !== id);
	}

	// Actions: Buildings
	async function handleSaveBuilding(e: SubmitEvent) {
		e.preventDefault();
		if ((editingItem && !canUpdateFacility) || (!editingItem && !canCreateFacility)) return;
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
				replaceBuilding((await updateBuilding(editingItem.id, payload)).data);
				toast.success('บันทึกข้อมูลอาคารสำเร็จ');
			} else {
				replaceBuilding((await createBuilding(payload)).data);
				toast.success('เพิ่มอาคารสำเร็จ');
			}
			showBuildingDialog = false;
		} catch {
			toast.error('บันทึกไม่สำเร็จ');
		} finally {
			submitting = false;
		}
	}

	// Actions: Rooms
	async function handleSaveRoom(e: SubmitEvent) {
		e.preventDefault();
		if ((editingItem && !canUpdateFacility) || (!editingItem && !canCreateFacility)) return;
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
				replaceRoom((await updateRoom(editingItem.id, payload)).data);
				toast.success('บันทึกข้อมูลห้องสำเร็จ');
			} else {
				replaceRoom((await createRoom(payload)).data);
				toast.success('เพิ่มห้องสำเร็จ');
			}
			showRoomDialog = false;
		} catch {
			toast.error('บันทึกไม่สำเร็จ');
		} finally {
			submitting = false;
		}
	}

	async function handleDelete() {
		if (!deleteTarget || !canDeleteFacility) return;
		submitting = true;
		try {
			if (deleteTarget.type === 'building') {
				await deleteBuilding(deleteTarget.id);
				removeBuilding(deleteTarget.id);
				toast.success('ลบอาคารสำเร็จ');
			} else {
				await deleteRoom(deleteTarget.id);
				removeRoom(deleteTarget.id);
				toast.success('ลบห้องสำเร็จ');
			}
			showDeleteDialog = false;
		} catch {
			toast.error('ลบไม่สำเร็จ (อาจมีข้อมูลเชื่อมโยง)');
		} finally {
			submitting = false;
		}
	}

	// Helpers
	function openAddBuilding() {
		if (!canCreateFacility) return;
		editingItem = null;
		showBuildingDialog = true;
	}
	function openEditBuilding(b: Building) {
		if (!canUpdateFacility) return;
		editingItem = b;
		showBuildingDialog = true;
	}
	function openAddRoom() {
		if (!canCreateFacility) return;
		editingItem = null;
		showRoomDialog = true;
	}
	function openEditRoom(r: Room) {
		if (!canUpdateFacility) return;
		editingItem = r;
		showRoomDialog = true;
	}
	function confirmDelete(type: 'building' | 'room', item: Building | Room) {
		if (!canDeleteFacility) return;
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
				const room = editingItem as Room;
				formBuildingId = room.building_id || '';
				formRoomType = room.room_type || 'GENERAL';
			} else {
				formBuildingId = (selectedBuildingFilter !== 'all' ? selectedBuildingFilter : '') || '';
				formRoomType = 'GENERAL';
			}
		}
	});

	// Narrowed Room reference for use in Room dialog template
	let editingRoom = $derived(
		editingItem && 'room_type' in editingItem ? (editingItem as Room) : null
	);

	onMount(loadData);
</script>

<svelte:head>
	<title>{data.title} - SchoolOrbit</title>
</svelte:head>

<PageShell
	title="จัดการอาคารสถานที่"
	description="ข้อมูลอาคาร (Buildings) และห้องเรียน (Rooms) สำหรับใช้งานในระบบ"
>
	{#if !canReadFacility}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ดูอาคารสถานที่"
			description="หน้านี้ต้องใช้สิทธิ์อ่านข้อมูลอาคารสถานที่ก่อนจึงจะแสดงรายการอาคารและห้องได้"
		/>
	{:else}
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
				{#if canCreateFacility}
					<div class="flex justify-end">
						<Button onclick={openAddBuilding}>
							<Plus class="w-4 h-4 mr-2" /> เพิ่มอาคาร
						</Button>
					</div>
				{/if}

				<Card.Root>
					<Table.Root>
						<Table.Header>
							<Table.Row>
								<Table.Head class="w-[100px]">รหัส</Table.Head>
								<Table.Head>ชื่ออาคาร</Table.Head>
								<Table.Head>รายละเอียด</Table.Head>
								{#if canMutateFacility}
									<Table.Head class="text-right">จัดการ</Table.Head>
								{/if}
							</Table.Row>
						</Table.Header>
						<Table.Body>
							{#if loading}
								<Table.Row>
									<Table.Cell colspan={canMutateFacility ? 4 : 3} class="p-0">
										<PageSkeleton variant="table" rows={4} columns={canMutateFacility ? 4 : 3} />
									</Table.Cell>
								</Table.Row>
							{:else if buildings.length === 0}
								<Table.Row>
									<Table.Cell colspan={canMutateFacility ? 4 : 3} class="h-24">
										<PageState
											title="ไม่พบข้อมูลอาคาร"
											description="เพิ่มอาคารเพื่อใช้จัดกลุ่มห้องเรียนและห้องปฏิบัติการ"
											actionLabel={canCreateFacility ? 'เพิ่มอาคาร' : undefined}
											onaction={openAddBuilding}
										/>
									</Table.Cell>
								</Table.Row>
							{:else}
								{#each buildings as b (b.id)}
									<Table.Row>
										<Table.Cell class="font-mono text-xs">{b.code || '-'}</Table.Cell>
										<Table.Cell>
											<div class="font-medium">{b.name_th}</div>
											{#if b.name_en}<div class="text-xs text-muted-foreground">
													{b.name_en}
												</div>{/if}
										</Table.Cell>
										<Table.Cell class="text-muted-foreground text-sm"
											>{b.description || '-'}</Table.Cell
										>
										{#if canMutateFacility}
											<Table.Cell class="text-right">
												{#if canUpdateFacility}
													<Button variant="ghost" size="icon" onclick={() => openEditBuilding(b)}>
														<Settings class="w-4 h-4" />
													</Button>
												{/if}
												{#if canDeleteFacility}
													<Button
														variant="ghost"
														size="icon"
														class="text-destructive"
														onclick={() => confirmDelete('building', b)}
													>
														<Trash2 class="w-4 h-4" />
													</Button>
												{/if}
											</Table.Cell>
										{/if}
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
								{#each buildings as b (b.id)}
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
					{#if canCreateFacility}
						<div class="ml-auto">
							<Button onclick={openAddRoom}>
								<Plus class="w-4 h-4 mr-2" /> เพิ่มห้อง
							</Button>
						</div>
					{/if}
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
								{#if canMutateFacility}
									<Table.Head class="text-right">จัดการ</Table.Head>
								{/if}
							</Table.Row>
						</Table.Header>
						<Table.Body>
							{#if loading && rooms.length === 0}
								<Table.Row>
									<Table.Cell colspan={canMutateFacility ? 6 : 5} class="p-0">
										<PageSkeleton variant="table" rows={4} columns={canMutateFacility ? 6 : 5} />
									</Table.Cell>
								</Table.Row>
							{:else if rooms.length === 0}
								<Table.Row>
									<Table.Cell colspan={canMutateFacility ? 6 : 5} class="h-24">
										<PageState
											title="ไม่พบข้อมูลห้อง"
											description="เพิ่มห้องเรียนหรือห้องปฏิบัติการเพื่อใช้กับตารางสอนและงานอาคาร"
											actionLabel={canCreateFacility ? 'เพิ่มห้อง' : undefined}
											onaction={openAddRoom}
										/>
									</Table.Cell>
								</Table.Row>
							{:else}
								{#each rooms as r (r.id)}
									<Table.Row>
										<Table.Cell class="font-bold">{r.code || '-'}</Table.Cell>
										<Table.Cell>
											<div class="font-medium">{r.name_th}</div>
											{#if r.name_en}<div class="text-xs text-muted-foreground">
													{r.name_en}
												</div>{/if}
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
										{#if canMutateFacility}
											<Table.Cell class="text-right">
												{#if canUpdateFacility}
													<Button variant="ghost" size="icon" onclick={() => openEditRoom(r)}>
														<Settings class="w-4 h-4" />
													</Button>
												{/if}
												{#if canDeleteFacility}
													<Button
														variant="ghost"
														size="icon"
														class="text-destructive"
														onclick={() => confirmDelete('room', r)}
													>
														<Trash2 class="w-4 h-4" />
													</Button>
												{/if}
											</Table.Cell>
										{/if}
									</Table.Row>
								{/each}
							{/if}
						</Table.Body>
					</Table.Root>
				</Card.Root>
			</Tabs.Content>
		</Tabs.Root>

		<!-- Building Dialog -->
		{#if canCreateFacility || canUpdateFacility}
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
							<Input
								name="name_en"
								value={editingItem?.name_en || ''}
								placeholder="Ex. Building 1"
							/>
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
		{/if}

		<!-- Room Dialog -->
		{#if canCreateFacility || canUpdateFacility}
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
										{#each buildings as b (b.id)}
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
										{#each ROOM_TYPES as t (t.value)}
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
								<Input type="number" name="floor" value={editingRoom?.floor?.toString() || ''} />
							</div>
							<div class="space-y-2">
								<Label>ความจุ (คน)</Label>
								<Input
									type="number"
									name="capacity"
									value={editingRoom?.capacity?.toString() || '40'}
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
		{/if}

		<!-- Delete Confirm -->
		{#if canDeleteFacility}
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
						<Button variant="destructive" onclick={handleDelete} disabled={submitting}
							>ยืนยันลบ</Button
						>
					</Dialog.Footer>
				</Dialog.Content>
			</Dialog.Root>
		{/if}
	{/if}
</PageShell>
