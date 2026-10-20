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
		type CreateRoomRequest,
		listBuildings,
		listRooms,
		createBuilding,
		updateBuilding,
		deleteBuilding,
		createRoom,
		updateRoom,
		deleteRoom
	} from '#lib/api/facility.js';

	import { Button } from '#lib/components/ui/button/index.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import BuildingResults from '#lib/components/facility/BuildingResults.svelte';
	import RoomResults from '#lib/components/facility/RoomResults.svelte';
	import * as AlertDialog from '#lib/components/ui/alert-dialog/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import { PageSkeleton, PageState, LoadingButton } from '#lib/components/app-state/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import * as Tabs from '#lib/components/ui/tabs/index.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';

	import { Building as BuildingIcon, DoorOpen, Plus, Search } from '@lucide/svelte';

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

	// Filters
	let searchTerm = $state('');
	let buildingSearch = $state('');
	let buildingPage = $state(1);
	let roomPage = $state(1);
	const pageSize = 25;
	const filteredBuildings = $derived(
		buildings.filter((building) =>
			[building.code, building.name_th, building.name_en, building.description].some((value) =>
				value?.toLocaleLowerCase().includes(buildingSearch.trim().toLocaleLowerCase())
			)
		)
	);
	const currentBuildingPage = $derived(
		Math.min(buildingPage, Math.max(1, Math.ceil(filteredBuildings.length / pageSize)))
	);
	const currentRoomPage = $derived(
		Math.min(roomPage, Math.max(1, Math.ceil(rooms.length / pageSize)))
	);
	const visibleBuildings = $derived(
		filteredBuildings.slice((currentBuildingPage - 1) * pageSize, currentBuildingPage * pageSize)
	);
	const visibleRooms = $derived(
		rooms.slice((currentRoomPage - 1) * pageSize, currentRoomPage * pageSize)
	);
	let selectedBuildingFilter = $state('all');

	// Dialogs
	let showBuildingDialog = $state(false);
	let showRoomDialog = $state(false);
	let showDeleteDialog = $state(false);
	let submitting = $state(false);
	let buildingSubmitError = $state('');
	let roomSubmitError = $state('');
	let deleteError = $state('');

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
				buildingSubmitError = roomSubmitError = deleteError = '';
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
			if (showBuildingDialog && (editingItem ? !canUpdateFacility : !canCreateFacility))
				showBuildingDialog = false;
			if (showRoomDialog && (editingItem ? !canUpdateFacility : !canCreateFacility))
				showRoomDialog = false;
			if (!canDeleteFacility) showDeleteDialog = false;
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
		buildingSubmitError = '';
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
			if (ownsDraft())
				buildingSubmitError = error instanceof Error ? error.message : 'บันทึกไม่สำเร็จ';
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
		let payload: CreateRoomRequest;
		roomSubmitError = '';
		try {
			payload = {
				name_th: String(formData.get('name_th') ?? ''),
				name_en: String(formData.get('name_en') ?? ''),
				code: String(formData.get('code') ?? ''),
				room_type: String(formData.get('room_type') ?? ''),
				building_id: String(formData.get('building_id') ?? '') || undefined,
				capacity: optionalInteger(formData.get('capacity'), 'ความจุ'),
				floor: optionalInteger(formData.get('floor'), 'ชั้น'),
				description: String(formData.get('description') ?? '')
			};
		} catch (error) {
			roomSubmitError = error instanceof Error ? error.message : 'ตรวจสอบข้อมูลตัวเลข';
			return;
		}
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
			if (ownsDraft()) roomSubmitError = error instanceof Error ? error.message : 'บันทึกไม่สำเร็จ';
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
		deleteError = '';
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
				deleteError = error instanceof Error ? error.message : 'ลบไม่สำเร็จ (อาจมีข้อมูลเชื่อมโยง)';
		} finally {
			if (current()) submitting = false;
		}
	}

	function optionalInteger(value: FormDataEntryValue | null, label: string): number | undefined {
		const text = String(value ?? '').trim();
		if (!text) return undefined;
		const result = Number(text);
		if (!Number.isInteger(result)) throw new Error(`${label}ต้องเป็นจำนวนเต็ม`);
		return result;
	}

	// Helpers
	function openAddBuilding() {
		if (!buildingsLoaded || !canCreateFacility) return;
		editingItem = null;
		buildingSubmitError = '';
		showBuildingDialog = true;
	}
	function openEditBuilding(b: Building) {
		if (!canUpdateFacility) return;
		editingItem = b;
		buildingSubmitError = '';
		showBuildingDialog = true;
	}
	function openAddRoom() {
		if (!buildingsLoaded || buildingsError || !canCreateFacility) return;
		editingItem = null;
		roomSubmitError = '';
		showRoomDialog = true;
	}
	function openEditRoom(r: Room) {
		if (!buildingsLoaded || buildingsError || !canUpdateFacility) return;
		editingItem = r;
		roomSubmitError = '';
		showRoomDialog = true;
	}
	function confirmDelete(type: 'building' | 'room', item: Building | Room) {
		if (!canDeleteFacility) return;
		deleteTarget = {
			type,
			id: item.id,
			name: item.name_th
		};
		deleteError = '';
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

{#snippet pagination(
	total: number,
	current: number,
	onchange: (page: number) => void,
	label: string
)}
	{#if total > pageSize}
		<nav
			class="mt-3 flex flex-wrap items-center justify-between gap-2 text-xs text-muted-foreground"
			aria-label={label}
		>
			<span class="tabular-nums"
				>{(current - 1) * pageSize + 1}–{Math.min(current * pageSize, total)} จาก {total}</span
			>
			<div class="flex items-center gap-2">
				<Button
					variant="outline"
					size="sm"
					disabled={current <= 1}
					onclick={() => onchange(current - 1)}>ก่อนหน้า</Button
				><Button
					variant="outline"
					size="sm"
					disabled={current * pageSize >= total}
					onclick={() => onchange(current + 1)}>ถัดไป</Button
				>
			</div>
		</nav>
	{/if}
{/snippet}

<PageShell title="จัดการอาคารสถานที่" description="อาคาร ห้องเรียน และห้องปฏิบัติการ">
	{#snippet actions()}
		{#if canReadFacility && canCreateFacility}
			<Button
				size="sm"
				disabled={!buildingsLoaded || (activeTab === 'rooms' && !!buildingsError)}
				onclick={activeTab === 'rooms' ? openAddRoom : openAddBuilding}
				><Plus class="size-4" />{activeTab === 'rooms' ? 'เพิ่มห้อง' : 'เพิ่มอาคาร'}</Button
			>
		{/if}
	{/snippet}
	{#if !canReadFacility}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ดูอาคารสถานที่"
			description="หน้านี้ต้องใช้สิทธิ์อ่านข้อมูลอาคารสถานที่ก่อนจึงจะแสดงรายการอาคารและห้องได้"
		/>
	{:else}
		<Tabs.Root bind:value={activeTab}>
			<Tabs.List
				><Tabs.Trigger value="buildings"><BuildingIcon class="size-4" />อาคาร</Tabs.Trigger
				><Tabs.Trigger value="rooms"
					><DoorOpen class="size-4" />ห้องเรียน/ห้องปฏิบัติการ</Tabs.Trigger
				></Tabs.List
			>
			<Tabs.Content value="buildings" class="space-y-3 pt-3">
				<section class="min-w-0 space-y-3" data-testid="facility-buildings" aria-busy={loading}>
					<div class="relative max-w-md">
						<Search
							class="pointer-events-none absolute top-2.5 left-3 size-4 text-muted-foreground"
						/><Input
							aria-label="ค้นหาอาคาร"
							placeholder="ค้นหาชื่ออาคารหรือรหัส"
							class="pl-9"
							bind:value={buildingSearch}
							oninput={() => (buildingPage = 1)}
						/>
					</div>
					{#if buildingsError}<PageState
							variant="error"
							title="โหลดอาคารไม่สำเร็จ"
							description={buildingsError}
							actionLabel="ลองอีกครั้ง"
							onaction={loadBuildings}
						/>{/if}
					{#if loading && !buildingsLoaded}<div role="status" aria-label="กำลังโหลดอาคาร">
							<PageSkeleton variant="table" rows={4} columns={4} />
						</div>
					{:else if buildingsLoaded && filteredBuildings.length === 0}<PageState
							title={buildingSearch.trim() ? 'ไม่พบอาคารที่ค้นหา' : 'ไม่พบข้อมูลอาคาร'}
							description={buildingSearch.trim()
								? 'ลองเปลี่ยนชื่อหรือรหัสที่ค้นหา'
								: 'เพิ่มอาคารเพื่อใช้จัดกลุ่มห้องเรียนและห้องปฏิบัติการ'}
						/>
					{:else if buildingsLoaded}<BuildingResults
							items={visibleBuildings}
							canUpdate={canUpdateFacility}
							canDelete={canDeleteFacility}
							onedit={openEditBuilding}
							ondelete={(item) => confirmDelete('building', item)}
						/>{@render pagination(
							filteredBuildings.length,
							currentBuildingPage,
							(page) => (buildingPage = page),
							'หน้ารายการอาคาร'
						)}{/if}
				</section>
			</Tabs.Content>
			<Tabs.Content value="rooms" class="space-y-3 pt-3">
				<section class="min-w-0 space-y-3" data-testid="facility-rooms" aria-busy={roomsLoading}>
					<div data-testid="facility-building-options">
						{#if loading && !buildingsLoaded}<p
								class="text-sm text-muted-foreground"
								role="status"
								aria-label="กำลังโหลดตัวเลือกอาคาร"
							>
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
					<div class="flex flex-col gap-2 sm:flex-row">
						<Select.Root
							type="single"
							bind:value={selectedBuildingFilter}
							onValueChange={() => (roomPage = 1)}
							disabled={!buildingsLoaded || !!buildingsError}
						>
							<Select.Trigger class="w-full min-w-0 sm:w-56" aria-label="เลือกอาคารสำหรับห้อง"
								><span class="truncate"
									>{selectedBuildingFilter === 'all'
										? 'ทุกอาคาร'
										: (buildings.find((building) => building.id === selectedBuildingFilter)
												?.name_th ?? 'เลือกอาคาร')}</span
								></Select.Trigger
							>
							<Select.Content
								><Select.Item value="all">ทุกอาคาร</Select.Item
								>{#each buildings as building (building.id)}<Select.Item value={building.id}
										>{building.name_th}</Select.Item
									>{/each}</Select.Content
							>
						</Select.Root>
						<div class="relative min-w-0 flex-1 sm:max-w-md">
							<Search
								class="pointer-events-none absolute top-2.5 left-3 size-4 text-muted-foreground"
							/><Input
								aria-label="ค้นหาห้อง"
								placeholder="ค้นหาชื่อห้องหรือรหัส"
								class="pl-9"
								bind:value={searchTerm}
								oninput={() => (roomPage = 1)}
							/>
						</div>
					</div>
					{#if roomsError}<PageState
							variant="error"
							title="โหลดห้องไม่สำเร็จ"
							description={roomsError}
							actionLabel="ลองอีกครั้ง"
							onaction={refreshRooms}
						/>{/if}
					{#if roomsLoading && !roomsLoaded}<div role="status" aria-label="กำลังโหลดห้อง">
							<PageSkeleton variant="table" rows={4} columns={6} />
						</div>
					{:else if roomsLoaded && rooms.length === 0}<PageState
							title="ไม่พบข้อมูลห้อง"
							description={searchTerm.trim() || selectedBuildingFilter !== 'all'
								? 'ลองเปลี่ยนคำค้นหาหรืออาคารที่เลือก'
								: 'เพิ่มห้องเรียนหรือห้องปฏิบัติการเพื่อใช้กับตารางสอนและงานอาคาร'}
						/>
					{:else if roomsLoaded}<RoomResults
							items={visibleRooms}
							roomTypes={ROOM_TYPES}
							canUpdate={canUpdateFacility}
							canDelete={canDeleteFacility}
							onedit={openEditRoom}
							ondelete={(item) => confirmDelete('room', item)}
						/>{@render pagination(
							rooms.length,
							currentRoomPage,
							(page) => (roomPage = page),
							'หน้ารายการห้อง'
						)}{/if}
				</section>
			</Tabs.Content>
		</Tabs.Root>

		{#if canCreateFacility || canUpdateFacility}
			<Dialog.Root bind:open={showBuildingDialog}>
				<Dialog.Content class="max-h-[calc(100dvh-2rem)] overflow-y-auto sm:max-w-lg">
					<Dialog.Header
						><Dialog.Title>{editingItem ? 'แก้ไขอาคาร' : 'เพิ่มอาคารใหม่'}</Dialog.Title
						><Dialog.Description>ระบุชื่ออาคารและข้อมูลที่ใช้จัดกลุ่มห้อง</Dialog.Description
						></Dialog.Header
					>
					<form onsubmit={handleSaveBuilding} class="space-y-4">
						<div class="grid gap-4 sm:grid-cols-2">
							<div class="space-y-2">
								<Label for="building-code">รหัสอาคาร</Label><Input
									id="building-code"
									name="code"
									value={editingItem?.code || ''}
									placeholder="เช่น BLD1"
								/>
							</div>
							<div class="space-y-2">
								<Label for="building-name-th"
									>ชื่ออาคาร (ไทย) <span class="text-destructive">*</span></Label
								><Input
									id="building-name-th"
									name="name_th"
									value={editingItem?.name_th || ''}
									required
								/>
							</div>
						</div>
						<div class="space-y-2">
							<Label for="building-name-en">ชื่ออาคาร (อังกฤษ)</Label><Input
								id="building-name-en"
								name="name_en"
								value={editingItem?.name_en || ''}
							/>
						</div>
						<div class="space-y-2">
							<Label for="building-description">รายละเอียด</Label><Textarea
								id="building-description"
								name="description"
								value={editingItem?.description || ''}
								rows={3}
							/>
						</div>
						{#if buildingSubmitError}<p
								role="alert"
								class="rounded-md border border-destructive/30 bg-destructive/5 p-3 text-sm text-destructive"
							>
								{buildingSubmitError}
							</p>{/if}
						<Dialog.Footer
							><Button variant="outline" type="button" onclick={() => (showBuildingDialog = false)}
								>ยกเลิก</Button
							><LoadingButton
								type="submit"
								loading={submitting}
								loadingLabel="กำลังบันทึก..."
								disabled={!buildingsLoaded ||
									(editingItem ? !canUpdateFacility : !canCreateFacility)}>บันทึก</LoadingButton
							></Dialog.Footer
						>
					</form>
				</Dialog.Content>
			</Dialog.Root>
			<Dialog.Root bind:open={showRoomDialog}>
				<Dialog.Content class="max-h-[calc(100dvh-2rem)] overflow-y-auto sm:max-w-xl">
					<Dialog.Header
						><Dialog.Title>{editingItem ? 'แก้ไขห้อง' : 'เพิ่มห้องใหม่'}</Dialog.Title
						><Dialog.Description
							>ระบุห้อง อาคาร และความจุสำหรับการใช้งานในโรงเรียน</Dialog.Description
						></Dialog.Header
					>
					<form onsubmit={handleSaveRoom} class="space-y-4">
						<div class="grid gap-4 sm:grid-cols-2">
							<div class="min-w-0 space-y-2">
								<Label for="room-building">อาคาร</Label><Select.Root
									type="single"
									bind:value={formBuildingId}
									><Select.Trigger id="room-building" class="w-full min-w-0"
										><span class="truncate"
											>{buildings.find((building) => building.id === formBuildingId)?.name_th ||
												'ไม่ระบุอาคาร'}</span
										></Select.Trigger
									><Select.Content
										>{#each buildings as building (building.id)}<Select.Item value={building.id}
												>{building.name_th}</Select.Item
											>{/each}</Select.Content
									></Select.Root
								><input type="hidden" name="building_id" value={formBuildingId} />
							</div>
							<div class="space-y-2">
								<Label for="room-type">ประเภทห้อง</Label><Select.Root
									type="single"
									bind:value={formRoomType}
									><Select.Trigger id="room-type" class="w-full"
										>{ROOM_TYPES.find((type) => type.value === formRoomType)?.label}</Select.Trigger
									><Select.Content
										>{#each ROOM_TYPES as type (type.value)}<Select.Item value={type.value}
												>{type.label}</Select.Item
											>{/each}</Select.Content
									></Select.Root
								><input type="hidden" name="room_type" value={formRoomType} />
							</div>
							<div class="space-y-2">
								<Label for="room-code">รหัส / เลขห้อง <span class="text-destructive">*</span></Label
								><Input id="room-code" name="code" value={editingItem?.code || ''} required />
							</div>
							<div class="space-y-2">
								<Label for="room-name-th"
									>ชื่อห้อง (ไทย) <span class="text-destructive">*</span></Label
								><Input
									id="room-name-th"
									name="name_th"
									value={editingItem?.name_th || ''}
									required
								/>
							</div>
							<div class="space-y-2 sm:col-span-2">
								<Label for="room-name-en">ชื่อห้อง (อังกฤษ)</Label><Input
									id="room-name-en"
									name="name_en"
									value={editingItem?.name_en || ''}
								/>
							</div>
							<div class="space-y-2">
								<Label for="room-floor">ชั้น</Label><Input
									id="room-floor"
									type="number"
									step="1"
									name="floor"
									value={editingRoom?.floor?.toString() ?? ''}
								/>
							</div>
							<div class="space-y-2">
								<Label for="room-capacity">ความจุ (คน)</Label><Input
									id="room-capacity"
									type="number"
									step="1"
									name="capacity"
									value={editingRoom?.capacity?.toString() ?? '40'}
								/>
							</div>
						</div>
						<div class="space-y-2">
							<Label for="room-description">รายละเอียดเพิ่มเติม</Label><Textarea
								id="room-description"
								name="description"
								value={editingItem?.description || ''}
								rows={3}
							/>
						</div>
						{#if roomSubmitError}<p
								role="alert"
								class="rounded-md border border-destructive/30 bg-destructive/5 p-3 text-sm text-destructive"
							>
								{roomSubmitError}
							</p>{/if}
						<Dialog.Footer
							><Button variant="outline" type="button" onclick={() => (showRoomDialog = false)}
								>ยกเลิก</Button
							><LoadingButton
								type="submit"
								loading={submitting}
								loadingLabel="กำลังบันทึก..."
								disabled={!buildingsLoaded ||
									!!buildingsError ||
									(editingItem ? !canUpdateFacility : !canCreateFacility)}>บันทึก</LoadingButton
							></Dialog.Footer
						>
					</form>
				</Dialog.Content>
			</Dialog.Root>
		{/if}
		{#if canDeleteFacility}
			<AlertDialog.Root bind:open={showDeleteDialog}>
				<AlertDialog.Content class="max-h-[calc(100dvh-2rem)] overflow-y-auto">
					<AlertDialog.Header
						><AlertDialog.Title>ยืนยันการลบ</AlertDialog.Title><AlertDialog.Description
							class="break-words"
							>ลบ “{deleteTarget?.name}” หรือไม่? การกระทำนี้ไม่สามารถเรียกคืนได้{#if deleteTarget?.type === 'building'}
								ห้องในอาคารจะยังอยู่และแสดงเป็นไม่ระบุอาคาร{/if}</AlertDialog.Description
						></AlertDialog.Header
					>
					{#if deleteError}<p
							role="alert"
							class="rounded-md border border-destructive/30 bg-destructive/5 p-3 text-sm text-destructive"
						>
							{deleteError}
						</p>{/if}
					<AlertDialog.Footer
						><AlertDialog.Cancel>ยกเลิก</AlertDialog.Cancel><AlertDialog.Action
							variant="destructive"
							disabled={submitting}
							onclick={(event) => {
								event.preventDefault();
								void handleDelete();
							}}>{submitting ? 'กำลังลบ...' : 'ยืนยันลบ'}</AlertDialog.Action
						></AlertDialog.Footer
					>
				</AlertDialog.Content>
			</AlertDialog.Root>
		{/if}
	{/if}
</PageShell>
