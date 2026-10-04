<script lang="ts">
	import { downloadXlsxWorkbook } from '#lib/utils/spreadsheet.js';
	import { untrack } from 'svelte';
	import type { PageProps } from './$types';
	import {
		getRound,
		listRounds,
		listExamRooms,
		addExamRoom,
		updateExamRoom,
		removeExamRoom,
		copyExamRoomsFromRound,
		getExamConfig,
		updateExamConfig,
		assignExamSeats,
		getExamSeats,
		type AdmissionRound,
		type ExamRoom,
		type ExamRoomGroup,
		type ExamConfig
	} from '#lib/api/admission.js';
	import { listRooms, type Room } from '#lib/api/facility.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { RegionUpdatingState } from '#lib/components/app-state/index.js';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';
	import { toast } from 'svelte-sonner';
	import {
		Building2,
		Plus,
		Trash2,
		Loader2,
		ClipboardList,
		FileDown,
		FileSpreadsheet,
		Copy,
		Settings,
		RefreshCw,
		Check
	} from '@lucide/svelte';

	let { data }: PageProps = $props();
	let id = $derived(data.id);
	const canManageAdmission = $derived($can.has(PERMISSIONS.ADMISSION_MANAGE_ALL));
	const roundRequest = new LatestRequest();
	const roomsRequest = new LatestRequest();
	const configRequest = new LatestRequest();
	const seatsRequest = new LatestRequest();
	const facilityRequest = new LatestRequest();
	const copyRoundsRequest = new LatestRequest();
	let renderedId = '';

	let round: AdmissionRound | null = $state(null);
	let roundLoaded = $state(false);
	let roundLoading = $state(true);
	let roundError = $state('');
	let allRounds: AdmissionRound[] = $state([]);
	let copyRoundsLoaded = $state(false);
	let copyRoundsLoading = $state(false);
	let copyRoundsError = $state('');
	let examRooms: ExamRoom[] = $state([]);
	let totalCapacity = $state(0);
	let totalAssigned = $state(0);
	let roomsLoaded = $state(false);
	let roomsLoading = $state(true);
	let roomsError = $state('');
	let facilityRooms: Room[] = $state([]);
	let facilityLoaded = $state(false);
	let facilityLoading = $state(false);
	let facilityError = $state('');
	let examConfig: ExamConfig = $state({
		examIdType: 'application_number',
		sortOrder: 'by_application'
	});
	let configLoaded = $state(false);
	let configLoading = $state(true);
	let configError = $state('');
	let seatGroups: ExamRoomGroup[] = $state([]);
	let seatsLoaded = $state(false);
	let seatsLoading = $state(false);
	let seatsError = $state('');

	let activeTab = $state<'setup' | 'seats'>('setup');
	let assigning = $state(false);
	let savingConfig = $state(false);
	let copying = $state(false);

	// Add room dialog
	let showAddRoomDialog = $state(false);
	let addRoomMode = $state<'facility' | 'custom'>('facility');
	let selectedFacilityRoomId = $state('');
	let customRoomName = $state('');
	let customRoomCapacity = $state(40);
	let addingRoom = $state(false);

	// Edit capacity inline
	let editingCapacityId = $state<string | null>(null);
	let editingCapacityValue = $state(0);

	// Copy from round
	let copyFromRoundId = $state('');

	// Assign seats dialog
	let showAssignDialog = $state(false);
	let assignMode = $state<'full' | 'append'>('full');

	function applyConfig(config: ExamConfig) {
		examConfig = {
			examIdType: config.examIdType ?? 'application_number',
			examIdPrefix: config.examIdPrefix ?? '',
			sortOrder: config.sortOrder ?? 'by_application'
		};
	}

	async function loadRound() {
		if (!id || !canManageAdmission) return;
		const sourceId = id;
		const { revision, signal } = roundRequest.begin();
		roundLoading = true;
		roundError = '';
		try {
			const value = await getRound(sourceId, { signal });
			if (!roundRequest.isCurrent(revision) || sourceId !== id) return;
			round = value;
			roundLoaded = true;
		} catch (cause) {
			if (!isAbortError(cause) && roundRequest.isCurrent(revision))
				roundError = cause instanceof Error ? cause.message : 'โหลดรอบรับสมัครไม่สำเร็จ';
		} finally {
			if (roundRequest.isCurrent(revision)) roundLoading = false;
		}
	}

	async function loadRooms() {
		if (!id || !canManageAdmission) return;
		const sourceId = id;
		const { revision, signal } = roomsRequest.begin();
		roomsLoading = true;
		roomsError = '';
		try {
			const value = await listExamRooms(sourceId, { signal });
			if (!roomsRequest.isCurrent(revision) || sourceId !== id) return;
			replaceExamRooms(value);
			roomsLoaded = true;
		} catch (cause) {
			if (!isAbortError(cause) && roomsRequest.isCurrent(revision))
				roomsError = cause instanceof Error ? cause.message : 'โหลดห้องสอบไม่สำเร็จ';
		} finally {
			if (roomsRequest.isCurrent(revision)) roomsLoading = false;
		}
	}

	async function loadConfig() {
		if (!id || !canManageAdmission) return;
		const sourceId = id;
		const { revision, signal } = configRequest.begin();
		configLoading = true;
		configError = '';
		try {
			const value = await getExamConfig(sourceId, { signal });
			if (!configRequest.isCurrent(revision) || sourceId !== id) return;
			applyConfig(value);
			configLoaded = true;
		} catch (cause) {
			if (!isAbortError(cause) && configRequest.isCurrent(revision))
				configError = cause instanceof Error ? cause.message : 'โหลดการตั้งค่าที่นั่งไม่สำเร็จ';
		} finally {
			if (configRequest.isCurrent(revision)) configLoading = false;
		}
	}

	async function loadFacilityRooms() {
		if (facilityLoaded || facilityLoading || !canManageAdmission) return;
		const sourceId = id;
		const { revision } = facilityRequest.begin();
		facilityLoading = true;
		facilityError = '';
		try {
			const result = await listRooms({});
			if (!facilityRequest.isCurrent(revision) || sourceId !== id) return;
			facilityRooms = result.data.filter((room: Room) => room.status === 'ACTIVE');
			facilityLoaded = true;
		} catch (cause) {
			if (facilityRequest.isCurrent(revision))
				facilityError = cause instanceof Error ? cause.message : 'โหลดรายชื่อห้องไม่สำเร็จ';
		} finally {
			if (facilityRequest.isCurrent(revision)) facilityLoading = false;
		}
	}

	async function loadCopyRounds() {
		if (!round || copyRoundsLoaded || copyRoundsLoading || !canManageAdmission) return;
		const sourceId = id;
		const { revision, signal } = copyRoundsRequest.begin();
		copyRoundsLoading = true;
		copyRoundsError = '';
		try {
			const rows = await listRounds(round.academicYearId, { signal });
			if (!copyRoundsRequest.isCurrent(revision) || sourceId !== id) return;
			allRounds = rows.filter((row) => row.id !== sourceId);
			copyRoundsLoaded = true;
		} catch (cause) {
			if (!isAbortError(cause) && copyRoundsRequest.isCurrent(revision))
				copyRoundsError = cause instanceof Error ? cause.message : 'โหลดรอบต้นทางไม่สำเร็จ';
		} finally {
			if (copyRoundsRequest.isCurrent(revision)) copyRoundsLoading = false;
		}
	}

	async function loadSeats(): Promise<ExamRoomGroup[] | null> {
		if (!id || !canManageAdmission) return null;
		const sourceId = id;
		const { revision, signal } = seatsRequest.begin();
		seatsLoading = true;
		seatsError = '';
		try {
			const result = await getExamSeats(sourceId, { signal });
			if (!seatsRequest.isCurrent(revision) || sourceId !== id) return null;
			seatGroups = result;
			seatsLoaded = true;
			return result;
		} catch (cause) {
			if (!isAbortError(cause) && seatsRequest.isCurrent(revision))
				seatsError = cause instanceof Error ? cause.message : 'โหลดผลจัดที่นั่งไม่สำเร็จ';
			return null;
		} finally {
			if (seatsRequest.isCurrent(revision)) seatsLoading = false;
		}
	}

	function updateRoomTotals(nextRooms = examRooms) {
		totalCapacity = nextRooms.reduce((sum, room) => sum + room.capacity, 0);
		totalAssigned = nextRooms.reduce((sum, room) => sum + room.assignedCount, 0);
	}

	function replaceExamRooms(data: {
		rooms: ExamRoom[];
		totalCapacity: number;
		totalAssigned: number;
	}) {
		examRooms = data.rooms;
		totalCapacity = data.totalCapacity;
		totalAssigned = data.totalAssigned;
	}

	function replaceExamRoom(room: ExamRoom) {
		const nextRooms = examRooms.some((item) => item.id === room.id)
			? examRooms.map((item) => (item.id === room.id ? room : item))
			: [...examRooms, room];
		examRooms = nextRooms.sort((a, b) => a.displayOrder - b.displayOrder);
		if (seatsLoaded)
			seatGroups = seatGroups.map((group) =>
				group.examRoomId === room.id
					? {
							...group,
							roomName: room.roomName,
							buildingName: room.buildingName,
							capacity: room.capacity
						}
					: group
			);
		updateRoomTotals();
	}

	function removeExamRoomFromList(roomId: string) {
		examRooms = examRooms.filter((room) => room.id !== roomId);
		seatGroups = seatGroups.filter((group) => group.examRoomId !== roomId);
		updateRoomTotals();
	}

	function applySeatAssignmentsToRooms(groups: ExamRoomGroup[]) {
		const assignedByRoom = new Map(groups.map((group) => [group.examRoomId, group.seats.length]));
		supersedeRoomsRead();
		examRooms = examRooms.map((room) => ({
			...room,
			assignedCount: assignedByRoom.get(room.id) ?? 0
		}));
		updateRoomTotals();
	}

	function supersedeRoomsRead() {
		roomsRequest.abort();
		roomsLoading = false;
		roomsError = '';
		roomsLoaded = true;
	}

	async function handleAddRoom() {
		if (!id || !canManageAdmission) return;
		const sourceId = id;
		addingRoom = true;
		try {
			let created: ExamRoom;
			if (addRoomMode === 'facility') {
				if (!selectedFacilityRoomId) {
					toast.error('กรุณาเลือกห้อง');
					return;
				}
				created = await addExamRoom(sourceId, { roomId: selectedFacilityRoomId });
			} else {
				if (!customRoomName.trim()) {
					toast.error('กรุณาระบุชื่อห้อง');
					return;
				}
				created = await addExamRoom(sourceId, {
					customName: customRoomName.trim(),
					capacityOverride: customRoomCapacity
				});
			}
			if (sourceId !== id) return;
			supersedeRoomsRead();
			replaceExamRoom(created);
			toast.success('เพิ่มห้องสอบแล้ว');
			showAddRoomDialog = false;
			selectedFacilityRoomId = '';
			customRoomName = '';
			customRoomCapacity = 40;
		} catch {
			toast.error('ไม่สามารถเพิ่มห้องสอบได้');
		} finally {
			addingRoom = false;
		}
	}

	async function handleRemoveRoom(roomId: string) {
		if (!id || !canManageAdmission || !confirm('ลบห้องสอบนี้?')) return;
		const sourceId = id;
		try {
			await removeExamRoom(sourceId, roomId);
			if (sourceId !== id) return;
			supersedeRoomsRead();
			removeExamRoomFromList(roomId);
			toast.success('ลบห้องสอบแล้ว');
		} catch {
			toast.error('ไม่สามารถลบห้องสอบได้');
		}
	}

	function startEditCapacity(room: ExamRoom) {
		if (!canManageAdmission) return;
		editingCapacityId = room.id;
		editingCapacityValue = room.capacity;
	}

	async function saveCapacity(roomId: string) {
		if (!id || !canManageAdmission || editingCapacityValue < 1) return;
		const sourceId = id;
		const capacityOverride = editingCapacityValue;
		try {
			const updated = await updateExamRoom(sourceId, roomId, { capacityOverride });
			if (sourceId !== id) return;
			supersedeRoomsRead();
			replaceExamRoom(updated);
			toast.success('อัปเดตความจุแล้ว');
			editingCapacityId = null;
		} catch {
			toast.error('ไม่สามารถอัปเดตความจุได้');
		}
	}

	async function handleCopyFromRound() {
		if (!canManageAdmission) return;
		if (!id || !copyFromRoundId || !copyRoundsLoaded || !roomsLoaded) {
			toast.error('กรุณาเลือกรอบที่ต้องการ copy');
			return;
		}
		copying = true;
		const sourceId = id;
		const fromRoundId = copyFromRoundId;
		try {
			const result = await copyExamRoomsFromRound(sourceId, fromRoundId);
			if (sourceId !== id) return;
			supersedeRoomsRead();
			replaceExamRooms(result);
			seatsRequest.abort();
			seatGroups = [];
			seatsLoaded = false;
			seatsLoading = false;
			seatsError = '';
			toast.success(result.message);
			copyFromRoundId = '';
		} catch {
			toast.error('ไม่สามารถ copy ห้องสอบได้');
		} finally {
			copying = false;
		}
	}

	async function handleSaveConfig() {
		if (!id || !canManageAdmission || !configLoaded) return;
		const sourceId = id;
		const nextConfig = { ...examConfig };
		savingConfig = true;
		try {
			await updateExamConfig(sourceId, nextConfig);
			if (sourceId !== id) return;
			configRequest.abort();
			configLoading = false;
			configError = '';
			toast.success('บันทึก config แล้ว');
		} catch {
			toast.error('ไม่สามารถบันทึก config ได้');
		} finally {
			savingConfig = false;
		}
	}

	async function handleAssignSeats() {
		if (!id || !canManageAdmission || !configLoaded || !roomsLoaded) return;
		const sourceId = id;
		const nextConfig = { ...examConfig };
		assigning = true;
		try {
			const result = await assignExamSeats(sourceId, {
				examIdType: nextConfig.examIdType,
				examIdPrefix: nextConfig.examIdPrefix,
				sortOrder: nextConfig.sortOrder,
				mode: assignMode
			});
			if (sourceId !== id) return;
			toast.success(result.message);
			showAssignDialog = false;
			activeTab = 'seats';
			const groups = await loadSeats();
			if (sourceId !== id) return;
			if (groups) applySeatAssignmentsToRooms(groups);
			else await loadRooms();
		} catch (e: unknown) {
			const err = e as { response?: { data?: { error?: string } } };
			toast.error(err?.response?.data?.error ?? 'ไม่สามารถจัดที่นั่งได้');
		} finally {
			assigning = false;
		}
	}

	// ===== PDF/XLSX Download helpers =====

	function printRoom(group: ExamRoomGroup) {
		const w = window.open('', '_blank');
		if (!w) return;
		const rows = group.seats
			.map(
				(s) => `<tr>
				<td>${s.examId ?? s.applicationNumber ?? ''}</td>
				<td style="text-align:center">${s.seatNumber}</td>
				<td>${s.fullName}</td>
				<td>${s.nationalId ?? ''}</td>
				<td>${s.trackName ?? ''}</td>
			</tr>`
			)
			.join('');
		w.document.write(`<!DOCTYPE html><html><head>
			<meta charset="utf-8">
			<title>รายชื่อ ${group.roomName}</title>
			<style>
				body{font-family:'Sarabun',sans-serif;font-size:14px;padding:20px}
				h2{margin-bottom:4px}h3{margin-bottom:16px;font-weight:normal}
				table{border-collapse:collapse;width:100%}
				th,td{border:1px solid #ccc;padding:6px 10px}
				th{background:#f5f5f5;font-weight:600}
				@media print{body{padding:0}}
			</style>
		</head><body>
			<h2>${round?.name ?? ''}</h2>
			<h3>ห้องสอบ: <strong>${group.roomName}</strong>${group.buildingName ? ' (' + group.buildingName + ')' : ''} — ความจุ ${group.capacity} ที่นั่ง</h3>
			<table>
				<thead><tr><th>เลขประจำตัวสอบ</th><th>ที่นั่ง</th><th>ชื่อ-นามสกุล</th><th>เลขบัตรประชาชน</th><th>สาย</th></tr></thead>
				<tbody>${rows}</tbody>
			</table>
			<p style="margin-top:12px;color:#666">รวม ${group.seats.length} คน</p>
			<script>window.onload=function(){window.print()}</${'script'}>
		</body></html>`);
		w.document.close();
	}

	async function downloadRoomXlsx(group: ExamRoomGroup) {
		const data = [
			['เลขประจำตัวสอบ', 'ที่นั่ง', 'ชื่อ-นามสกุล', 'เลขบัตรประชาชน', 'สาย'],
			...group.seats.map((s) => [
				s.examId ?? s.applicationNumber ?? '',
				s.seatNumber,
				s.fullName,
				s.nationalId ?? '',
				s.trackName ?? ''
			])
		];
		await downloadXlsxWorkbook(
			[{ name: group.roomName.slice(0, 31), rows: data }],
			`ห้องสอบ-${group.roomName}.xlsx`
		);
	}

	function printAllAdmitCards() {
		const w = window.open('', '_blank');
		if (!w) return;
		const examDate = round?.examDate
			? new Date(round.examDate).toLocaleDateString('th-TH', {
					year: 'numeric',
					month: 'long',
					day: 'numeric'
				})
			: '-';
		const cards = seatGroups
			.flatMap((g) =>
				g.seats.map(
					(s) => `
			<div style="border:2px solid #333;padding:16px;margin:8px;width:280px;display:inline-block;vertical-align:top;font-size:13px;font-family:'Sarabun',sans-serif">
				<div style="font-weight:bold;font-size:15px;margin-bottom:10px">${round?.name ?? ''}</div>
				<table style="width:100%;font-size:13px">
					<tr><td style="color:#555;padding:2px 0">ชื่อ-นามสกุล</td><td style="font-weight:600">${s.fullName}</td></tr>
					<tr><td style="color:#555;padding:2px 0">เลขประจำตัวสอบ</td><td style="font-weight:700;color:#1d4ed8;font-size:15px">${s.examId ?? s.applicationNumber ?? ''}</td></tr>
					<tr><td style="color:#555;padding:2px 0">สายการเรียน</td><td style="font-weight:600">${s.trackName ?? '-'}</td></tr>
					<tr><td style="color:#555;padding:2px 0">ห้องสอบ</td><td style="font-weight:600">${g.roomName}</td></tr>
					<tr><td style="color:#555;padding:2px 0">เลขที่นั่ง</td><td style="font-weight:600">${s.seatNumber}</td></tr>
					<tr><td style="color:#555;padding:2px 0">วันสอบ</td><td>${examDate}</td></tr>
				</table>
			</div>`
				)
			)
			.join('');
		w.document.write(`<!DOCTYPE html><html><head>
			<meta charset="utf-8"><title>บัตรสอบ</title>
			<style>body{padding:16px}@media print{@page{margin:8mm}}</style>
		</head><body>${cards}
			<script>window.onload=function(){window.print()}</${'script'}>
		</body></html>`);
		w.document.close();
	}

	async function downloadAllXlsx() {
		const data = [
			['ห้องสอบ', 'เลขที่นั่ง', 'เลขประจำตัวสอบ', 'ชื่อ-นามสกุล', 'เลขบัตรประชาชน', 'สาย'],
			...seatGroups.flatMap((g) =>
				g.seats.map((s) => [
					g.roomName,
					s.seatNumber,
					s.examId ?? s.applicationNumber ?? '',
					s.fullName,
					s.nationalId ?? '',
					s.trackName ?? ''
				])
			)
		];
		await downloadXlsxWorkbook(
			[{ name: 'ที่นั่งสอบทั้งหมด', rows: data }],
			`ที่นั่งสอบ-${round?.name ?? ''}.xlsx`
		);
	}

	const examIdTypeLabel: Record<string, string> = {
		application_number: 'เลขใบสมัคร',
		sequential: 'ลำดับต่อเนื่อง',
		custom_prefix: 'กำหนด Prefix เอง'
	};
	const sortOrderLabel: Record<string, string> = {
		by_application: 'ตามลำดับการสมัคร',
		by_track: 'แบ่งตามสาย',
		random: 'สุ่ม'
	};

	$effect.pre(() => {
		const routeRound = data.round;
		const routeRooms = data.rooms;
		const routeConfig = data.config;
		const routeId = data.id;
		const { revision: roundRevision } = roundRequest.begin();
		const { revision: roomsRevision } = roomsRequest.begin();
		const { revision: configRevision } = configRequest.begin();
		facilityRequest.abort();
		copyRoundsRequest.abort();
		seatsRequest.abort();
		untrack(() => {
			if (renderedId !== routeId) {
				renderedId = routeId;
				round = null;
				roundLoaded = false;
				examRooms = [];
				totalCapacity = 0;
				totalAssigned = 0;
				roomsLoaded = false;
				examConfig = { examIdType: 'application_number', sortOrder: 'by_application' };
				configLoaded = false;
				seatGroups = [];
				seatsLoaded = false;
				seatsLoading = false;
				seatsError = '';
				activeTab = 'setup';
				facilityRooms = [];
				facilityLoaded = false;
				facilityLoading = false;
				facilityError = '';
				allRounds = [];
				copyRoundsLoaded = false;
				copyRoundsLoading = false;
				copyRoundsError = '';
				copyFromRoundId = '';
				showAddRoomDialog = false;
				showAssignDialog = false;
				assigning = false;
				savingConfig = false;
				copying = false;
				addingRoom = false;
				editingCapacityId = null;
			}
			roundLoading = true;
			roomsLoading = true;
			configLoading = true;
			roundError = '';
			roomsError = '';
			configError = '';
		});
		void routeRound.then((result) => {
			if (!roundRequest.isCurrent(roundRevision)) return;
			untrack(() => {
				if (result.ok && result.data) {
					round = result.data;
					roundLoaded = true;
				} else if (!result.ok) roundError = result.error;
				roundLoading = false;
			});
		});
		void routeRooms.then((result) => {
			if (!roomsRequest.isCurrent(roomsRevision)) return;
			untrack(() => {
				if (result.ok && result.data) {
					replaceExamRooms(result.data);
					roomsLoaded = true;
				} else if (!result.ok) roomsError = result.error;
				roomsLoading = false;
			});
		});
		void routeConfig.then((result) => {
			if (!configRequest.isCurrent(configRevision)) return;
			untrack(() => {
				if (result.ok && result.data) {
					applyConfig(result.data);
					configLoaded = true;
				} else if (!result.ok) configError = result.error;
				configLoading = false;
			});
		});
		return () => {
			roundRequest.abort();
			roomsRequest.abort();
			configRequest.abort();
			seatsRequest.abort();
			facilityRequest.abort();
			copyRoundsRequest.abort();
		};
	});
