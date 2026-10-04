<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import type { PageProps } from './$types';
	import { authStore } from '#lib/stores/auth.js';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { captureRouteLoad } from '#lib/navigation/route-load.js';
	import { toast } from 'svelte-sonner';

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
	} from '#lib/api/facility.js';

	import * as Card from '#lib/components/ui/card/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import * as Tabs from '#lib/components/ui/tabs/index.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';

	import {
		Building as BuildingIcon,
		DoorOpen,
		Plus,
		Search,
		Settings,
		Trash2
	} from '@lucide/svelte';

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

	let { data }: PageProps = $props();
	const source = $derived(data.buildings),
		buildingsRequest = new LatestRequest(),
		roomsRequest = new LatestRequest();
	let buildingsLoaded = $state(false),
		buildingsError = $state(''),
		roomsLoaded = $state(false),
		roomsLoading = $state(false),
		roomsError = $state('');
	let owner = '',
		ownerEpoch = 0,
		draftEpoch = 0,
		activeRoomsKey = '',
		disposed = false;
	const roomFiltersKey = $derived(`${selectedBuildingFilter}|${searchTerm.trim()}`);
	function applyBuildings(result: Awaited<typeof data.buildings>, revision: number) {
		if (!buildingsRequest.isCurrent(revision)) return;
		loading = false;
		if (result.ok) {
			buildings = result.data?.data ?? [];
			buildingsLoaded = true;
		} else buildingsError = result.error;
	}
	$effect.pre(() => {
		const operation = source,
			identity = `${$authStore.user?.id ?? ''}|${canReadFacility}`;
		untrack(() => {
			if (owner !== identity) {
				owner = identity;
				ownerEpoch++;
				buildings = [];
				buildingsLoaded = false;
				rooms = [];
				roomsLoaded = false;
				roomsRequest.abort();
				showBuildingDialog = false;
				showRoomDialog = false;
				showDeleteDialog = false;
				submitting = false;
			}
			const ticket = buildingsRequest.begin();
			loading = true;
			buildingsError = '';
			void operation.then((result) => applyBuildings(result, ticket.revision));
		});
		return () => buildingsRequest.abort();
	});
	$effect.pre(() => {
		const permissions = `${canCreateFacility}|${canUpdateFacility}|${canDeleteFacility}`;
		untrack(() => {
			void permissions;
			ownerEpoch++;
			submitting = false;
		});
	});
	$effect.pre(() => {
		const opened = activeTab === 'rooms',
			allowed = canReadFacility,
			key = roomFiltersKey;
		untrack(() => {
			if (!opened || !allowed) {
				roomsRequest.abort();
				rooms = [];
				roomsLoaded = false;
				roomsLoading = false;
				roomsError = '';
				return;
			}
			if (activeRoomsKey !== key) {
				activeRoomsKey = key;
				rooms = [];
				roomsLoaded = false;
			}
			void refreshRooms();
		});
		return () => roomsRequest.abort();
	});
	$effect.pre(() => {
		const context = `${showBuildingDialog}|${showRoomDialog}|${showDeleteDialog}`;
		untrack(() => {
			void context;
			draftEpoch++;
		});
	});
	onDestroy(() => {
		disposed = true;
		ownerEpoch++;
		buildingsRequest.abort();
		roomsRequest.abort();
	});
	async function loadBuildings() {
		if (!canReadFacility || disposed) return;
		const ticket = buildingsRequest.begin();
		loading = true;
		buildingsError = '';
		applyBuildings(
			await captureRouteLoad(listBuildings({ signal: ticket.signal }), 'โหลดรายการอาคารไม่สำเร็จ'),
			ticket.revision
		);
	}
	async function refreshRooms() {
		if (!canReadFacility || activeTab !== 'rooms' || disposed) return;
		const ticket = roomsRequest.begin();
		roomsLoading = true;
		roomsError = '';
		const result = await captureRouteLoad(
			listRooms(
				{
					building_id: selectedBuildingFilter === 'all' ? undefined : selectedBuildingFilter,
					search: searchTerm.trim() || undefined
				},
				{ signal: ticket.signal }
			),
			'โหลดรายการห้องไม่สำเร็จ'
		);
		if (!roomsRequest.isCurrent(ticket.revision)) return;
		roomsLoading = false;
		if (result.ok) {
			rooms = result.data.data;
			roomsLoaded = true;
		} else roomsError = result.error;
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
		if (selectedBuildingFilter === id) selectedBuildingFilter = 'all';
		buildings = buildings.filter((building) => building.id !== id);
		rooms = rooms
			.map((room) =>
				room.building_id === id ? { ...room, building_id: null, building_name: null } : room
			)
			.filter(roomMatchesCurrentFilters);
	}

	function roomMatchesCurrentFilters(room: Room) {
		if (selectedBuildingFilter !== 'all' && room.building_id !== selectedBuildingFilter)
			return false;
		const query = searchTerm.trim().toLowerCase();
		if (!query) return true;
		return [room.name_th, room.code]
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

	// Actions snapshot owner, target and draft before awaiting server state.
	async function handleSaveBuilding(e: SubmitEvent) {
		e.preventDefault();
		const target = editingItem?.id;
		if (
			disposed ||
			submitting ||
			!showBuildingDialog ||
			!buildingsLoaded ||
			(target ? !canUpdateFacility : !canCreateFacility)
		)
			return;
		const epoch = ownerEpoch,
			draft = draftEpoch;
		const current = () =>
			!disposed && epoch === ownerEpoch && (target ? canUpdateFacility : canCreateFacility);
		const ownsDraft = () => current() && draft === draftEpoch && showBuildingDialog;
		const formData = new FormData(e.target as HTMLFormElement);
		const payload = {
			name_th: String(formData.get('name_th') ?? ''),
			name_en: String(formData.get('name_en') ?? ''),
			code: String(formData.get('code') ?? ''),
			description: String(formData.get('description') ?? '')
		};
		buildingsRequest.abort();
		loading = false;
		submitting = true;
		try {
			const saved = (target ? await updateBuilding(target, payload) : await createBuilding(payload))
				.data;
			if (!current()) return;
			buildingsRequest.abort();
			loading = false;
			roomsRequest.abort();
			roomsLoading = false;
			replaceBuilding(saved);
			// A building rename also changes joined names used by the visible room list.
			if (activeTab === 'rooms' && !roomsLoaded) await refreshRooms();
			if (ownsDraft()) {
				toast.success(target ? 'บันทึกข้อมูลอาคารสำเร็จ' : 'เพิ่มอาคารสำเร็จ');
				showBuildingDialog = false;
			}
		} catch (error) {
			if (ownsDraft()) toast.error(error instanceof Error ? error.message : 'บันทึกไม่สำเร็จ');
		} finally {
			if (current()) submitting = false;
		}
	}
	async function handleSaveRoom(e: SubmitEvent) {
		e.preventDefault();
		const target = editingItem?.id;
		if (
			disposed ||
			submitting ||
			!showRoomDialog ||
			!buildingsLoaded ||
			buildingsError ||
			(target ? !canUpdateFacility : !canCreateFacility)
		)
			return;
		const epoch = ownerEpoch,
			draft = draftEpoch;
		const current = () =>
			!disposed && epoch === ownerEpoch && (target ? canUpdateFacility : canCreateFacility);
		const ownsDraft = () => current() && draft === draftEpoch && showRoomDialog;
		const formData = new FormData(e.target as HTMLFormElement);
		const payload = {
			name_th: String(formData.get('name_th') ?? ''),
			name_en: String(formData.get('name_en') ?? ''),
			code: String(formData.get('code') ?? ''),
			room_type: String(formData.get('room_type') ?? ''),
			building_id: String(formData.get('building_id') ?? '') || undefined,
			capacity: parseInt(String(formData.get('capacity'))) || 40,
			floor: parseInt(String(formData.get('floor'))) || undefined,
			description: String(formData.get('description') ?? '')
		};
		roomsRequest.abort();
		roomsLoading = false;
		submitting = true;
		try {
			const saved = (target ? await updateRoom(target, payload) : await createRoom(payload)).data;
			if (!current()) return;
			roomsRequest.abort();
			roomsLoading = false;
			if (roomsLoaded) replaceRoom(saved);
			else if (activeTab === 'rooms') await refreshRooms();
			if (ownsDraft()) {
				toast.success(target ? 'บันทึกข้อมูลห้องสำเร็จ' : 'เพิ่มห้องสำเร็จ');
				showRoomDialog = false;
			}
		} catch (error) {
			if (ownsDraft()) toast.error(error instanceof Error ? error.message : 'บันทึกไม่สำเร็จ');
		} finally {
			if (current()) submitting = false;
		}
	}
	async function handleDelete() {
		const target = deleteTarget;
		if (!target || !canDeleteFacility || disposed || submitting || !showDeleteDialog) return;
		const epoch = ownerEpoch,
			draft = draftEpoch;
		const current = () => !disposed && epoch === ownerEpoch && canDeleteFacility;
		const ownsDraft = () => current() && draft === draftEpoch && showDeleteDialog;
		submitting = true;
		try {
			if (target.type === 'building') await deleteBuilding(target.id);
			else await deleteRoom(target.id);
			if (!current()) return;
			roomsRequest.abort();
			roomsLoading = false;
			if (target.type === 'building') {
				buildingsRequest.abort();
				loading = false;
				removeBuilding(target.id);
			} else removeRoom(target.id);
			if (activeTab === 'rooms' && !roomsLoaded) await refreshRooms();
			if (ownsDraft()) {
				toast.success(target.type === 'building' ? 'ลบอาคารสำเร็จ' : 'ลบห้องสำเร็จ');
				showDeleteDialog = false;
			}
		} catch (error) {
			if (ownsDraft())
				toast.error(error instanceof Error ? error.message : 'ลบไม่สำเร็จ (อาจมีข้อมูลเชื่อมโยง)');
		} finally {
			if (current()) submitting = false;
		}
	}

	// Helpers
	function openAddBuilding() {
		if (!buildingsLoaded || !canCreateFacility) return;
		editingItem = null;
		showBuildingDialog = true;
	}
	function openEditBuilding(b: Building) {
		if (!canUpdateFacility) return;
		editingItem = b;
		showBuildingDialog = true;
	}
	function openAddRoom() {
		if (!buildingsLoaded || buildingsError || !canCreateFacility) return;
		editingItem = null;
		showRoomDialog = true;
	}
	function openEditRoom(r: Room) {
		if (!buildingsLoaded || buildingsError || !canUpdateFacility) return;
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
</script>

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
				<section data-testid="facility-buildings" aria-busy={loading}>
					{#if loading && buildingsLoaded}<p role="status">กำลังอัปเดตอาคาร</p>{/if}
					{#if buildingsError}<PageState
							variant="error"
							title="โหลดอาคารไม่สำเร็จ"
							description={buildingsError}
							actionLabel="ลองอีกครั้ง"
							onaction={loadBuildings}
						/>{/if}
					{#if canCreateFacility}
						<div class="flex justify-end">
							<Button onclick={openAddBuilding} disabled={!buildingsLoaded}>
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
								{#if loading && !buildingsLoaded}
									<Table.Row>
										<Table.Cell colspan={canMutateFacility ? 4 : 3} class="p-0">
											<div role="status" aria-label="กำลังโหลดอาคาร">
												<PageSkeleton
													variant="table"
													rows={4}
													columns={canMutateFacility ? 4 : 3}
												/>
											</div>
										</Table.Cell>
									</Table.Row>
								{:else if buildingsLoaded && buildings.length === 0}
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
								{:else if buildingsLoaded}
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
														<Button
															variant="ghost"
															size="icon"
															aria-label={`แก้ไขอาคาร ${b.name_th}`}
															onclick={() => openEditBuilding(b)}
														>
															<Settings class="w-4 h-4" />
														</Button>
													{/if}
													{#if canDeleteFacility}
														<Button
															variant="ghost"
															size="icon"
															class="text-destructive"
															aria-label={`ลบอาคาร ${b.name_th}`}
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
				</section>
			</Tabs.Content>

			<!-- Rooms Tab -->
			<Tabs.Content value="rooms" class="space-y-4 pt-4">
				<section data-testid="facility-rooms" aria-busy={roomsLoading}>
					<div data-testid="facility-building-options">
						{#if loading && !buildingsLoaded}<p role="status" aria-label="กำลังโหลดตัวเลือกอาคาร">
								กำลังโหลดตัวเลือกอาคาร
							</p>{/if}
						{#if buildingsError}<PageState
								variant="error"
								title="โหลดตัวเลือกอาคารไม่สำเร็จ"
								description={buildingsError}
								actionLabel="ลองอีกครั้ง"
								onaction={loadBuildings}
							/>{/if}
					</div>
					{#if roomsLoading && roomsLoaded}<p role="status">กำลังอัปเดตห้อง</p>{/if}
					{#if roomsError}<PageState
							variant="error"
							title="โหลดห้องไม่สำเร็จ"
							description={roomsError}
							actionLabel="ลองอีกครั้ง"
							onaction={refreshRooms}
						/>{/if}
					<div class="flex flex-wrap items-center gap-3 rounded-xl border bg-card p-3 sm:p-4">
						<div class="w-[200px]">
							<Select.Root
								type="single"
								bind:value={selectedBuildingFilter}
								disabled={!buildingsLoaded || !!buildingsError}
							>
								<Select.Trigger class="w-full" aria-label="เลือกอาคารสำหรับห้อง">
									{selectedBuildingFilter === 'all'
										? 'ทุกอาคาร'
										: buildings.find((b) => b.id === selectedBuildingFilter)?.name_th ||
											'เลือกอาคาร'}
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
							<Input placeholder="ค้นหาชื่อห้อง/รหัส..." class="pl-8" bind:value={searchTerm} />
						</div>
						{#if canCreateFacility}
							<div class="ml-auto">
								<Button onclick={openAddRoom} disabled={!buildingsLoaded || !!buildingsError}>
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
								{#if roomsLoading && !roomsLoaded}
									<Table.Row>
										<Table.Cell colspan={canMutateFacility ? 6 : 5} class="p-0">
											<div role="status" aria-label="กำลังโหลดห้อง">
												<PageSkeleton
													variant="table"
													rows={4}
													columns={canMutateFacility ? 6 : 5}
												/>
											</div>
										</Table.Cell>
									</Table.Row>
								{:else if roomsLoaded && rooms.length === 0}
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
								{:else if roomsLoaded}
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
														<Button
															variant="ghost"
															size="icon"
															aria-label={`แก้ไขห้อง ${r.name_th}`}
															onclick={() => openEditRoom(r)}
														>
															<Settings class="w-4 h-4" />
														</Button>
													{/if}
													{#if canDeleteFacility}
														<Button
															variant="ghost"
															size="icon"
															class="text-destructive"
															aria-label={`ลบห้อง ${r.name_th}`}
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
				</section>
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
							<Button type="submit" disabled={submitting || !buildingsLoaded || !!buildingsError}
								>บันทึก</Button
							>
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
