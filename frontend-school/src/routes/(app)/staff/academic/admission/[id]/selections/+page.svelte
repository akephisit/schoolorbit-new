<script lang="ts">
	import { onMount } from 'svelte';
	import type { PageProps } from './$types';
	import {
		getRound,
		listTracks,
		listSubjects,
		getTrackRanking,
		assignRooms,
		assignRoomsGlobal,
		changeApplicationTrack,
		updateSelectionSettings,
		moveApplicationRoom,
		resetRoomAssignments,
		type AdmissionRound,
		type AdmissionTrack,
		type AdmissionExamSubject,
		type TrackRankingResult
	} from '$lib/api/admission';
	import { Button } from '$lib/components/ui/button';
	import { Badge } from '$lib/components/ui/badge';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import { Label } from '$lib/components/ui/label';
	import * as RadioGroup from '$lib/components/ui/radio-group';
	import * as Card from '$lib/components/ui/card';
	import * as Table from '$lib/components/ui/table';
	import * as Select from '$lib/components/ui/select';
	import * as Dialog from '$lib/components/ui/dialog';
	import { toast } from 'svelte-sonner';
	import { ArrowLeft, GraduationCap, Trophy, Check, LoaderCircle, RotateCcw, Trash2 } from 'lucide-svelte';

	let { data, params }: PageProps = $props();
	let id = $derived(params.id);

	let round: AdmissionRound | null = $state(null);
	let tracks: AdmissionTrack[] = $state([]);
	let subjects: AdmissionExamSubject[] = $state([]);
	let selectedTrack = $state('');
	let selectedSubjectIdsByTrack: Record<string, string[]> = $state({});
	let currentSubjectIds = $derived(
		selectedSubjectIdsByTrack[selectedTrack] ?? subjects.map((s) => s.id)
	);
	let roomAssignmentMethodByTrack: Record<string, 'sequential' | 'round_robin'> = $state({});
	let currentMethod = $derived(roomAssignmentMethodByTrack[selectedTrack] ?? 'sequential');
	let ranking = $state<TrackRankingResult | null>(null);
	let loading = $state(false);
	let assigning = $state(false);
	let assigned = $state(false);
	let moveTargetTrackId: Record<string, string> = $state({});
	let moving: Record<string, boolean> = $state({});
	let moveTargetRoomId: Record<string, string> = $state({});
	let movingRoom: Record<string, boolean> = $state({});
	let assignmentMode = $state<'per_track' | 'global'>('per_track');
	let showAssignDialog = $state(false);
	let showAssignAllDialog = $state(false);
	let showAssignGlobalDialog = $state(false);
	let resetting = $state(false);
	let assigningAll = $state(false);
	let assigningGlobal = $state(false);
	let assignAllProgress = $state({ done: 0, total: 0 });
	let settingsLoaded = $state(false);
	let saveSettingsTimer: ReturnType<typeof setTimeout> | null = null;
	let reverting: Record<string, boolean> = $state({});

	let acceptedApps = $derived(ranking?.applications.filter((a) => !a.isOverflow) ?? []);
	let overflowApps = $derived(ranking?.applications.filter((a) => a.isOverflow) ?? []);
	let otherTracks = $derived(tracks.filter((t) => t.id !== selectedTrack));

	async function loadBase() {
		if (!id) return;
		const [r, t, s] = await Promise.all([getRound(id), listTracks(id), listSubjects(id)]);
		round = r;
		tracks = t;
		subjects = s;
		// โหลดการตั้งค่าที่บันทึกไว้ (แชร์ระหว่าง staff)
		const saved = r.selectionSettings;
		if (saved?.subjectsByTrack) {
			selectedSubjectIdsByTrack = saved.subjectsByTrack;
		}
		if (saved?.methodByTrack) {
			roomAssignmentMethodByTrack = saved.methodByTrack as Record<string, 'sequential' | 'round_robin'>;
		}
		if (saved?.assignmentMode === 'global') {
			assignmentMode = 'global';
		}
		if (t.length > 0) selectedTrack = t[0].id;
		settingsLoaded = true;
	}

	async function loadRanking() {
		if (!selectedTrack) return;
		loading = true;
		ranking = null;
		assigned = false;
		try {
			ranking = await getTrackRanking(selectedTrack, currentSubjectIds, currentMethod);
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'โหลดผลเรียงคะแนนไม่สำเร็จ');
		} finally {
			loading = false;
		}
	}

	function handleAssignRooms() {
		if (!id || !selectedTrack) return;
		showAssignDialog = true;
	}

	async function confirmAssignRooms() {
		showAssignDialog = false;
		if (!id || !selectedTrack) return;
		assigning = true;
		try {
			await assignRooms(id, selectedTrack, currentSubjectIds, currentMethod);
			toast.success('จัดห้องสำเร็จ!');
			assigned = true;
			await loadRanking();
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'จัดห้องไม่สำเร็จ');
		} finally {
			assigning = false;
		}
	}

	async function confirmAssignAll() {
		showAssignAllDialog = false;
		if (!id || tracks.length === 0) return;
		assigningAll = true;
		assignAllProgress = { done: 0, total: tracks.length };
		let failed = 0;
		for (const track of tracks) {
			try {
				await assignRooms(id, track.id, selectedSubjectIdsByTrack[track.id] ?? subjects.map((s) => s.id), roomAssignmentMethodByTrack[track.id] ?? 'sequential');
				assignAllProgress = { ...assignAllProgress, done: assignAllProgress.done + 1 };
			} catch {
				failed++;
			}
		}
		assigningAll = false;
		if (failed === 0) {
			toast.success(`จัดห้องทุกสายสำเร็จ (${tracks.length} สาย)`);
		} else {
			toast.warning(`จัดห้องสำเร็จ ${tracks.length - failed} สาย, ล้มเหลว ${failed} สาย`);
		}
		// reload ranking ของ track ที่กำลังดูอยู่
		await loadRanking();
	}

	async function confirmAssignGlobal() {
		showAssignGlobalDialog = false;
		if (!id) return;
		assigningGlobal = true;
		assignmentMode = 'global';
		try {
			await assignRoomsGlobal(id, 'sequential');
			toast.success('จัดห้องรวมทุกสายสำเร็จ!');
			assigned = false;
			await loadRanking();
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'จัดห้องรวมทุกสายไม่สำเร็จ');
		} finally {
			assigningGlobal = false;
		}
	}

	async function moveToTrack(appId: string) {
		const targetId = moveTargetTrackId[appId];
		if (!targetId) return;
		moving = { ...moving, [appId]: true };
		try {
			await changeApplicationTrack(appId, targetId);
			// Optimistic removal — ลบออกจาก list ทันที ไม่ต้อง reload
			if (ranking) {
				ranking = {
					...ranking,
					applications: ranking.applications.filter((a) => a.applicationId !== appId)
				};
				assigned = false; // ห้องต้องจัดใหม่เพราะจำนวนเปลี่ยน
			}
			const targetTrack = tracks.find((t) => t.id === targetId);
			toast.success(`ย้ายไปสาย ${targetTrack?.name ?? ''} สำเร็จ`);
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'ย้ายสายไม่สำเร็จ');
		} finally {
			moving = { ...moving, [appId]: false };
		}
	}

	async function revertTrack(appId: string) {
		reverting = { ...reverting, [appId]: true };
		try {
			await changeApplicationTrack(appId, null);
			// Optimistic removal — นักเรียนกลับไปสายเดิม ลบออกจาก list นี้
			if (ranking) {
				ranking = {
					...ranking,
					applications: ranking.applications.filter((a) => a.applicationId !== appId)
				};
				assigned = false;
			}
			toast.success('ย้อนกลับสายเดิมสำเร็จ');
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'ย้อนกลับไม่สำเร็จ');
		} finally {
			reverting = { ...reverting, [appId]: false };
		}
	}

	async function handleReset() {
		if (!selectedTrack) return;
		resetting = true;
		try {
			await resetRoomAssignments(selectedTrack);
			toast.success('ล้างการจัดห้องสำเร็จ');
			assigned = false;
			await loadRanking();
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'ล้างการจัดห้องไม่สำเร็จ');
		} finally {
			resetting = false;
		}
	}

	async function moveRoom(appId: string) {
		const targetRoomId = moveTargetRoomId[appId];
		if (!targetRoomId) return;
		movingRoom = { ...movingRoom, [appId]: true };
		try {
			await moveApplicationRoom(appId, targetRoomId);
			const roomName = ranking?.rooms.find((r) => r.roomId === targetRoomId)?.roomName;
			if (ranking) {
				ranking = {
					...ranking,
					applications: ranking.applications.map((a) =>
						a.applicationId === appId
							? { ...a, assignedRoom: roomName, assignedRoomId: targetRoomId, roomSaved: true }
							: a
					)
				};
			}
			moveTargetRoomId = { ...moveTargetRoomId, [appId]: '' };
			toast.success('ย้ายห้องสำเร็จ');
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'ย้ายห้องไม่สำเร็จ');
		} finally {
			movingRoom = { ...movingRoom, [appId]: false };
		}
	}

	// reload เมื่อ selectedTrack, selectedSubjectIdsByTrack หรือ roomAssignmentMethodByTrack เปลี่ยน
	// และ debounced save settings ลง DB
	$effect(() => {
		void selectedSubjectIdsByTrack;
		void roomAssignmentMethodByTrack;
		void assignmentMode;
		if (selectedTrack) loadRanking();
		if (settingsLoaded && id) {
			if (saveSettingsTimer) clearTimeout(saveSettingsTimer);
			const capturedSubjects = { ...selectedSubjectIdsByTrack };
			const capturedMethods = { ...roomAssignmentMethodByTrack };
			const capturedMode = assignmentMode;
			saveSettingsTimer = setTimeout(() => {
				updateSelectionSettings(id, {
					subjectsByTrack: capturedSubjects,
					methodByTrack: capturedMethods,
					assignmentMode: capturedMode
				}).catch(() => {});
			}, 500);
		}
	});

	onMount(loadBase);
