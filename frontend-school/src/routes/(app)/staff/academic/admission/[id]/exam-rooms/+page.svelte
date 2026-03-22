<script lang="ts">
	import { onMount } from 'svelte';
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
	} from '$lib/api/admission';
	import { listRooms, type Room } from '$lib/api/facility';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Badge } from '$lib/components/ui/badge';
	import * as Card from '$lib/components/ui/card';
	import * as Table from '$lib/components/ui/table';
	import * as Select from '$lib/components/ui/select';
	import * as Dialog from '$lib/components/ui/dialog';
	import { toast } from 'svelte-sonner';
	import {
		ArrowLeft,
		Building2,
		Plus,
		Trash2,
		Loader2,
		ClipboardList,
		Printer,
		Copy,
		Settings,
		RefreshCw,
		Pencil,
		Check
	} from 'lucide-svelte';

	let { data, params }: PageProps = $props();
	let id = $derived(params.id);

	let round: AdmissionRound | null = $state(null);
	let allRounds: AdmissionRound[] = $state([]);
	let examRooms: ExamRoom[] = $state([]);
	let totalCapacity = $state(0);
	let totalAssigned = $state(0);
	let facilityRooms: Room[] = $state([]);
	let examConfig: ExamConfig = $state({
		examIdType: 'application_number',
		sortOrder: 'by_application'
	});
	let seatGroups: ExamRoomGroup[] = $state([]);

	let activeTab = $state<'setup' | 'seats'>('setup');
	let loading = $state(true);
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

	async function loadAll() {
		if (!id) return;
		loading = true;
		try {
			const [roundData, examRoomsData, configData, facilityData, allRoundsData] =
				await Promise.all([
					getRound(id),
					listExamRooms(id),
					getExamConfig(id),
					listRooms({}),
					listRounds()
				]);
			round = roundData;
			examRooms = examRoomsData.rooms;
			totalCapacity = examRoomsData.totalCapacity;
			totalAssigned = examRoomsData.totalAssigned;
			examConfig = {
				examIdType: configData.examIdType ?? 'application_number',
				examIdPrefix: configData.examIdPrefix ?? '',
				sortOrder: configData.sortOrder ?? 'by_application'
			};
			facilityRooms = facilityData.data.filter((r: Room) => r.status === 'ACTIVE');
			allRounds = allRoundsData.filter((r: AdmissionRound) => r.id !== id);
		} catch {
			toast.error('ไม่สามารถโหลดข้อมูลได้');
		} finally {
			loading = false;
		}
	}

	async function loadSeats() {
		if (!id) return;
		try {
			const result = await getExamSeats(id);
			seatGroups = Array.isArray(result) ? result : [];
		} catch {
			toast.error('ไม่สามารถโหลดผลจัดที่นั่งได้');
			seatGroups = [];
		}
	}

	async function refreshRooms() {
		if (!id) return;
		const res = await listExamRooms(id);
		examRooms = res.rooms;
		totalCapacity = res.totalCapacity;
		totalAssigned = res.totalAssigned;
	}

	async function handleAddRoom() {
		if (!id) return;
		addingRoom = true;
		try {
			if (addRoomMode === 'facility') {
				if (!selectedFacilityRoomId) { toast.error('กรุณาเลือกห้อง'); return; }
				await addExamRoom(id, { roomId: selectedFacilityRoomId });
			} else {
				if (!customRoomName.trim()) { toast.error('กรุณาระบุชื่อห้อง'); return; }
				await addExamRoom(id, { customName: customRoomName.trim(), capacityOverride: customRoomCapacity });
			}
			toast.success('เพิ่มห้องสอบแล้ว');
			showAddRoomDialog = false;
			selectedFacilityRoomId = '';
			customRoomName = '';
			customRoomCapacity = 40;
			await refreshRooms();
		} catch {
			toast.error('ไม่สามารถเพิ่มห้องสอบได้');
		} finally {
			addingRoom = false;
		}
	}

	async function handleRemoveRoom(roomId: string) {
		if (!id || !confirm('ลบห้องสอบนี้?')) return;
		try {
			await removeExamRoom(id, roomId);
			toast.success('ลบห้องสอบแล้ว');
			await refreshRooms();
		} catch {
			toast.error('ไม่สามารถลบห้องสอบได้');
		}
	}

	function startEditCapacity(room: ExamRoom) {
		editingCapacityId = room.id;
		editingCapacityValue = room.capacity;
	}

	async function saveCapacity(roomId: string) {
		if (!id || editingCapacityValue < 1) return;
		try {
			await updateExamRoom(id, roomId, { capacityOverride: editingCapacityValue });
			toast.success('อัปเดตความจุแล้ว');
			editingCapacityId = null;
			await refreshRooms();
		} catch {
			toast.error('ไม่สามารถอัปเดตความจุได้');
		}
	}

	async function handleCopyFromRound() {
		if (!id || !copyFromRoundId) { toast.error('กรุณาเลือกรอบที่ต้องการ copy'); return; }
		copying = true;
		try {
			const result = await copyExamRoomsFromRound(id, copyFromRoundId);
			toast.success(result.message);
			copyFromRoundId = '';
			await refreshRooms();
		} catch {
			toast.error('ไม่สามารถ copy ห้องสอบได้');
		} finally {
			copying = false;
		}
	}

	async function handleSaveConfig() {
		if (!id) return;
		savingConfig = true;
		try {
			await updateExamConfig(id, examConfig);
			toast.success('บันทึก config แล้ว');
		} catch {
			toast.error('ไม่สามารถบันทึก config ได้');
		} finally {
			savingConfig = false;
		}
	}

	async function handleAssignSeats() {
		if (!id) return;
		assigning = true;
		try {
			const result = await assignExamSeats(id, {
				examIdType: examConfig.examIdType,
				examIdPrefix: examConfig.examIdPrefix,
				sortOrder: examConfig.sortOrder
			});
			toast.success(result.message);
			showAssignDialog = false;
			await Promise.all([loadSeats(), refreshRooms()]);
			activeTab = 'seats';
		} catch (e: unknown) {
			const err = e as { response?: { data?: { error?: string } } };
			toast.error(err?.response?.data?.error ?? 'ไม่สามารถจัดที่นั่งได้');
		} finally {
			assigning = false;
		}
	}

	function printRoom(group: ExamRoomGroup) {
		const w = window.open('', '_blank');
		if (!w) return;
		const rows = group.seats
			.map((s) => `<tr>
				<td>${s.examId ?? s.applicationNumber ?? ''}</td>
				<td style="text-align:center">${s.seatNumber}</td>
				<td>${s.fullName}</td>
				<td>${s.nationalId}</td>
				<td>${s.trackName ?? ''}</td>
			</tr>`).join('');
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
			<script>window.onload=function(){window.print()}<\/script>
		</body></html>`);
		w.document.close();
	}

	function printAllAdmitCards() {
		const w = window.open('', '_blank');
		if (!w) return;
		const examDate = round?.examDate
			? new Date(round.examDate).toLocaleDateString('th-TH', { year: 'numeric', month: 'long', day: 'numeric' })
			: '-';
		const cards = seatGroups.flatMap((g) => g.seats.map((s) => `
			<div style="border:2px solid #333;padding:16px;margin:8px;width:280px;display:inline-block;vertical-align:top;font-size:13px;font-family:'Sarabun',sans-serif">
				<div style="font-weight:bold;font-size:15px;margin-bottom:10px">${round?.name ?? ''}</div>
				<table style="width:100%;font-size:13px">
					<tr><td style="color:#555;padding:2px 0">ชื่อ-นามสกุล</td><td style="font-weight:600">${s.fullName}</td></tr>
					<tr><td style="color:#555;padding:2px 0">เลขประจำตัวสอบ</td><td style="font-weight:700;color:#1d4ed8;font-size:15px">${s.examId ?? s.applicationNumber ?? ''}</td></tr>
					<tr><td style="color:#555;padding:2px 0">ห้องสอบ</td><td style="font-weight:600">${g.roomName}</td></tr>
					<tr><td style="color:#555;padding:2px 0">เลขที่นั่ง</td><td style="font-weight:600">${s.seatNumber}</td></tr>
					<tr><td style="color:#555;padding:2px 0">วันสอบ</td><td>${examDate}</td></tr>
				</table>
			</div>`)).join('');
		w.document.write(`<!DOCTYPE html><html><head>
			<meta charset="utf-8"><title>บัตรสอบ</title>
			<style>body{padding:16px}@media print{@page{margin:8mm}}</style>
		</head><body>${cards}
			<script>window.onload=function(){window.print()}<\/script>
		</body></html>`);
		w.document.close();
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

	onMount(() => { loadAll(); });
</script>

<div class="space-y-4 p-4">
	<!-- Header -->
	<div class="flex items-center gap-3">
		<Button variant="ghost" size="icon" href="/staff/academic/admission/{id}">
			<ArrowLeft class="h-4 w-4" />
		</Button>
		<div>
			<h1 class="text-xl font-bold">จัดห้องสอบ</h1>
			{#if round}
				<p class="text-muted-foreground text-sm">{round.name}</p>
			{/if}
		</div>
	</div>

	{#if loading}
		<div class="flex items-center justify-center py-20">
			<Loader2 class="text-muted-foreground h-7 w-7 animate-spin" />
		</div>
	{:else}
		<!-- Tabs -->
		<div class="border-b">
			<nav class="flex gap-1">
				<button
					class="flex items-center gap-2 border-b-2 px-4 py-2 text-sm font-medium transition-colors
						{activeTab === 'setup' ? 'border-primary text-primary' : 'border-transparent text-muted-foreground hover:text-foreground'}"
					onclick={() => (activeTab = 'setup')}
				>
					<Settings class="h-4 w-4" /> ตั้งค่าห้องสอบ
				</button>
				<button
					class="flex items-center gap-2 border-b-2 px-4 py-2 text-sm font-medium transition-colors
						{activeTab === 'seats' ? 'border-primary text-primary' : 'border-transparent text-muted-foreground hover:text-foreground'}"
					onclick={() => { activeTab = 'seats'; if (seatGroups.length === 0) loadSeats(); }}
				>
					<ClipboardList class="h-4 w-4" /> ผลจัดที่นั่ง
					{#if totalAssigned > 0}
						<Badge variant="secondary">{totalAssigned}</Badge>
					{/if}
				</button>
			</nav>
		</div>

		<!-- ===== Tab: Setup ===== -->
		{#if activeTab === 'setup'}
			<div class="grid grid-cols-1 gap-4 lg:grid-cols-3">

				<!-- Left: Room list -->
				<div class="space-y-3 lg:col-span-2">
					<div class="flex items-center justify-between">
						<p class="text-muted-foreground text-sm">
							{examRooms.length} ห้อง · ความจุรวม <strong>{totalCapacity}</strong> ที่นั่ง
							{#if totalAssigned > 0}· จัดแล้ว <strong>{totalAssigned}</strong> คน{/if}
						</p>
						<Button size="sm" onclick={() => (showAddRoomDialog = true)}>
							<Plus class="mr-1.5 h-4 w-4" /> เพิ่มห้อง
						</Button>
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
											<Table.Cell class="text-muted-foreground text-sm">{room.buildingName ?? '—'}</Table.Cell>
											<Table.Cell class="text-center">
												{#if editingCapacityId === room.id}
													<div class="flex items-center justify-center gap-1">
														<Input type="number" min="1" class="h-7 w-16 text-center text-xs"
															bind:value={editingCapacityValue}
															onkeydown={(e) => e.key === 'Enter' && saveCapacity(room.id)} />
														<Button size="icon" variant="ghost" class="h-7 w-7"
															onclick={() => saveCapacity(room.id)}>
															<Check class="h-3.5 w-3.5 text-green-600" />
														</Button>
													</div>
												{:else}
													<button class="hover:text-primary hover:underline underline-offset-2"
														onclick={() => startEditCapacity(room)}>
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
												<Button variant="ghost" size="icon" class="h-7 w-7 text-red-400 hover:text-red-600"
													onclick={() => handleRemoveRoom(room.id)}>
													<Trash2 class="h-3.5 w-3.5" />
												</Button>
											</Table.Cell>
										</Table.Row>
									{/each}
								</Table.Body>
							</Table.Root>
						</Card.Root>
					{/if}
				</div>

				<!-- Right: Config + Actions -->
				<div class="space-y-3">
					<!-- Config -->
					<Card.Root>
						<Card.Header class="pb-3">
							<Card.Title class="flex items-center gap-2 text-sm">
								<Settings class="h-4 w-4" /> ตั้งค่าการจัดที่นั่ง
							</Card.Title>
						</Card.Header>
						<Card.Content class="space-y-3">
							<div class="space-y-1.5">
								<p class="text-sm font-medium">รูปแบบเลขประจำตัวสอบ</p>
								<Select.Root
									type="single"
									value={examConfig.examIdType ?? 'application_number'}
									onValueChange={(v) => (examConfig = { ...examConfig, examIdType: v as ExamConfig['examIdType'] })}
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
									<Input bind:value={examConfig.examIdPrefix} placeholder="เช่น 6801 → 68010001…" />
								</div>
							{/if}

							<div class="space-y-1.5">
								<p class="text-sm font-medium">ลำดับรายชื่อ</p>
								<Select.Root
									type="single"
									value={examConfig.sortOrder ?? 'by_application'}
									onValueChange={(v) => (examConfig = { ...examConfig, sortOrder: v as ExamConfig['sortOrder'] })}
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

							<Button size="sm" variant="outline" class="w-full" onclick={handleSaveConfig} disabled={savingConfig}>
								{#if savingConfig}<Loader2 class="mr-1.5 h-3.5 w-3.5 animate-spin" />{/if}
								บันทึก config
							</Button>
						</Card.Content>
					</Card.Root>

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
							>
								<Select.Trigger class="w-full">
									{allRounds.find((r) => r.id === copyFromRoundId)?.name ?? '— เลือกรอบ —'}
								</Select.Trigger>
								<Select.Content>
									{#each allRounds as r (r.id)}
										<Select.Item value={r.id}>{r.name}</Select.Item>
									{/each}
								</Select.Content>
							</Select.Root>
							<Button size="sm" variant="outline" class="w-full"
								onclick={handleCopyFromRound} disabled={copying || !copyFromRoundId}>
								{#if copying}<Loader2 class="mr-1.5 h-3.5 w-3.5 animate-spin" />{/if}
								Copy ห้องสอบ (แทนที่ของเดิม)
							</Button>
						</Card.Content>
					</Card.Root>

					<!-- Assign button -->
					<Button class="w-full" size="lg"
						disabled={examRooms.length === 0}
						onclick={() => (showAssignDialog = true)}>
						<RefreshCw class="mr-2 h-4 w-4" />
						จัดที่นั่งสอบ
					</Button>
					{#if totalAssigned > 0}
						<p class="text-center text-xs text-blue-600">จัดแล้ว {totalAssigned} คน · กดอีกครั้งเพื่อจัดใหม่</p>
					{/if}
				</div>
			</div>

		<!-- ===== Tab: Seats ===== -->
		{:else}
			<div class="space-y-4">
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
						<Button size="sm" variant="outline" onclick={printAllAdmitCards}>
							<Printer class="mr-1.5 h-4 w-4" /> พิมพ์บัตรสอบทั้งหมด
						</Button>
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
										<Button size="sm" variant="outline" onclick={() => printRoom(group)}>
											<Printer class="mr-1.5 h-3.5 w-3.5" /> พิมพ์รายชื่อ
										</Button>
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
												<Table.Cell class="font-mono text-sm">{seat.examId ?? seat.applicationNumber ?? '—'}</Table.Cell>
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
			</div>
		{/if}
	{/if}
</div>

<!-- ===== Dialog: Add Room ===== -->
<Dialog.Root bind:open={showAddRoomDialog}>
	<Dialog.Content class="sm:max-w-md">
		<Dialog.Header>
			<Dialog.Title>เพิ่มห้องสอบ</Dialog.Title>
		</Dialog.Header>

		<div class="space-y-4 py-2">
			<div class="flex gap-2">
				<Button size="sm" variant={addRoomMode === 'facility' ? 'default' : 'outline'}
					onclick={() => (addRoomMode = 'facility')}>
					เลือกจากอาคาร
				</Button>
				<Button size="sm" variant={addRoomMode === 'custom' ? 'default' : 'outline'}
					onclick={() => (addRoomMode = 'custom')}>
					เพิ่มเอง
				</Button>
			</div>

			{#if addRoomMode === 'facility'}
				<div class="space-y-1.5">
					<p class="text-sm font-medium">เลือกห้อง</p>
					<Select.Root type="single" value={selectedFacilityRoomId}
						onValueChange={(v) => (selectedFacilityRoomId = v)}>
						<Select.Trigger class="w-full">
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
			<Button onclick={handleAddRoom} disabled={addingRoom}>
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
			<Dialog.Title>จัดที่นั่งสอบ</Dialog.Title>
			<Dialog.Description>
				จะจัดที่นั่งสอบให้ผู้สมัครทั้งหมดในรอบนี้
				{#if totalAssigned > 0}
					<br /><span class="text-orange-600">⚠ จะล้างผลเดิม {totalAssigned} คน แล้วจัดใหม่</span>
				{/if}
			</Dialog.Description>
		</Dialog.Header>
		<div class="rounded-md bg-muted px-4 py-3 text-sm space-y-1">
			<p>รูปแบบเลขประจำตัวสอบ: <strong>{examIdTypeLabel[examConfig.examIdType ?? 'application_number']}</strong></p>
			<p>ลำดับรายชื่อ: <strong>{sortOrderLabel[examConfig.sortOrder ?? 'by_application']}</strong></p>
			<p>ห้องสอบ: <strong>{examRooms.length} ห้อง</strong> · ความจุรวม <strong>{totalCapacity} ที่นั่ง</strong></p>
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
