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
	import { listRooms, listBuildings, type Room, type Building } from '$lib/api/facility';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import * as Card from '$lib/components/ui/card';
	import * as Table from '$lib/components/ui/table';
	import * as Select from '$lib/components/ui/select';
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
		RefreshCw
	} from 'lucide-svelte';

	let { data, params }: PageProps = $props();
	let id = $derived(params.id);

	let round: AdmissionRound | null = $state(null);
	let allRounds: AdmissionRound[] = $state([]);

	// ห้องสอบที่ configure ไว้
	let examRooms: ExamRoom[] = $state([]);
	let totalCapacity = $state(0);
	let totalAssigned = $state(0);

	// ห้องทั้งหมดจาก facility
	let facilityRooms: Room[] = $state([]);
	let buildings: Building[] = $state([]);

	// Config
	let examConfig: ExamConfig = $state({
		examIdType: 'application_number',
		sortOrder: 'by_application'
	});

	// ผลจัดที่นั่ง
	let seatGroups: ExamRoomGroup[] = $state([]);

	let activeTab = $state<'setup' | 'seats'>('setup');
	let loading = $state(true);
	let assigning = $state(false);
	let savingConfig = $state(false);

	// modal เพิ่มห้อง
	let showAddRoom = $state(false);
	let addRoomMode = $state<'facility' | 'custom'>('facility');
	let selectedFacilityRoomId = $state('');
	let customRoomName = $state('');
	let customRoomCapacity = $state(40);
	let addingRoom = $state(false);

	// Copy from round
	let copyFromRoundId = $state('');
	let copying = $state(false);

	// Edit capacity inline
	let editingCapacity: Record<string, number> = $state({});

	async function loadAll() {
		if (!id) return;
		loading = true;
		try {
			const [roundData, examRoomsData, configData, facilityData, buildingsData, allRoundsData] =
				await Promise.all([
					getRound(id),
					listExamRooms(id),
					getExamConfig(id),
					listRooms({ }),
					listBuildings(),
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
			facilityRooms = facilityData.data;
			buildings = buildingsData.data;
			allRounds = allRoundsData.filter((r: AdmissionRound) => r.id !== id);
		} catch (e) {
			toast.error('ไม่สามารถโหลดข้อมูลได้');
		} finally {
			loading = false;
		}
	}

	async function loadSeats() {
		if (!id) return;
		try {
			seatGroups = await getExamSeats(id);
		} catch (e) {
			toast.error('ไม่สามารถโหลดผลจัดที่นั่งได้');
		}
	}

	async function handleAddRoom() {
		if (!id) return;
		addingRoom = true;
		try {
			if (addRoomMode === 'facility') {
				if (!selectedFacilityRoomId) {
					toast.error('กรุณาเลือกห้อง');
					return;
				}
				await addExamRoom(id, { roomId: selectedFacilityRoomId });
			} else {
				if (!customRoomName.trim()) {
					toast.error('กรุณาระบุชื่อห้อง');
					return;
				}
				await addExamRoom(id, {
					customName: customRoomName.trim(),
					capacityOverride: customRoomCapacity
				});
			}
			toast.success('เพิ่มห้องสอบแล้ว');
			showAddRoom = false;
			selectedFacilityRoomId = '';
			customRoomName = '';
			customRoomCapacity = 40;
			const examRoomsData = await listExamRooms(id);
			examRooms = examRoomsData.rooms;
			totalCapacity = examRoomsData.totalCapacity;
			totalAssigned = examRoomsData.totalAssigned;
		} catch (e) {
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
			const examRoomsData = await listExamRooms(id);
			examRooms = examRoomsData.rooms;
			totalCapacity = examRoomsData.totalCapacity;
			totalAssigned = examRoomsData.totalAssigned;
		} catch (e) {
			toast.error('ไม่สามารถลบห้องสอบได้');
		}
	}

	async function handleUpdateCapacity(roomId: string) {
		if (!id) return;
		const cap = editingCapacity[roomId];
		if (!cap || cap < 1) return;
		try {
			await updateExamRoom(id, roomId, { capacityOverride: cap });
			toast.success('อัปเดตความจุแล้ว');
			delete editingCapacity[roomId];
			const examRoomsData = await listExamRooms(id);
			examRooms = examRoomsData.rooms;
			totalCapacity = examRoomsData.totalCapacity;
		} catch (e) {
			toast.error('ไม่สามารถอัปเดตความจุได้');
		}
	}

	async function handleCopyFromRound() {
		if (!id || !copyFromRoundId) {
			toast.error('กรุณาเลือกรอบที่ต้องการ copy');
			return;
		}
		copying = true;
		try {
			const result = await copyExamRoomsFromRound(id, copyFromRoundId);
			toast.success(result.message);
			const examRoomsData = await listExamRooms(id);
			examRooms = examRoomsData.rooms;
			totalCapacity = examRoomsData.totalCapacity;
			totalAssigned = examRoomsData.totalAssigned;
			copyFromRoundId = '';
		} catch (e) {
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
		} catch (e) {
			toast.error('ไม่สามารถบันทึก config ได้');
		} finally {
			savingConfig = false;
		}
	}

	async function handleAssignSeats() {
		if (!id) return;
		if (examRooms.length === 0) {
			toast.error('กรุณาเพิ่มห้องสอบก่อน');
			return;
		}
		assigning = true;
		try {
			const result = await assignExamSeats(id, {
				examIdType: examConfig.examIdType,
				examIdPrefix: examConfig.examIdPrefix,
				sortOrder: examConfig.sortOrder
			});
			toast.success(result.message);
			await loadSeats();
			activeTab = 'seats';
			const examRoomsData = await listExamRooms(id);
			examRooms = examRoomsData.rooms;
			totalAssigned = examRoomsData.totalAssigned;
		} catch (e: unknown) {
			const err = e as { response?: { data?: { error?: string } } };
			toast.error(err?.response?.data?.error ?? 'ไม่สามารถจัดที่นั่งได้');
		} finally {
			assigning = false;
		}
	}

	function getBuildingName(room: Room): string {
		const building = buildings.find((b) => b.id === room.building_id);
		return building ? building.name_th : '';
	}

	function printRoom(group: ExamRoomGroup) {
		const w = window.open('', '_blank');
		if (!w) return;
		const rows = group.seats
			.map(
				(s) =>
					`<tr>
            <td style="padding:4px 8px;border:1px solid #ccc;text-align:center">${s.examId ?? s.applicationNumber ?? ''}</td>
            <td style="padding:4px 8px;border:1px solid #ccc;text-align:center">${s.seatNumber}</td>
            <td style="padding:4px 8px;border:1px solid #ccc">${s.fullName}</td>
            <td style="padding:4px 8px;border:1px solid #ccc;text-align:center">${s.nationalId}</td>
            <td style="padding:4px 8px;border:1px solid #ccc;text-align:center">${s.trackName ?? ''}</td>
          </tr>`
			)
			.join('');
		w.document.write(`<!DOCTYPE html><html><head>
      <meta charset="utf-8">
      <title>รายชื่อ ${group.roomName}</title>
      <style>body{font-family:'Sarabun',sans-serif;font-size:14px;padding:20px}
      table{border-collapse:collapse;width:100%}
      th{background:#f0f0f0;padding:6px 8px;border:1px solid #ccc}
      @media print{body{padding:0}}</style>
    </head><body>
      <h2 style="margin-bottom:4px">${round?.name ?? ''}</h2>
      <h3 style="margin-bottom:16px">ห้องสอบ: ${group.roomName}${group.buildingName ? ' (' + group.buildingName + ')' : ''} — ความจุ ${group.capacity} ที่นั่ง</h3>
      <table>
        <thead><tr>
          <th>เลขประจำตัวสอบ</th><th>ที่นั่ง</th><th>ชื่อ-นามสกุล</th><th>เลขบัตรประชาชน</th><th>สาย</th>
        </tr></thead>
        <tbody>${rows}</tbody>
      </table>
      <p style="margin-top:16px;color:#666">ทั้งหมด ${group.seats.length} คน</p>
      <script>window.onload=function(){window.print()}<\/script>
    </body></html>`);
		w.document.close();
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
          <div style="border:2px solid #333;padding:16px;margin:8px;width:280px;display:inline-block;vertical-align:top;font-size:13px">
            <div style="font-weight:bold;font-size:16px;margin-bottom:8px">${round?.name ?? ''}</div>
            <table style="width:100%">
              <tr><td style="color:#666">ชื่อ-นามสกุล</td><td style="font-weight:bold">${s.fullName}</td></tr>
              <tr><td style="color:#666">เลขประจำตัวสอบ</td><td style="font-weight:bold;color:#1d4ed8">${s.examId ?? s.applicationNumber ?? ''}</td></tr>
              <tr><td style="color:#666">ห้องสอบ</td><td style="font-weight:bold">${g.roomName}</td></tr>
              <tr><td style="color:#666">ที่นั่ง</td><td style="font-weight:bold">${s.seatNumber}</td></tr>
              <tr><td style="color:#666">วันสอบ</td><td>${examDate}</td></tr>
            </table>
          </div>`
				)
			)
			.join('');
		w.document.write(`<!DOCTYPE html><html><head>
      <meta charset="utf-8"><title>บัตรสอบ</title>
      <style>body{font-family:'Sarabun',sans-serif;padding:16px}
      @media print{@page{margin:10mm}}</style>
    </head><body>${cards}
      <script>window.onload=function(){window.print()}<\/script>
    </body></html>`);
		w.document.close();
	}

	onMount(() => {
		loadAll();
	});
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
		<div class="flex items-center justify-center py-16">
			<Loader2 class="text-muted-foreground h-8 w-8 animate-spin" />
		</div>
	{:else}
		<!-- Tabs -->
		<div class="border-b">
			<div class="flex gap-1">
				<button
					class="px-4 py-2 text-sm font-medium transition-colors {activeTab === 'setup'
						? 'border-b-2 border-blue-600 text-blue-600'
						: 'text-muted-foreground hover:text-foreground'}"
					onclick={() => (activeTab = 'setup')}
				>
					<span class="flex items-center gap-2"><Settings class="h-4 w-4" /> ตั้งค่าห้องสอบ</span>
				</button>
				<button
					class="px-4 py-2 text-sm font-medium transition-colors {activeTab === 'seats'
						? 'border-b-2 border-blue-600 text-blue-600'
						: 'text-muted-foreground hover:text-foreground'}"
					onclick={() => {
						activeTab = 'seats';
						if (seatGroups.length === 0) loadSeats();
					}}
				>
					<span class="flex items-center gap-2"
						><ClipboardList class="h-4 w-4" /> ผลจัดที่นั่ง
						{#if totalAssigned > 0}
							<span class="rounded-full bg-blue-100 px-2 py-0.5 text-xs text-blue-700"
								>{totalAssigned}</span
							>
						{/if}
					</span>
				</button>
			</div>
		</div>

		<!-- ========= Tab Setup ========= -->
		{#if activeTab === 'setup'}
			<div class="grid grid-cols-1 gap-4 lg:grid-cols-3">
				<!-- ซ้าย: รายการห้อง -->
				<div class="lg:col-span-2 space-y-3">
					<!-- Summary bar -->
					<div class="flex items-center justify-between rounded-lg border bg-blue-50 px-4 py-2.5">
						<span class="text-sm">
							ห้องสอบ <strong>{examRooms.length}</strong> ห้อง |
							ความจุรวม <strong>{totalCapacity}</strong> ที่นั่ง
							{#if totalAssigned > 0}
								| จัดแล้ว <strong>{totalAssigned}</strong> คน
							{/if}
						</span>
						<Button
							size="sm"
							onclick={() => (showAddRoom = !showAddRoom)}
							variant={showAddRoom ? 'secondary' : 'default'}
						>
							<Plus class="mr-1 h-4 w-4" /> เพิ่มห้อง
						</Button>
					</div>

					<!-- Add room panel -->
					{#if showAddRoom}
						<Card.Root class="border-blue-200">
							<Card.Content class="pt-4 space-y-3">
								<div class="flex gap-2">
									<button
										class="rounded px-3 py-1.5 text-sm font-medium transition-colors {addRoomMode === 'facility'
											? 'bg-blue-600 text-white'
											: 'bg-muted'}"
										onclick={() => (addRoomMode = 'facility')}>เลือกจากอาคาร</button
									>
									<button
										class="rounded px-3 py-1.5 text-sm font-medium transition-colors {addRoomMode === 'custom'
											? 'bg-blue-600 text-white'
											: 'bg-muted'}"
										onclick={() => (addRoomMode = 'custom')}>เพิ่มเอง</button
									>
								</div>

								{#if addRoomMode === 'facility'}
									<div>
										<label class="mb-1 block text-sm font-medium">เลือกห้อง</label>
										<select
											bind:value={selectedFacilityRoomId}
											class="w-full rounded-md border px-3 py-2 text-sm"
										>
											<option value="">-- เลือกห้อง --</option>
											{#each facilityRooms.filter((r) => r.status === 'ACTIVE') as room}
												<option value={room.id}>
													{room.name_th}{room.code ? ' (' + room.code + ')' : ''} — ความจุ {room.capacity}
													{#if room.building_name}
														| {room.building_name}
													{/if}
												</option>
											{/each}
										</select>
									</div>
								{:else}
									<div class="grid grid-cols-2 gap-3">
										<div>
											<label class="mb-1 block text-sm font-medium">ชื่อห้องสอบ</label>
											<Input bind:value={customRoomName} placeholder="เช่น ห้องประชุมใหญ่" />
										</div>
										<div>
											<label class="mb-1 block text-sm font-medium">ความจุ (คน)</label>
											<Input type="number" min="1" bind:value={customRoomCapacity} />
										</div>
									</div>
								{/if}

								<div class="flex gap-2">
									<Button size="sm" onclick={handleAddRoom} disabled={addingRoom}>
										{#if addingRoom}<Loader2 class="mr-1 h-4 w-4 animate-spin" />{/if}
										เพิ่มห้อง
									</Button>
									<Button size="sm" variant="ghost" onclick={() => (showAddRoom = false)}>ยกเลิก</Button>
								</div>
							</Card.Content>
						</Card.Root>
					{/if}

					<!-- Room list -->
					{#if examRooms.length === 0}
						<div class="text-muted-foreground rounded-lg border border-dashed py-10 text-center text-sm">
							<Building2 class="mx-auto mb-2 h-8 w-8 opacity-40" />
							ยังไม่มีห้องสอบ กด "เพิ่มห้อง" เพื่อเริ่มต้น
						</div>
					{:else}
						<div class="overflow-hidden rounded-lg border">
							<Table.Root>
								<Table.Header>
									<Table.Row>
										<Table.Head>ห้อง</Table.Head>
										<Table.Head>อาคาร</Table.Head>
										<Table.Head class="text-center">ความจุ</Table.Head>
										<Table.Head class="text-center">จัดแล้ว</Table.Head>
										<Table.Head></Table.Head>
									</Table.Row>
								</Table.Header>
								<Table.Body>
									{#each examRooms as room}
										<Table.Row>
											<Table.Cell class="font-medium">{room.roomName}</Table.Cell>
											<Table.Cell class="text-muted-foreground text-sm">{room.buildingName ?? '-'}</Table.Cell>
											<Table.Cell class="text-center">
												{#if editingCapacity[room.id] !== undefined}
													<div class="flex items-center gap-1 justify-center">
														<Input
															type="number"
															min="1"
															class="h-7 w-16 text-center text-sm"
															bind:value={editingCapacity[room.id]}
														/>
														<Button size="sm" class="h-7 px-2 text-xs" onclick={() => handleUpdateCapacity(room.id)}>บันทึก</Button>
														<Button size="sm" variant="ghost" class="h-7 px-2" onclick={() => delete editingCapacity[room.id]}>✕</Button>
													</div>
												{:else}
													<button
														class="hover:text-blue-600 underline-offset-2 hover:underline"
														onclick={() => (editingCapacity[room.id] = room.capacity)}
													>{room.capacity}</button>
												{/if}
											</Table.Cell>
											<Table.Cell class="text-center">{room.assignedCount}</Table.Cell>
											<Table.Cell class="text-right">
												<Button
													size="icon"
													variant="ghost"
													class="h-7 w-7 text-red-500 hover:text-red-700"
													onclick={() => handleRemoveRoom(room.id)}
												>
													<Trash2 class="h-4 w-4" />
												</Button>
											</Table.Cell>
										</Table.Row>
									{/each}
								</Table.Body>
							</Table.Root>
						</div>
					{/if}
				</div>

				<!-- ขวา: Config -->
				<div class="space-y-3">
					<!-- Config การจัดที่นั่ง -->
					<Card.Root>
						<Card.Header class="pb-2">
							<Card.Title class="text-sm">ตั้งค่าการจัดที่นั่ง</Card.Title>
						</Card.Header>
						<Card.Content class="space-y-3">
							<div>
								<label class="mb-1 block text-sm font-medium">รูปแบบเลขประจำตัวสอบ</label>
								<select
									bind:value={examConfig.examIdType}
									class="w-full rounded-md border px-3 py-2 text-sm"
								>
									<option value="application_number">ใช้เลขใบสมัคร</option>
									<option value="sequential">ลำดับต่อเนื่อง (1, 2, 3…)</option>
									<option value="custom_prefix">กำหนด Prefix เอง</option>
								</select>
							</div>

							{#if examConfig.examIdType === 'custom_prefix'}
								<div>
									<label class="mb-1 block text-sm font-medium">Prefix</label>
									<Input
										bind:value={examConfig.examIdPrefix}
										placeholder="เช่น 6801 → 68010001, 68010002…"
									/>
								</div>
							{/if}

							<div>
								<label class="mb-1 block text-sm font-medium">ลำดับรายชื่อ</label>
								<select
									bind:value={examConfig.sortOrder}
									class="w-full rounded-md border px-3 py-2 text-sm"
								>
									<option value="by_application">ตามลำดับการสมัคร</option>
									<option value="by_track">แบ่งตามสาย แล้วเรียงตามการสมัคร</option>
									<option value="random">สุ่ม</option>
								</select>
							</div>

							<Button size="sm" variant="outline" onclick={handleSaveConfig} disabled={savingConfig} class="w-full">
								{#if savingConfig}<Loader2 class="mr-1 h-4 w-4 animate-spin" />{/if}
								บันทึก config
							</Button>
						</Card.Content>
					</Card.Root>

					<!-- Copy จากรอบอื่น -->
					<Card.Root>
						<Card.Header class="pb-2">
							<Card.Title class="text-sm flex items-center gap-2"><Copy class="h-4 w-4" /> Copy จากรอบอื่น</Card.Title>
						</Card.Header>
						<Card.Content class="space-y-2">
							<select
								bind:value={copyFromRoundId}
								class="w-full rounded-md border px-3 py-2 text-sm"
							>
								<option value="">-- เลือกรอบ --</option>
								{#each allRounds as r}
									<option value={r.id}>{r.name}</option>
								{/each}
							</select>
							<Button
								size="sm"
								variant="outline"
								onclick={handleCopyFromRound}
								disabled={copying || !copyFromRoundId}
								class="w-full"
							>
								{#if copying}<Loader2 class="mr-1 h-4 w-4 animate-spin" />{/if}
								Copy ห้องสอบ
							</Button>
							<p class="text-muted-foreground text-xs">จะลบห้องสอบเดิมออกก่อน</p>
						</Card.Content>
					</Card.Root>

					<!-- ปุ่มจัดที่นั่ง -->
					<Button
						class="w-full"
						size="lg"
						disabled={assigning || examRooms.length === 0}
						onclick={handleAssignSeats}
					>
						{#if assigning}
							<Loader2 class="mr-2 h-4 w-4 animate-spin" />
							กำลังจัดที่นั่ง…
						{:else}
							<RefreshCw class="mr-2 h-4 w-4" />
							จัดที่นั่งสอบ
						{/if}
					</Button>
					{#if totalAssigned > 0}
						<p class="text-center text-xs text-blue-600">จัดแล้ว {totalAssigned} คน (กดอีกครั้งเพื่อจัดใหม่)</p>
					{/if}
				</div>
			</div>

		<!-- ========= Tab Seats ========= -->
		{:else}
			<div class="space-y-4">
				{#if seatGroups.length === 0}
					<div class="text-muted-foreground rounded-lg border border-dashed py-16 text-center text-sm">
						<ClipboardList class="mx-auto mb-2 h-8 w-8 opacity-40" />
						ยังไม่มีผลจัดที่นั่ง กลับไปแท็บ "ตั้งค่า" แล้วกด "จัดที่นั่งสอบ"
					</div>
				{:else}
					<div class="flex justify-between items-center">
						<p class="text-sm text-muted-foreground">รวม {seatGroups.reduce((s, g) => s + g.seats.length, 0)} คน ใน {seatGroups.length} ห้อง</p>
						<Button size="sm" variant="outline" onclick={printAllAdmitCards}>
							<Printer class="mr-1.5 h-4 w-4" /> พิมพ์บัตรสอบทั้งหมด
						</Button>
					</div>

					{#each seatGroups as group}
						<Card.Root>
							<Card.Header class="pb-2">
								<div class="flex items-center justify-between">
									<div>
										<Card.Title class="text-base">{group.roomName}</Card.Title>
										{#if group.buildingName}
											<p class="text-muted-foreground text-xs">{group.buildingName}</p>
										{/if}
									</div>
									<div class="flex items-center gap-3">
										<span class="text-sm text-muted-foreground">{group.seats.length}/{group.capacity} คน</span>
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
											<Table.Head class="w-28">เลขประจำตัวสอบ</Table.Head>
											<Table.Head class="w-16 text-center">ที่นั่ง</Table.Head>
											<Table.Head>ชื่อ-นามสกุล</Table.Head>
											<Table.Head>เลขบัตรประชาชน</Table.Head>
											<Table.Head>สาย</Table.Head>
										</Table.Row>
									</Table.Header>
									<Table.Body>
										{#each group.seats as seat}
											<Table.Row>
												<Table.Cell class="font-mono text-sm">{seat.examId ?? seat.applicationNumber ?? '-'}</Table.Cell>
												<Table.Cell class="text-center">{seat.seatNumber}</Table.Cell>
												<Table.Cell>{seat.fullName}</Table.Cell>
												<Table.Cell class="font-mono text-sm">{seat.nationalId}</Table.Cell>
												<Table.Cell class="text-muted-foreground text-sm">{seat.trackName ?? '-'}</Table.Cell>
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