</script>

<svelte:head>
	<title>{data.title} - SchoolOrbit</title>
</svelte:head>

<div class="space-y-5">
	<div class="flex items-center gap-3">
		<Button href="/staff/academic/admission/{id}" variant="ghost" size="sm">
			<ArrowLeft class="w-4 h-4 mr-1" /> ย้อนกลับ
		</Button>
		<h1 class="text-2xl font-bold flex items-center gap-2">
			<GraduationCap class="w-6 h-6" /> จัดห้องเรียน (เรียงคะแนน)
		</h1>
	</div>

	{#if round}
		<p class="text-muted-foreground text-sm">{round.name}</p>
	{/if}

	<!-- Track Selector -->
	<Card.Root>
		<Card.Content class="pt-4 pb-4 flex items-center gap-4 flex-wrap">
			<p class="text-sm font-medium shrink-0">สาย:</p>
			<div class="flex gap-2 flex-wrap flex-1">
				{#each tracks as track (track.id)}
					<Button
						variant={selectedTrack === track.id ? 'default' : 'outline'}
						size="sm"
						onclick={() => {
							selectedTrack = track.id;
						}}
					>
						{track.name}
					</Button>
				{/each}
			</div>
			{#if tracks.length > 1}
				<Button
					variant="secondary"
					size="sm"
					class="gap-1.5 shrink-0"
					disabled={assigningAll || assigningGlobal}
					onclick={() => (showAssignAllDialog = true)}
				>
					{#if assigningAll}
						<LoaderCircle class="w-3.5 h-3.5 animate-spin" />
						จัดอยู่... ({assignAllProgress.done}/{assignAllProgress.total})
					{:else}
						<GraduationCap class="w-3.5 h-3.5" />
						จัดห้อง (แยกตามสาย)
					{/if}
				</Button>
				<Button
					variant="secondary"
					size="sm"
					class="gap-1.5 shrink-0"
					disabled={assigningAll || assigningGlobal}
					onclick={() => (showAssignGlobalDialog = true)}
				>
					{#if assigningGlobal}
						<LoaderCircle class="w-3.5 h-3.5 animate-spin" />
						จัดอยู่...
					{:else}
						<GraduationCap class="w-3.5 h-3.5" />
						จัดห้อง (รวมทุกคน)
					{/if}
				</Button>
			{/if}
		</Card.Content>
	</Card.Root>

	<!-- Subject Picker (pass 1) -->
	{#if subjects.length > 0}
		<Card.Root>
			<Card.Content class="pt-4 pb-3 space-y-2">
				<p class="text-sm font-medium">วิชาที่ใช้คัดเลือก (pass 1)</p>
				<div class="flex flex-wrap gap-3">
					{#each subjects as s (s.id)}
						<div class="flex items-center gap-1.5">
							<Checkbox
								id="subj-{s.id}-{selectedTrack}"
								checked={currentSubjectIds.includes(s.id)}
								onCheckedChange={(v) => {
									const current =
										selectedSubjectIdsByTrack[selectedTrack] ?? subjects.map((x) => x.id);
									selectedSubjectIdsByTrack = {
										...selectedSubjectIdsByTrack,
										[selectedTrack]: v
											? [...current, s.id]
											: current.filter((x) => x !== s.id)
									};
								}}
							/>
							<Label for="subj-{s.id}-{selectedTrack}" class="font-normal cursor-pointer text-sm">
								{s.name}
								<span class="text-xs text-muted-foreground">({s.maxScore})</span>
							</Label>
						</div>
					{/each}
				</div>
				<p class="text-xs text-muted-foreground">
					คนที่ผ่านการคัดเลือกจะถูกเรียงใหม่ด้วยคะแนนรวมทุกวิชา (pass 2) เพื่อจัดห้อง
				</p>
			</Card.Content>
		</Card.Root>
	{/if}

	<!-- Room Assignment Method -->
	<Card.Root>
		<Card.Content class="pt-4 pb-3 space-y-2">
			<p class="text-sm font-medium">วิธีจัดห้อง (pass 2)</p>
			<RadioGroup.Root
				value={currentMethod}
				onValueChange={(v) => {
					roomAssignmentMethodByTrack = { ...roomAssignmentMethodByTrack, [selectedTrack]: v as 'sequential' | 'round_robin' };
				}}
				class="flex flex-wrap gap-4"
			>
				<div class="flex items-center gap-2">
					<RadioGroup.Item value="sequential" id="method-seq-{selectedTrack}" />
					<Label for="method-seq-{selectedTrack}" class="font-normal cursor-pointer">
						เรียงตามคะแนน
						<span class="text-xs text-muted-foreground">(คะแนนสูงอยู่ห้องแรก)</span>
					</Label>
				</div>
				<div class="flex items-center gap-2">
					<RadioGroup.Item value="round_robin" id="method-rr-{selectedTrack}" />
					<Label for="method-rr-{selectedTrack}" class="font-normal cursor-pointer">
						กระจายเฉลี่ย (round-robin)
						<span class="text-xs text-muted-foreground">(สลับห้องตามลำดับคะแนน ทุกห้องได้คนคะแนนสูง-ต่ำปนกัน)</span>
					</Label>
				</div>
			</RadioGroup.Root>
		</Card.Content>
	</Card.Root>

	{#if loading}
		<Card.Root>
			<Card.Content class="flex justify-center py-16">
				<LoaderCircle class="w-8 h-8 animate-spin text-primary" />
			</Card.Content>
		</Card.Root>
	{:else if ranking}
		<!-- Room Summary -->
		{#if ranking.rooms?.length > 0}
			<div class="grid grid-cols-2 sm:grid-cols-4 gap-3">
				{#each ranking.rooms as room (room.roomId)}
					<Card.Root>
						<Card.Content class="pt-3 pb-3 text-center space-y-1">
							<p class="font-semibold">{room.roomName}</p>
							{#if room.studentCount > 0}
								<p class="text-sm font-medium">{room.studentCount} / {room.capacity} คน</p>
								<p class="text-xs text-muted-foreground">
									ชาย {room.maleCount} · หญิง {room.femaleCount}
									{#if room.studentCount - room.maleCount - room.femaleCount > 0}
										· ไม่ระบุ {room.studentCount - room.maleCount - room.femaleCount}
									{/if}
								</p>
							{:else}
								<p class="text-xs text-muted-foreground">รับ {room.capacity} คน</p>
							{/if}
						</Card.Content>
					</Card.Root>
				{/each}
			</div>
		{/if}

		<!-- Accepted Ranking Table -->
		<Card.Root>
			<Card.Header class="flex flex-row items-center justify-between pb-3">
				<Card.Title class="flex items-center gap-2">
					<Trophy class="w-5 h-5 text-yellow-500" />
					ผลเรียงคะแนน — {ranking.trackName}
					<Badge variant="secondary">{acceptedApps.length} คน</Badge>
				</Card.Title>
				<div class="flex gap-2">
					{#if acceptedApps.some((a) => a.roomSaved)}
						<Button
							variant="ghost"
							size="sm"
							class="gap-1.5 text-muted-foreground hover:text-destructive"
							disabled={resetting}
							onclick={handleReset}
							title="ล้างการจัดห้องทั้งหมด"
						>
							{#if resetting}
								<LoaderCircle class="w-3.5 h-3.5 animate-spin" />
							{:else}
								<Trash2 class="w-3.5 h-3.5" />
							{/if}
							ล้างการจัดห้อง
						</Button>
					{/if}
					<Button
						onclick={handleAssignRooms}
						disabled={assigning || acceptedApps.length === 0}
						variant={assigned ? 'outline' : 'default'}
						class="gap-2"
					>
						{#if assigning}
							<LoaderCircle class="w-4 h-4 animate-spin" />
						{:else}
							<Check class="w-4 h-4" />
						{/if}
						{assigning ? 'กำลังจัดห้อง...' : assigned ? 'จัดห้องแล้ว (จัดใหม่)' : 'บันทึกจัดห้อง'}
					</Button>
				</div>
			</Card.Header>

			<div class="overflow-x-auto">
			<Table.Root>
				<Table.Header>
					<Table.Row>
						<Table.Head class="w-16 text-center">อันดับคัด</Table.Head>
						<Table.Head>เลขที่ใบสมัคร</Table.Head>
						<Table.Head>ชื่อ-สกุล</Table.Head>
						<Table.Head class="text-center">คะแนนคัด</Table.Head>
						<Table.Head class="text-center">คะแนนรวม</Table.Head>
						<Table.Head class="text-center">อันดับสุดท้าย</Table.Head>
						<Table.Head class="text-center">ห้องที่ได้</Table.Head>
						<Table.Head>ย้ายสาย</Table.Head>
					</Table.Row>
				</Table.Header>
				<Table.Body>
					{#each acceptedApps as app (app.applicationId)}
						<Table.Row class={app.isTrackOverridden ? 'bg-orange-50/50' : ''}>
							<Table.Cell class="text-center">
								<span
									class="inline-flex items-center justify-center w-7 h-7 rounded-full text-xs font-bold {app.selectionRank ===
									1
										? 'bg-yellow-100 text-yellow-700'
										: app.selectionRank <= 3
											? 'bg-gray-100 text-gray-700'
											: 'text-muted-foreground'}"
								>
									{app.selectionRank}
								</span>
							</Table.Cell>
							<Table.Cell class="font-mono text-xs">{app.applicationNumber ?? '-'}</Table.Cell>
							<Table.Cell class="font-medium">
								{app.fullName}
								{#if app.isTrackOverridden && app.originalTrackName}
									<span class="ml-1.5 inline-flex items-center gap-1 rounded-full bg-orange-100 px-2 py-0.5 text-xs font-medium text-orange-700">
										ย้ายมาจาก {app.originalTrackName}
									</span>
								{/if}
							</Table.Cell>
							<Table.Cell class="text-center font-semibold text-blue-600">
								{app.selectionScore.toFixed(1)}
							</Table.Cell>
							<Table.Cell class="text-center font-semibold text-primary">
								{app.totalScore.toFixed(1)}
							</Table.Cell>
							<Table.Cell class="text-center">
								{#if app.finalRank != null}
									<span class="text-sm font-medium">{app.finalRank}</span>
								{:else}
									<span class="text-xs text-muted-foreground">-</span>
								{/if}
							</Table.Cell>
							<Table.Cell class="text-center">
								{#if ranking?.rooms && ranking.rooms.length > 0 && app.roomSaved}
									<div class="flex items-center justify-center gap-1.5 flex-wrap">
										<Badge variant="outline">{app.assignedRoom}</Badge>
										<Select.Root
											type="single"
											value={moveTargetRoomId[app.applicationId] ?? ''}
											onValueChange={(v) => {
												moveTargetRoomId = { ...moveTargetRoomId, [app.applicationId]: v };
											}}
										>
											<Select.Trigger class="h-6 text-xs w-20 px-2">
												{ranking.rooms.find((r) => r.roomId === moveTargetRoomId[app.applicationId])?.roomName ?? 'ย้าย'}
											</Select.Trigger>
											<Select.Content>
												{#each ranking.rooms.filter((r) => r.roomName !== app.assignedRoom) as room (room.roomId)}
													<Select.Item value={room.roomId}>{room.roomName}</Select.Item>
												{/each}
											</Select.Content>
										</Select.Root>
										{#if moveTargetRoomId[app.applicationId]}
											<Button
												size="sm"
												class="h-6 text-xs px-2"
												disabled={movingRoom[app.applicationId]}
												onclick={() => moveRoom(app.applicationId)}
											>
												{#if movingRoom[app.applicationId]}
													<LoaderCircle class="w-3 h-3 animate-spin" />
												{:else}
													ย้าย
												{/if}
											</Button>
										{/if}
									</div>
								{:else if app.assignedRoom}
									<Badge variant="outline">{app.assignedRoom}</Badge>
								{:else}
									<span class="text-xs text-muted-foreground">ยังไม่จัดห้อง</span>
								{/if}
							</Table.Cell>
							<Table.Cell>
								<div class="flex gap-2 items-center flex-wrap">
									{#if app.isTrackOverridden}
										<Button
											size="sm"
											variant="ghost"
											class="h-8 text-xs text-orange-600 hover:text-orange-700 hover:bg-orange-50 gap-1"
											disabled={reverting[app.applicationId]}
											onclick={() => revertTrack(app.applicationId)}
											title="ย้อนกลับสายที่สมัคร"
										>
											{#if reverting[app.applicationId]}
												<LoaderCircle class="w-3 h-3 animate-spin" />
											{:else}
												<RotateCcw class="w-3 h-3" />
												ย้อนกลับ
											{/if}
										</Button>
									{/if}
									<Select.Root
										type="single"
										value={moveTargetTrackId[app.applicationId] ?? ''}
										onValueChange={(v) => {
											moveTargetTrackId = { ...moveTargetTrackId, [app.applicationId]: v };
										}}
									>
										<Select.Trigger class="h-8 text-xs w-36">
											{otherTracks.find(
												(t) => t.id === moveTargetTrackId[app.applicationId]
											)?.name ?? 'เลือกสาย'}
										</Select.Trigger>
										<Select.Content>
											{#each otherTracks as t (t.id)}
												<Select.Item value={t.id}>{t.name}</Select.Item>
											{/each}
										</Select.Content>
									</Select.Root>
									<Button
										size="sm"
										class="h-8 text-xs"
										disabled={!moveTargetTrackId[app.applicationId] || moving[app.applicationId]}
										onclick={() => moveToTrack(app.applicationId)}
									>
										{#if moving[app.applicationId]}
											<LoaderCircle class="w-3 h-3 animate-spin" />
										{:else}
											ย้าย
										{/if}
									</Button>
								</div>
							</Table.Cell>
						</Table.Row>
					{/each}
				</Table.Body>
			</Table.Root>
			</div>
		</Card.Root>

		<!-- Overflow Section -->
		{#if overflowApps.length > 0}
			<Card.Root class="border-orange-200">
				<Card.Header class="pb-3">
					<Card.Title class="text-orange-600">
						เกินโควต้า ({overflowApps.length} คน)
					</Card.Title>
					<p class="text-xs text-muted-foreground">
						นักเรียนกลุ่มนี้ไม่ผ่านการคัดเลือก สามารถย้ายไปสายอื่นได้
					</p>
				</Card.Header>
				<div class="overflow-x-auto">
				<Table.Root>
					<Table.Header>
						<Table.Row>
							<Table.Head class="w-16 text-center">อันดับคัด</Table.Head>
							<Table.Head>เลขที่ใบสมัคร</Table.Head>
							<Table.Head>ชื่อ-สกุล</Table.Head>
							<Table.Head class="text-center">คะแนนคัด</Table.Head>
							<Table.Head class="text-center">คะแนนรวม</Table.Head>
							<Table.Head>ย้ายสาย</Table.Head>
						</Table.Row>
					</Table.Header>
					<Table.Body>
						{#each overflowApps as app (app.applicationId)}
							<Table.Row class="bg-orange-50">
								<Table.Cell class="text-center text-muted-foreground text-sm">
									{app.selectionRank}
								</Table.Cell>
								<Table.Cell class="font-mono text-xs">{app.applicationNumber ?? '-'}</Table.Cell>
								<Table.Cell class="font-medium">
									{app.fullName}
									{#if app.isTrackOverridden && app.originalTrackName}
										<span class="ml-1.5 inline-flex items-center gap-1 rounded-full bg-orange-200 px-2 py-0.5 text-xs font-medium text-orange-800">
											ย้ายมาจาก {app.originalTrackName}
										</span>
									{/if}
								</Table.Cell>
								<Table.Cell class="text-center text-blue-600 font-semibold">
									{app.selectionScore.toFixed(1)}
								</Table.Cell>
								<Table.Cell class="text-center font-semibold">
									{app.totalScore.toFixed(1)}
								</Table.Cell>
								<Table.Cell>
									<div class="flex gap-2 items-center flex-wrap">
										{#if app.isTrackOverridden}
											<Button
												size="sm"
												variant="ghost"
												class="h-8 text-xs text-orange-600 hover:text-orange-700 hover:bg-orange-100 gap-1"
												disabled={reverting[app.applicationId]}
												onclick={() => revertTrack(app.applicationId)}
												title="ย้อนกลับสายที่สมัคร"
											>
												{#if reverting[app.applicationId]}
													<LoaderCircle class="w-3 h-3 animate-spin" />
												{:else}
													<RotateCcw class="w-3 h-3" />
													ย้อนกลับ
												{/if}
											</Button>
										{/if}
										<Select.Root
											type="single"
											value={moveTargetTrackId[app.applicationId] ?? ''}
											onValueChange={(v) => {
												moveTargetTrackId = { ...moveTargetTrackId, [app.applicationId]: v };
											}}
										>
											<Select.Trigger class="h-8 text-xs w-36">
												{otherTracks.find(
													(t) => t.id === moveTargetTrackId[app.applicationId]
												)?.name ?? 'เลือกสาย'}
											</Select.Trigger>
											<Select.Content>
												{#each otherTracks as t (t.id)}
													<Select.Item value={t.id}>{t.name}</Select.Item>
												{/each}
											</Select.Content>
										</Select.Root>
										<Button
											size="sm"
											class="h-8 text-xs"
											disabled={!moveTargetTrackId[app.applicationId] || moving[app.applicationId]}
											onclick={() => moveToTrack(app.applicationId)}
										>
											{#if moving[app.applicationId]}
												<LoaderCircle class="w-3 h-3 animate-spin" />
											{:else}
												ย้าย
											{/if}
										</Button>
									</div>
								</Table.Cell>
							</Table.Row>
						{/each}
					</Table.Body>
				</Table.Root>
				</div>
			</Card.Root>
		{/if}
	{/if}
</div>

<Dialog.Root bind:open={showAssignDialog}>
	<Dialog.Content class="sm:max-w-[400px]">
		<Dialog.Header>
			<Dialog.Title>ยืนยันการจัดห้อง</Dialog.Title>
			<Dialog.Description>
				การดำเนินการนี้จะลบผลจัดห้องเดิมและจัดใหม่ทั้งหมด
				<strong class="text-orange-600">รวมถึงการย้ายห้องที่ปรับด้วยมือ</strong>
				ต้องการดำเนินการต่อหรือไม่?
			</Dialog.Description>
		</Dialog.Header>
		<Dialog.Footer class="flex-col sm:flex-row gap-2">
			<Button variant="outline" onclick={() => (showAssignDialog = false)}>ยกเลิก</Button>
			<Button onclick={confirmAssignRooms}>ยืนยัน</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<Dialog.Root bind:open={showAssignAllDialog}>
	<Dialog.Content class="sm:max-w-[440px]">
		<Dialog.Header>
			<Dialog.Title>ยืนยันการจัดห้อง (แยกตามสาย)</Dialog.Title>
			<Dialog.Description>
				จะจัดห้องให้ทุกสาย ({tracks.length} สาย) พร้อมกัน แต่ละสายใช้วิชาและวิธีจัดห้องของตัวเอง
				ผลจัดห้องเดิมของทุกสายจะถูกแทนที่
				<strong class="text-orange-600">รวมถึงการย้ายห้องที่ปรับด้วยมือ</strong>
				ต้องการดำเนินการต่อหรือไม่?
			</Dialog.Description>
		</Dialog.Header>
		<Dialog.Footer class="flex-col sm:flex-row gap-2">
			<Button variant="outline" onclick={() => (showAssignAllDialog = false)}>ยกเลิก</Button>
			<Button onclick={confirmAssignAll}>ยืนยัน</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<Dialog.Root bind:open={showAssignGlobalDialog}>
	<Dialog.Content class="sm:max-w-[440px]">
		<Dialog.Header>
			<Dialog.Title>ยืนยันการจัดห้อง (รวมทุกคน)</Dialog.Title>
			<Dialog.Description>
				นักเรียนทุกสายจะถูกนำมาเรียงคะแนนรวมด้วยกัน แล้วจัดลงทุกห้องจากทุกสายโดยไม่สนว่าสมัครสายใด
				<br /><br />
				ผลจัดห้องเดิม<strong>ทุกสาย</strong>จะถูกแทนที่
				<strong class="text-orange-600">รวมถึงการย้ายห้องที่ปรับด้วยมือ</strong>
				ต้องการดำเนินการต่อหรือไม่?
			</Dialog.Description>
		</Dialog.Header>
		<Dialog.Footer class="flex-col sm:flex-row gap-2">
			<Button variant="outline" onclick={() => (showAssignGlobalDialog = false)}>ยกเลิก</Button>
			<Button onclick={confirmAssignGlobal}>ยืนยัน</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