</script>

<PageShell
	title="จัดห้องสอบ"
	description={round?.name ?? 'ตั้งค่าห้องสอบและจัดเลขที่นั่งสอบ'}
	backHref="/staff/academic/admission/{id}"
>
	{#snippet actions()}
		{#if canManageAdmission}
			<Button
				size="icon"
				variant="outline"
				onclick={loadRound}
				disabled={roundLoading}
				aria-label="โหลดชื่อรอบใหม่"
			>
				<RefreshCw class="h-4 w-4" />
			</Button>
		{/if}
	{/snippet}
	{#if roundLoading && roomsLoading && configLoading && !roundLoaded && !roomsLoaded && !configLoaded}
		<PageSkeleton variant="detail" />
	{:else if !canManageAdmission}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์จัดการห้องสอบ"
			description="หน้านี้ใช้สำหรับตั้งค่าห้องสอบและจัดเลขที่นั่ง ซึ่งต้องมีสิทธิ์จัดการงานรับสมัคร"
		/>
	{:else}
		{#if roundError}
			<PageState
				variant="error"
				title="โหลดชื่อรอบรับสมัครไม่สำเร็จ"
				description={roundError}
				actionLabel="ลองอีกครั้ง"
				onaction={loadRound}
			/>
		{:else if roundLoading && roundLoaded}
			<RegionUpdatingState class="static" label="กำลังอัปเดตรอบรับสมัคร..." />
		{/if}
		<!-- Tabs -->
		<div class="border-b">
			<nav class="flex gap-1">
				<button
					class="flex items-center gap-2 border-b-2 px-4 py-2 text-sm font-medium transition-colors
						{activeTab === 'setup'
						? 'border-primary text-primary'
						: 'border-transparent text-muted-foreground hover:text-foreground'}"
					onclick={() => (activeTab = 'setup')}
				>
					<Settings class="h-4 w-4" /> ตั้งค่าห้องสอบ
				</button>
				<button
					class="flex items-center gap-2 border-b-2 px-4 py-2 text-sm font-medium transition-colors
						{activeTab === 'seats'
						? 'border-primary text-primary'
						: 'border-transparent text-muted-foreground hover:text-foreground'}"
					onclick={() => {
						activeTab = 'seats';
						if (!seatsLoaded && !seatsLoading) void loadSeats();
					}}
				>
					<ClipboardList class="h-4 w-4" /> ผลจัดที่นั่ง
					{#if totalAssigned > 0}
						<Badge variant="secondary">{totalAssigned}</Badge>
					{/if}
				</button>
				{#if activeTab === 'seats'}
					<Button
						size="icon"
						variant="outline"
						onclick={loadSeats}
						disabled={seatsLoading}
						aria-label="โหลดที่นั่งใหม่"
					>
						<RefreshCw class="h-4 w-4" />
					</Button>
				{/if}
			</nav>
		</div>

		<!-- ===== Tab: Setup ===== -->
		{#if activeTab === 'setup'}
			<div class="grid grid-cols-1 gap-4 lg:grid-cols-3">
				<!-- Left: Room list -->
				<div class="space-y-3 lg:col-span-2" aria-busy={roomsLoading}>
					{#if roomsLoading && !roomsLoaded}
						<PageSkeleton variant="table" rows={4} columns={5} />
					{:else if roomsError && !roomsLoaded}
						<PageState
							variant="error"
							title="โหลดห้องสอบไม่สำเร็จ"
							description={roomsError}
							actionLabel="ลองอีกครั้ง"
							onaction={loadRooms}
						/>
					{:else}
						{#if roomsLoading}<RegionUpdatingState
								class="static"
								label="กำลังอัปเดตห้องสอบ..."
							/>{/if}
						{#if roomsError}
							<p role="alert" class="text-sm text-destructive">
								{roomsError}
								<Button size="sm" variant="outline" onclick={loadRooms}>ลองใหม่</Button>
							</p>
						{/if}
						<div class="flex items-center justify-between">
							<p class="text-muted-foreground text-sm">
								{examRooms.length} ห้อง · ความจุรวม <strong>{totalCapacity}</strong> ที่นั่ง
								{#if totalAssigned > 0}· จัดแล้ว <strong>{totalAssigned}</strong> คน{/if}
							</p>
							<div class="flex gap-2">
								<Button
									size="icon"
									variant="outline"
									onclick={loadRooms}
									disabled={roomsLoading}
									aria-label="โหลดห้องสอบใหม่"
								>
									<RefreshCw class="h-4 w-4" />
								</Button>
								<Button
									size="sm"
									onclick={() => {
										showAddRoomDialog = true;
										if (addRoomMode === 'facility') void loadFacilityRooms();
									}}
								>
									<Plus class="mr-1.5 h-4 w-4" /> เพิ่มห้อง
								</Button>
							</div>
						</div>

						{#if examRooms.length === 0}
							<div class="text-muted-foreground rounded-lg border border-dashed py-12 text-center">
								<Building2 class="mx-auto mb-2 h-8 w-8 opacity-30" />
								<p class="text-sm">ยังไม่มีห้องสอบ กด "เพิ่มห้อง" เพื่อเริ่มต้น</p>
							</div>
						{:else}
							<Card.Root>
								<Table.Root>
									<Table.Header>
										<Table.Row>
											<Table.Head>ห้องสอบ</Table.Head>
											<Table.Head>อาคาร</Table.Head>
											<Table.Head class="w-28 text-center">ความจุ</Table.Head>
											<Table.Head class="w-24 text-center">จัดแล้ว</Table.Head>
											<Table.Head class="w-10"></Table.Head>
										</Table.Row>
									</Table.Header>
									<Table.Body>
										{#each examRooms as room (room.id)}
											<Table.Row>
												<Table.Cell class="font-medium">{room.roomName}</Table.Cell>
												<Table.Cell class="text-muted-foreground text-sm"
													>{room.buildingName ?? '—'}</Table.Cell
												>
												<Table.Cell class="text-center">
													{#if editingCapacityId === room.id}
														<div class="flex items-center justify-center gap-1">
															<Input
																type="number"
																min="1"
																class="h-7 w-16 text-center text-xs"
																bind:value={editingCapacityValue}
																onkeydown={(e) => e.key === 'Enter' && saveCapacity(room.id)}
															/>
															<Button
																size="icon"
																variant="ghost"
																class="h-7 w-7"
																onclick={() => saveCapacity(room.id)}
															>
																<Check class="h-3.5 w-3.5 text-green-600" />
															</Button>
														</div>
													{:else}
														<button
															class="hover:text-primary hover:underline underline-offset-2"
															onclick={() => startEditCapacity(room)}
														>
															{room.capacity}
														</button>
													{/if}
												</Table.Cell>
												<Table.Cell class="text-center">
													{#if room.assignedCount > 0}
														<Badge variant="secondary">{room.assignedCount}</Badge>
													{:else}
														<span class="text-muted-foreground text-sm">—</span>
													{/if}
												</Table.Cell>
												<Table.Cell>
													<Button
														variant="ghost"
														size="icon"
														class="h-7 w-7 text-red-400 hover:text-red-600"
														onclick={() => handleRemoveRoom(room.id)}
													>
														<Trash2 class="h-3.5 w-3.5" />
													</Button>
												</Table.Cell>
											</Table.Row>
										{/each}
									</Table.Body>
								</Table.Root>
							</Card.Root>
						{/if}
					{/if}
				</div>

				<!-- Right: Config + Actions -->
				<div class="space-y-3" aria-busy={configLoading}>
					<!-- Config -->
					{#if configLoading && !configLoaded}
						<PageSkeleton variant="detail" />
					{:else if configError && !configLoaded}
						<PageState
							variant="error"
							title="โหลดการตั้งค่าที่นั่งไม่สำเร็จ"
							description={configError}
							actionLabel="ลองอีกครั้ง"
							onaction={loadConfig}
						/>
					{:else}
						{#if configLoading}<RegionUpdatingState
								class="static"
								label="กำลังอัปเดตการตั้งค่า..."
							/>{/if}
						{#if configError}
							<p role="alert" class="text-sm text-destructive">
								{configError}
								<Button size="sm" variant="outline" onclick={loadConfig}>ลองใหม่</Button>
							</p>
						{/if}
						<Card.Root>
							<Card.Header class="pb-3">
								<div class="flex items-center justify-between">
									<Card.Title class="flex items-center gap-2 text-sm">
										<Settings class="h-4 w-4" /> ตั้งค่าการจัดที่นั่ง
									</Card.Title>
									<Button
										size="icon"
										variant="ghost"
										onclick={loadConfig}
										disabled={configLoading}
										aria-label="โหลดการตั้งค่าใหม่"
									>
										<RefreshCw class="h-4 w-4" />
									</Button>
								</div>
							</Card.Header>
							<Card.Content class="space-y-3">
								<div class="space-y-1.5">
									<p class="text-sm font-medium">รูปแบบเลขประจำตัวสอบ</p>
									<Select.Root
										type="single"
										value={examConfig.examIdType ?? 'application_number'}
										onValueChange={(v) =>
											(examConfig = { ...examConfig, examIdType: v as ExamConfig['examIdType'] })}
									>
										<Select.Trigger class="w-full">
											{examIdTypeLabel[examConfig.examIdType ?? 'application_number']}
										</Select.Trigger>
										<Select.Content>
											<Select.Item value="application_number">เลขใบสมัคร</Select.Item>
											<Select.Item value="sequential">ลำดับต่อเนื่อง (1, 2, 3…)</Select.Item>
											<Select.Item value="custom_prefix">กำหนด Prefix เอง</Select.Item>
										</Select.Content>
									</Select.Root>
								</div>

								{#if examConfig.examIdType === 'custom_prefix'}
									<div class="space-y-1.5">
										<p class="text-sm font-medium">Prefix</p>
										<Input
											bind:value={examConfig.examIdPrefix}
											placeholder="เช่น 6801 → 68010001…"
										/>
									</div>
								{/if}

								<div class="space-y-1.5">
									<p class="text-sm font-medium">ลำดับรายชื่อ</p>
									<Select.Root
										type="single"
										value={examConfig.sortOrder ?? 'by_application'}
										onValueChange={(v) =>
											(examConfig = { ...examConfig, sortOrder: v as ExamConfig['sortOrder'] })}
									>
										<Select.Trigger class="w-full">
											{sortOrderLabel[examConfig.sortOrder ?? 'by_application']}
										</Select.Trigger>
										<Select.Content>
											<Select.Item value="by_application">ตามลำดับการสมัคร</Select.Item>
											<Select.Item value="by_track">แบ่งตามสาย แล้วเรียงการสมัคร</Select.Item>
											<Select.Item value="random">สุ่ม</Select.Item>
										</Select.Content>
									</Select.Root>
								</div>

								<Button
									size="sm"
									variant="outline"
									class="w-full"
									onclick={handleSaveConfig}
									disabled={savingConfig}
								>
									{#if savingConfig}<Loader2 class="mr-1.5 h-3.5 w-3.5 animate-spin" />{/if}
									บันทึก config
								</Button>
							</Card.Content>
						</Card.Root>
					{/if}

					<!-- Copy from round -->
					<Card.Root>
						<Card.Header class="pb-3">
							<Card.Title class="flex items-center gap-2 text-sm">
								<Copy class="h-4 w-4" /> Copy จากรอบอื่น
							</Card.Title>
						</Card.Header>
						<Card.Content class="space-y-2">
							<Select.Root
								type="single"
								value={copyFromRoundId}
								onValueChange={(v) => (copyFromRoundId = v)}
								onOpenChange={(open) => open && void loadCopyRounds()}
							>
								<Select.Trigger class="w-full" disabled={!round}>
									{allRounds.find((r) => r.id === copyFromRoundId)?.name ?? '— เลือกรอบ —'}
								</Select.Trigger>
								<Select.Content>
									{#if copyRoundsLoading}<p class="px-2 py-1 text-xs">กำลังโหลดรอบต้นทาง...</p>{/if}
									{#each allRounds as r (r.id)}
										<Select.Item value={r.id}>{r.name}</Select.Item>
									{/each}
								</Select.Content>
							</Select.Root>
							{#if copyRoundsError}
								<p role="alert" class="text-xs text-destructive">
									{copyRoundsError}
									<Button size="sm" variant="outline" onclick={loadCopyRounds}>ลองใหม่</Button>
								</p>
							{/if}
							<Button
								size="sm"
								variant="outline"
								class="w-full"
								onclick={handleCopyFromRound}
								disabled={copying || !copyFromRoundId}
							>
								{#if copying}<Loader2 class="mr-1.5 h-3.5 w-3.5 animate-spin" />{/if}
								Copy ห้องสอบ (แทนที่ของเดิม)
							</Button>
						</Card.Content>
					</Card.Root>

					<!-- Assign buttons -->
					<div class="flex gap-2">
						<Button
							class="flex-1"
							size="lg"
							disabled={!configLoaded || !roomsLoaded || examRooms.length === 0}
							onclick={() => {
								assignMode = 'full';
								showAssignDialog = true;
							}}
						>
							<RefreshCw class="mr-2 h-4 w-4" />
							จัดใหม่ทั้งหมด
						</Button>
						{#if totalAssigned > 0}
							<Button
								class="flex-1"
								size="lg"
								variant="outline"
								onclick={() => {
									assignMode = 'append';
									showAssignDialog = true;
								}}
							>
								<Plus class="mr-2 h-4 w-4" />
								เพิ่มคนใหม่
							</Button>
						{/if}
					</div>
					{#if totalAssigned > 0}
						<p class="text-center text-xs text-muted-foreground">จัดแล้ว {totalAssigned} คน</p>
					{/if}
				</div>
			</div>

			<!-- ===== Tab: Seats ===== -->
		{:else}
			<div class="space-y-4" aria-busy={seatsLoading}>
				{#if seatsLoading && !seatsLoaded}
					<PageSkeleton variant="table" rows={5} columns={5} />
				{:else if seatsError && !seatsLoaded}
					<PageState
						variant="error"
						title="โหลดผลจัดที่นั่งไม่สำเร็จ"
						description={seatsError}
						actionLabel="ลองอีกครั้ง"
						onaction={loadSeats}
					/>
				{:else}
					{#if seatsLoading}<RegionUpdatingState
							class="static"
							label="กำลังอัปเดตผลจัดที่นั่ง..."
						/>{/if}
					{#if seatsError}
						<p role="alert" class="text-sm text-destructive">
							{seatsError}
							<Button size="sm" variant="outline" onclick={loadSeats}>ลองใหม่</Button>
						</p>
					{/if}
					{#if seatGroups.length === 0}
						<div class="text-muted-foreground rounded-lg border border-dashed py-16 text-center">
							<ClipboardList class="mx-auto mb-2 h-8 w-8 opacity-30" />
							<p class="text-sm">ยังไม่มีผลจัดที่นั่ง</p>
							<p class="text-xs mt-1">กลับแท็บ "ตั้งค่า" แล้วกด "จัดที่นั่งสอบ"</p>
						</div>
					{:else}
						<div class="flex items-center justify-between">
							<p class="text-muted-foreground text-sm">
								รวม {seatGroups.reduce((s, g) => s + g.seats.length, 0)} คน ใน {seatGroups.length} ห้อง
							</p>
							<div class="flex gap-1.5">
								<Button size="sm" variant="outline" onclick={printAllAdmitCards}>
									<FileDown class="mr-1.5 h-4 w-4" /> พิมพ์บัตรสอบ
								</Button>
								<Button size="sm" variant="outline" onclick={downloadAllXlsx}>
									<FileSpreadsheet class="mr-1.5 h-4 w-4" /> XLSX ทุกห้อง
								</Button>
							</div>
						</div>

						{#each seatGroups as group (group.examRoomId)}
							<Card.Root>
								<Card.Header class="pb-2">
									<div class="flex items-center justify-between">
										<div>
											<Card.Title>{group.roomName}</Card.Title>
											{#if group.buildingName}
												<Card.Description>{group.buildingName}</Card.Description>
											{/if}
										</div>
										<div class="flex items-center gap-3">
											<Badge variant="outline">{group.seats.length}/{group.capacity}</Badge>
											<div class="flex gap-1.5">
												<Button size="sm" variant="outline" onclick={() => printRoom(group)}>
													<FileDown class="mr-1 h-3.5 w-3.5" /> พิมพ์รายชื่อ
												</Button>
												<Button size="sm" variant="outline" onclick={() => downloadRoomXlsx(group)}>
													<FileSpreadsheet class="mr-1 h-3.5 w-3.5" /> XLSX
												</Button>
											</div>
										</div>
									</div>
								</Card.Header>
								<Card.Content class="pt-0">
									<Table.Root>
										<Table.Header>
											<Table.Row>
												<Table.Head class="w-36">เลขประจำตัวสอบ</Table.Head>
												<Table.Head class="w-16 text-center">ที่นั่ง</Table.Head>
												<Table.Head>ชื่อ-นามสกุล</Table.Head>
												<Table.Head>เลขบัตรประชาชน</Table.Head>
												<Table.Head>สาย</Table.Head>
											</Table.Row>
										</Table.Header>
										<Table.Body>
											{#each group.seats as seat (seat.applicationId)}
												<Table.Row>
													<Table.Cell class="font-mono text-sm"
														>{seat.examId ?? seat.applicationNumber ?? '—'}</Table.Cell
													>
													<Table.Cell class="text-center font-medium">{seat.seatNumber}</Table.Cell>
													<Table.Cell>{seat.fullName}</Table.Cell>
													<Table.Cell class="font-mono text-sm">{seat.nationalId}</Table.Cell>
													<Table.Cell>
														{#if seat.trackName}
															<Badge variant="secondary">{seat.trackName}</Badge>
														{:else}—{/if}
													</Table.Cell>
												</Table.Row>
											{/each}
										</Table.Body>
									</Table.Root>
								</Card.Content>
							</Card.Root>
						{/each}
					{/if}
				{/if}
			</div>
		{/if}
	{/if}
</PageShell>

<!-- ===== Dialog: Add Room ===== -->
<Dialog.Root bind:open={showAddRoomDialog}>
	<Dialog.Content class="sm:max-w-md">
		<Dialog.Header>
			<Dialog.Title>เพิ่มห้องสอบ</Dialog.Title>
		</Dialog.Header>

		<div class="space-y-4 py-2">
			<div class="flex gap-2">
				<Button
					size="sm"
					variant={addRoomMode === 'facility' ? 'default' : 'outline'}
					onclick={() => {
						addRoomMode = 'facility';
						void loadFacilityRooms();
					}}
				>
					เลือกจากอาคาร
				</Button>
				<Button
					size="sm"
					variant={addRoomMode === 'custom' ? 'default' : 'outline'}
					onclick={() => (addRoomMode = 'custom')}
				>
					เพิ่มเอง
				</Button>
			</div>

			{#if addRoomMode === 'facility'}
				<div class="space-y-1.5">
					<p class="text-sm font-medium">เลือกห้อง</p>
					{#if facilityLoading}
						<RegionUpdatingState class="static" label="กำลังโหลดห้องอาคาร..." />
					{:else if facilityError}
						<p role="alert" class="text-sm text-destructive">
							{facilityError}
							<Button size="sm" variant="outline" onclick={loadFacilityRooms}>ลองใหม่</Button>
						</p>
					{/if}
					<Select.Root
						type="single"
						value={selectedFacilityRoomId}
						onValueChange={(v) => (selectedFacilityRoomId = v)}
					>
						<Select.Trigger class="w-full" disabled={!facilityLoaded}>
							{#if selectedFacilityRoomId}
								{facilityRooms.find((r) => r.id === selectedFacilityRoomId)?.name_th ?? '—'}
							{:else}
								— เลือกห้อง —
							{/if}
						</Select.Trigger>
						<Select.Content>
							{#each facilityRooms as room (room.id)}
								<Select.Item value={room.id}>
									{room.name_th}{room.code ? ` (${room.code})` : ''} · {room.capacity} คน
								</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
			{:else}
				<div class="grid grid-cols-2 gap-3">
					<div class="space-y-1.5">
						<p class="text-sm font-medium">ชื่อห้องสอบ</p>
						<Input bind:value={customRoomName} placeholder="เช่น ห้องประชุมใหญ่" />
					</div>
					<div class="space-y-1.5">
						<p class="text-sm font-medium">ความจุ (คน)</p>
						<Input type="number" min="1" bind:value={customRoomCapacity} />
					</div>
				</div>
			{/if}
		</div>

		<Dialog.Footer>
			<Button variant="outline" onclick={() => (showAddRoomDialog = false)}>ยกเลิก</Button>
			<Button
				onclick={handleAddRoom}
				disabled={addingRoom || (addRoomMode === 'facility' && !facilityLoaded)}
			>
				{#if addingRoom}<Loader2 class="mr-1.5 h-4 w-4 animate-spin" />{/if}
				เพิ่มห้อง
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<!-- ===== Dialog: Confirm Assign ===== -->
<Dialog.Root bind:open={showAssignDialog}>
	<Dialog.Content class="sm:max-w-sm">
		<Dialog.Header>
			<Dialog.Title>{assignMode === 'append' ? 'เพิ่มคนใหม่' : 'จัดที่นั่งสอบ'}</Dialog.Title>
			<Dialog.Description>
				{#if assignMode === 'append'}
					เพิ่มที่นั่งเฉพาะคนที่ยังไม่มีที่นั่ง — ผลเดิม {totalAssigned} คนไม่เปลี่ยน
				{:else}
					จะจัดที่นั่งสอบให้ผู้สมัครทุกคนใหม่ตั้งแต่ต้น
					{#if totalAssigned > 0}
						<br /><span class="text-orange-600">⚠ จะล้างผลเดิม {totalAssigned} คน แล้วจัดใหม่</span>
					{/if}
				{/if}
			</Dialog.Description>
		</Dialog.Header>
		<div class="rounded-md bg-muted px-4 py-3 text-sm space-y-1">
			<p>
				รูปแบบเลขประจำตัวสอบ: <strong
					>{examIdTypeLabel[examConfig.examIdType ?? 'application_number']}</strong
				>
			</p>
			<p>
				ลำดับรายชื่อ: <strong>{sortOrderLabel[examConfig.sortOrder ?? 'by_application']}</strong>
			</p>
			<p>
				ห้องสอบ: <strong>{examRooms.length} ห้อง</strong> · ความจุรวม
				<strong>{totalCapacity} ที่นั่ง</strong>
			</p>
		</div>
		<Dialog.Footer>
			<Button variant="outline" onclick={() => (showAssignDialog = false)}>ยกเลิก</Button>
			<Button onclick={handleAssignSeats} disabled={assigning}>
				{#if assigning}<Loader2 class="mr-1.5 h-4 w-4 animate-spin" />{/if}
				ยืนยันจัดที่นั่ง
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
