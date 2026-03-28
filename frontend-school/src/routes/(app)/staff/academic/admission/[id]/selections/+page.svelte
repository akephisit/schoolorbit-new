<script lang="ts">
	import { onMount } from 'svelte';
	import type { PageProps } from './$types';
	import {
		getRound,
		listTracks,
		listSubjects,
		getTrackRanking,
		getGlobalRanking,
		getRoomsForRound,
		assignRooms,
		assignRoomsGlobal,
		changeApplicationTrack,
		updateSelectionSettings,
		moveApplicationRoom,
		resetRoomAssignments,
		resetAllRoomAssignments,
		type AdmissionRound,
		type AdmissionTrack,
		type AdmissionExamSubject,
		type TrackRankingResult,
		type GlobalRankingResult,
		type RoomBasic
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
	import { ArrowLeft, GraduationCap, Trophy, Check, LoaderCircle, RotateCcw, Trash2, GripVertical, Layers, SplitSquareVertical } from 'lucide-svelte';

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

	// Global mode state
	let globalRanking = $state<GlobalRankingResult | null>(null);
	let loadingGlobal = $state(false);
	let globalViewTab = $state('all'); // 'all' | roomId | 'overflow'
	let roomOrderForGlobal = $state<RoomBasic[]>([]);
	let loadingRooms = $state(false);
	let dragIndex = $state<number | null>(null);

	// Dialog state
	let showAssignDialog = $state(false);
	let showAssignAllDialog = $state(false);
	let showAssignGlobalDialog = $state(false);
	let resetting = $state(false);
	let resettingAll = $state(false);
	let showResetAllDialog = $state(false);
	let assigningAll = $state(false);
	let assigningGlobal = $state(false);
	let assignAllProgress = $state({ done: 0, total: 0 });
	let settingsLoaded = $state(false);
	let saveSettingsTimer: ReturnType<typeof setTimeout> | null = null;
	let reverting: Record<string, boolean> = $state({});

	let acceptedApps = $derived(ranking?.applications.filter((a) => !a.isOverflow) ?? []);
	let overflowApps = $derived(ranking?.applications.filter((a) => a.isOverflow) ?? []);
	let otherTracks = $derived(tracks.filter((t) => t.id !== selectedTrack));

	let globalAccepted = $derived(globalRanking?.applications.filter((a) => !a.isOverflow) ?? []);
	let globalOverflow = $derived(globalRanking?.applications.filter((a) => a.isOverflow) ?? []);
	// เรียงห้องตาม DnD order (roomOrderForGlobal) ถ้ามี ไม่งั้นตามที่ backend ส่งมา
	let globalRoomsSorted = $derived(() => {
		const rooms = globalRanking?.rooms ?? [];
		if (roomOrderForGlobal.length === 0) return rooms;
		const orderMap = new Map(roomOrderForGlobal.map((r, i) => [r.roomId, i]));
		return [...rooms].sort((a, b) => (orderMap.get(a.roomId) ?? 999) - (orderMap.get(b.roomId) ?? 999));
	});
	let globalTabApps = $derived(() => {
		if (globalViewTab === 'all') return globalAccepted;
		if (globalViewTab === 'overflow') return globalOverflow;
		return globalAccepted.filter((a) => a.assignedRoomId === globalViewTab);
	});

	async function loadBase() {
		if (!id) return;
		const [r, t, s] = await Promise.all([getRound(id), listTracks(id), listSubjects(id)]);
		round = r;
		tracks = t;
		subjects = s;
		const saved = r.selectionSettings;
		if (saved?.subjectsByTrack) {
			selectedSubjectIdsByTrack = saved.subjectsByTrack;
		}
		if (saved?.methodByTrack) {
			roomAssignmentMethodByTrack = saved.methodByTrack as Record<string, 'sequential' | 'round_robin'>;
		}
		if (saved?.assignmentMode === 'global') {
			assignmentMode = 'global';
			await Promise.all([loadRoomsForGlobal(), loadGlobalRanking()]);
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

	async function loadRoomsForGlobal() {
		if (!id) return;
		loadingRooms = true;
		try {
			roomOrderForGlobal = await getRoomsForRound(id);
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'โหลดรายการห้องไม่สำเร็จ');
		} finally {
			loadingRooms = false;
		}
	}

	async function loadGlobalRanking() {
		if (!id) return;
		loadingGlobal = true;
		try {
			globalRanking = await getGlobalRanking(id);
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'โหลดผลรวมไม่สำเร็จ');
		} finally {
			loadingGlobal = false;
		}
	}

	async function switchMode(mode: 'per_track' | 'global') {
		if (assignmentMode === mode) return;
		assignmentMode = mode;
		if (mode === 'global' && roomOrderForGlobal.length === 0) {
			await loadRoomsForGlobal();
		}
	}

	async function confirmAssignRooms() {
		showAssignDialog = false;
		if (!id || !selectedTrack) return;
		assigning = true;
		try {
			await assignRooms(id, selectedTrack, currentSubjectIds, currentMethod);
			toast.success('จัดห้องสำเร็จ!');
			assigned = true;
			updateSelectionSettings(id, { assignmentMode: 'per_track' }).catch(() => {});
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
			updateSelectionSettings(id, { assignmentMode: 'per_track' }).catch(() => {});
		} else {
			toast.warning(`จัดห้องสำเร็จ ${tracks.length - failed} สาย, ล้มเหลว ${failed} สาย`);
		}
		await loadRanking();
	}

	async function confirmAssignGlobal() {
		showAssignGlobalDialog = false;
		if (!id) return;
		assigningGlobal = true;
		try {
			await assignRoomsGlobal(id, 'sequential', roomOrderForGlobal.map((r) => r.roomId));
			toast.success('จัดห้องรวมทุกสายสำเร็จ!');
			assigned = false;
			globalViewTab = 'all';
			updateSelectionSettings(id, { assignmentMode: 'global' }).catch(() => {});
			await loadGlobalRanking();
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
			if (ranking) {
				ranking = {
					...ranking,
					applications: ranking.applications.filter((a) => a.applicationId !== appId)
				};
				assigned = false;
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

	async function moveRoomGlobal(appId: string) {
		const targetRoomId = moveTargetRoomId[appId];
		if (!targetRoomId) return;
		movingRoom = { ...movingRoom, [appId]: true };
		try {
			await moveApplicationRoom(appId, targetRoomId);
			const roomName = globalRanking?.rooms.find((r) => r.roomId === targetRoomId)?.roomName;
			if (globalRanking) {
				globalRanking = {
					...globalRanking,
					applications: globalRanking.applications.map((a) =>
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

	async function handleResetAll() {
		if (!id) return;
		resettingAll = true;
		try {
			await resetAllRoomAssignments(id);
			toast.success('ล้างการจัดห้องทั้งหมดสำเร็จ');
			ranking = null;
			globalRanking = null;
			assigned = false;
			await loadRanking();
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'ล้างการจัดห้องไม่สำเร็จ');
		} finally {
			resettingAll = false;
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

	$effect(() => {
		void selectedSubjectIdsByTrack;
		void roomAssignmentMethodByTrack;
		if (selectedTrack && assignmentMode === 'per_track') loadRanking();
		if (settingsLoaded && id) {
			if (saveSettingsTimer) clearTimeout(saveSettingsTimer);
			const capturedSubjects = { ...selectedSubjectIdsByTrack };
			const capturedMethods = { ...roomAssignmentMethodByTrack };
			saveSettingsTimer = setTimeout(() => {
				updateSelectionSettings(id, {
					subjectsByTrack: capturedSubjects,
					methodByTrack: capturedMethods,
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

	<!-- Step 1: เลือกโหมดจัดห้อง -->
	<Card.Root>
		<Card.Content class="pt-4 pb-4 space-y-3">
			<div class="flex items-center justify-between">
				<p class="text-sm font-semibold text-muted-foreground uppercase tracking-wide">โหมดจัดห้อง</p>
				<Button
					variant="ghost"
					size="sm"
					class="gap-1.5 text-muted-foreground hover:text-destructive text-xs"
					disabled={resettingAll}
					onclick={() => (showResetAllDialog = true)}
				>
					{#if resettingAll}
						<LoaderCircle class="w-3.5 h-3.5 animate-spin" />
					{:else}
						<Trash2 class="w-3.5 h-3.5" />
					{/if}
					ล้างการจัดห้องทั้งหมด
				</Button>
			</div>
			<div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
				<button
					onclick={() => switchMode('per_track')}
					class="flex items-start gap-3 rounded-lg border-2 p-4 text-left transition-colors {assignmentMode === 'per_track' ? 'border-primary bg-primary/5' : 'border-border hover:border-muted-foreground/40'}"
				>
					<SplitSquareVertical class="w-5 h-5 mt-0.5 shrink-0 {assignmentMode === 'per_track' ? 'text-primary' : 'text-muted-foreground'}" />
					<div>
						<p class="font-semibold text-sm">แยกตามสาย</p>
						<p class="text-xs text-muted-foreground mt-0.5">แต่ละสายคัดเลือกและจัดห้องเองแยกกัน ใช้วิชาและเกณฑ์ของแต่ละสาย</p>
					</div>
				</button>
				<button
					onclick={() => switchMode('global')}
					class="flex items-start gap-3 rounded-lg border-2 p-4 text-left transition-colors {assignmentMode === 'global' ? 'border-primary bg-primary/5' : 'border-border hover:border-muted-foreground/40'}"
				>
					<Layers class="w-5 h-5 mt-0.5 shrink-0 {assignmentMode === 'global' ? 'text-primary' : 'text-muted-foreground'}" />
					<div>
						<p class="font-semibold text-sm">รวมทุกคน</p>
						<p class="text-xs text-muted-foreground mt-0.5">นำคะแนนทุกคนทุกสายมาเรียงรวมกัน แล้วจัดลงห้องทั้งหมดโดยไม่แยกสาย</p>
					</div>
				</button>
			</div>
		</Card.Content>
	</Card.Root>

	<!-- ========== โหมด: แยกตามสาย ========== -->
	{#if assignmentMode === 'per_track'}
		<!-- Track Selector -->
		<Card.Root>
			<Card.Content class="pt-4 pb-4 flex items-center gap-4 flex-wrap">
				<p class="text-sm font-medium shrink-0">สาย:</p>
				<div class="flex gap-2 flex-wrap flex-1">
					{#each tracks as track (track.id)}
						<Button
							variant={selectedTrack === track.id ? 'default' : 'outline'}
							size="sm"
							onclick={() => { selectedTrack = track.id; }}
						>
							{track.name}
						</Button>
					{/each}
				</div>
				{#if tracks.length > 1}
					<Button
						variant="ghost"
						size="sm"
						class="gap-1.5 shrink-0 text-muted-foreground"
						disabled={assigningAll}
						onclick={() => (showAssignAllDialog = true)}
						title="จัดห้องทุกสายพร้อมกันด้วยการตั้งค่าปัจจุบัน"
					>
						{#if assigningAll}
							<LoaderCircle class="w-3.5 h-3.5 animate-spin" />
							จัดอยู่... ({assignAllProgress.done}/{assignAllProgress.total})
						{:else}
							จัดทุกสายพร้อมกัน
						{/if}
					</Button>
				{/if}
			</Card.Content>
		</Card.Root>

		<!-- วิชาที่ใช้คัดผ่าน-ไม่ผ่าน -->
		{#if subjects.length > 0}
			<Card.Root>
				<Card.Content class="pt-4 pb-3 space-y-2">
					<p class="text-sm font-medium">วิชาที่ใช้คัดผ่าน-ไม่ผ่าน</p>
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
						คนที่ผ่านจะถูกเรียงใหม่ด้วยคะแนนรวมทุกวิชาเพื่อจัดลงห้อง
					</p>
				</Card.Content>
			</Card.Root>
		{/if}

		<!-- วิธีจัดห้อง -->
		<Card.Root>
			<Card.Content class="pt-4 pb-3 space-y-2">
				<p class="text-sm font-medium">วิธีจัดห้อง</p>
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
							<span class="text-xs text-muted-foreground">(สลับห้องตามลำดับ ทุกห้องได้คนคะแนนสูง-ต่ำปนกัน)</span>
						</Label>
					</div>
				</RadioGroup.Root>
			</Card.Content>
		</Card.Root>

		<!-- ผลเรียงคะแนน -->
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

			<!-- Accepted Table -->
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
							onclick={() => (showAssignDialog = true)}
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
							<Table.Head class="text-center">อันดับในห้อง</Table.Head>
							<Table.Head class="text-center">ห้องที่ได้</Table.Head>
							<Table.Head>ย้ายสาย</Table.Head>
						</Table.Row>
					</Table.Header>
					<Table.Body>
						{#each acceptedApps as app (app.applicationId)}
							<Table.Row class={app.isTrackOverridden ? 'bg-orange-50/50' : ''}>
								<Table.Cell class="text-center">
									<span
										class="inline-flex items-center justify-center w-7 h-7 rounded-full text-xs font-bold {app.selectionRank === 1 ? 'bg-yellow-100 text-yellow-700' : app.selectionRank <= 3 ? 'bg-gray-100 text-gray-700' : 'text-muted-foreground'}"
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
												{otherTracks.find((t) => t.id === moveTargetTrackId[app.applicationId])?.name ?? 'เลือกสาย'}
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

			<!-- Overflow -->
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
													{otherTracks.find((t) => t.id === moveTargetTrackId[app.applicationId])?.name ?? 'เลือกสาย'}
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

	<!-- ========== โหมด: รวมทุกคน ========== -->
	{:else}
		<!-- DnD Room Ordering -->
		<Card.Root>
			<Card.Content class="pt-4 pb-4 space-y-3">
				<div class="flex items-center justify-between">
					<div>
						<p class="text-sm font-medium">ลำดับห้อง</p>
						<p class="text-xs text-muted-foreground">ลาก-วางเพื่อกำหนดว่าห้องไหนได้นักเรียนคะแนนสูงก่อน</p>
					</div>
					<Button
						onclick={() => (showAssignGlobalDialog = true)}
						disabled={assigningGlobal || roomOrderForGlobal.length === 0}
						class="gap-2 shrink-0"
					>
						{#if assigningGlobal}
							<LoaderCircle class="w-4 h-4 animate-spin" />
							กำลังจัดห้อง...
						{:else}
							<Check class="w-4 h-4" />
							จัดห้อง
						{/if}
					</Button>
				</div>

				{#if loadingRooms}
					<div class="flex justify-center py-8">
						<LoaderCircle class="w-6 h-6 animate-spin text-primary" />
					</div>
				{:else if roomOrderForGlobal.length === 0}
					<p class="text-sm text-muted-foreground text-center py-6">ไม่พบห้องเรียนในรอบนี้</p>
				{:else}
					<div class="space-y-2" role="list">
						{#each roomOrderForGlobal as room, i (room.roomId)}
							<div
								role="listitem"
								draggable={true}
								ondragstart={() => { dragIndex = i; }}
								ondragenter={() => {
									if (dragIndex === null || dragIndex === i) return;
									const newOrder = [...roomOrderForGlobal];
									const [moved] = newOrder.splice(dragIndex, 1);
									newOrder.splice(i, 0, moved);
									roomOrderForGlobal = newOrder;
									dragIndex = i;
								}}
								ondragend={() => { dragIndex = null; }}
								ondragover={(e) => e.preventDefault()}
								class="flex items-center gap-3 rounded-lg border bg-background px-3 py-2.5 transition-opacity {dragIndex === i ? 'opacity-50' : ''} cursor-grab active:cursor-grabbing touch-none"
								style="touch-action: none;"
							>
								<GripVertical class="w-4 h-4 text-muted-foreground shrink-0" />
								<span class="text-xs text-muted-foreground w-5 text-center shrink-0">{i + 1}</span>
								<span class="font-medium text-sm flex-1">{room.roomName}</span>
								<span class="text-xs text-muted-foreground">รับ {room.capacity} คน</span>
							</div>
						{/each}
					</div>
				{/if}
			</Card.Content>
		</Card.Root>

		<!-- ผลจัดห้อง (tabs) -->
		{#if loadingGlobal}
			<Card.Root>
				<Card.Content class="flex justify-center py-16">
					<LoaderCircle class="w-8 h-8 animate-spin text-primary" />
				</Card.Content>
			</Card.Root>
		{:else if globalRanking}
			<!-- Room summary cards -->
			{#if globalRanking.rooms.length > 0}
				<div class="grid grid-cols-2 sm:grid-cols-4 gap-3">
					{#each globalRoomsSorted() as room (room.roomId)}
						<Card.Root>
							<Card.Content class="pt-3 pb-3 text-center space-y-1">
								<p class="font-semibold">{room.roomName}</p>
								{#if room.studentCount > 0}
									<p class="text-sm font-medium">{room.studentCount} / {room.capacity} คน</p>
									<p class="text-xs text-muted-foreground">ชาย {room.maleCount} · หญิง {room.femaleCount}</p>
								{:else}
									<p class="text-xs text-muted-foreground">รับ {room.capacity} คน</p>
								{/if}
							</Card.Content>
						</Card.Root>
					{/each}
				</div>
			{/if}

			<!-- Tabs -->
			<div class="flex gap-2 flex-wrap">
				<Button
					variant={globalViewTab === 'all' ? 'default' : 'outline'}
					size="sm"
					onclick={() => (globalViewTab = 'all')}
				>
					ทั้งหมด ({globalAccepted.length})
				</Button>
				{#each globalRoomsSorted() as room (room.roomId)}
					<Button
						variant={globalViewTab === room.roomId ? 'default' : 'outline'}
						size="sm"
						onclick={() => (globalViewTab = room.roomId)}
					>
						{room.roomName} ({room.studentCount})
					</Button>
				{/each}
				{#if globalOverflow.length > 0}
					<Button
						variant={globalViewTab === 'overflow' ? 'destructive' : 'outline'}
						size="sm"
						onclick={() => (globalViewTab = 'overflow')}
						class={globalViewTab !== 'overflow' ? 'border-orange-300 text-orange-600 hover:bg-orange-50' : ''}
					>
						เกินโควต้า ({globalOverflow.length})
					</Button>
				{/if}
			</div>

			<!-- Table -->
			<Card.Root class={globalViewTab === 'overflow' ? 'border-orange-200' : ''}>
				<Card.Header class="pb-3">
					<Card.Title class="flex items-center gap-2">
						{#if globalViewTab === 'overflow'}
							<span class="text-orange-600">เกินโควต้า</span>
						{:else if globalViewTab === 'all'}
							<Trophy class="w-5 h-5 text-yellow-500" />
							ผลรวมทุกสาย
						{:else}
							{globalRoomsSorted().find((r) => r.roomId === globalViewTab)?.roomName ?? ''}
						{/if}
					</Card.Title>
				</Card.Header>
				<div class="overflow-x-auto">
				<Table.Root>
					<Table.Header>
						<Table.Row>
							<Table.Head class="w-16 text-center">อันดับรวม</Table.Head>
							<Table.Head>เลขที่ใบสมัคร</Table.Head>
							<Table.Head>ชื่อ-สกุล</Table.Head>
							<Table.Head>สายที่สมัคร</Table.Head>
							<Table.Head class="text-center">คะแนนรวม</Table.Head>
							{#if globalViewTab !== 'overflow'}
								<Table.Head class="text-center">ห้องที่ได้</Table.Head>
								<Table.Head>ย้ายห้อง</Table.Head>
							{/if}
						</Table.Row>
					</Table.Header>
					<Table.Body>
						{@const tabApps = globalViewTab === 'overflow' ? globalOverflow : globalViewTab === 'all' ? globalAccepted : globalAccepted.filter((a) => a.assignedRoomId === globalViewTab)}
						{#each tabApps as app (app.applicationId)}
							<Table.Row class={globalViewTab === 'overflow' ? 'bg-orange-50' : ''}>
								<Table.Cell class="text-center">
									<span class="inline-flex items-center justify-center w-7 h-7 rounded-full text-xs font-bold {app.globalRank === 1 ? 'bg-yellow-100 text-yellow-700' : app.globalRank <= 3 ? 'bg-gray-100 text-gray-700' : 'text-muted-foreground'}">
										{app.globalRank}
									</span>
								</Table.Cell>
								<Table.Cell class="font-mono text-xs">{app.applicationNumber ?? '-'}</Table.Cell>
								<Table.Cell class="font-medium">{app.fullName}</Table.Cell>
								<Table.Cell class="text-sm text-muted-foreground">{app.originalTrackName ?? '-'}</Table.Cell>
								<Table.Cell class="text-center font-semibold text-primary">{app.totalScore.toFixed(1)}</Table.Cell>
								{#if globalViewTab !== 'overflow'}
									<Table.Cell class="text-center">
										{#if app.assignedRoom}
											<Badge variant="outline">{app.assignedRoom}</Badge>
										{:else}
											<span class="text-xs text-muted-foreground">-</span>
										{/if}
									</Table.Cell>
									<Table.Cell>
										{#if globalRanking && globalRanking.rooms.length > 1}
											<div class="flex items-center gap-1.5">
												<Select.Root
													type="single"
													value={moveTargetRoomId[app.applicationId] ?? ''}
													onValueChange={(v) => {
														moveTargetRoomId = { ...moveTargetRoomId, [app.applicationId]: v };
													}}
												>
													<Select.Trigger class="h-6 text-xs w-24 px-2">
														{globalRanking.rooms.find((r) => r.roomId === moveTargetRoomId[app.applicationId])?.roomName ?? 'ย้าย'}
													</Select.Trigger>
													<Select.Content>
														{#each globalRanking.rooms.filter((r) => r.roomName !== app.assignedRoom) as room (room.roomId)}
															<Select.Item value={room.roomId}>{room.roomName}</Select.Item>
														{/each}
													</Select.Content>
												</Select.Root>
												{#if moveTargetRoomId[app.applicationId]}
													<Button
														size="sm"
														class="h-6 text-xs px-2"
														disabled={movingRoom[app.applicationId]}
														onclick={() => moveRoomGlobal(app.applicationId)}
													>
														{#if movingRoom[app.applicationId]}
															<LoaderCircle class="w-3 h-3 animate-spin" />
														{:else}
															ย้าย
														{/if}
													</Button>
												{/if}
											</div>
										{/if}
									</Table.Cell>
								{/if}
							</Table.Row>
						{/each}
					</Table.Body>
				</Table.Root>
				</div>
			</Card.Root>
		{/if}
	{/if}
</div>

<!-- Dialogs -->
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
			<Dialog.Title>ยืนยันการจัดห้องทุกสาย</Dialog.Title>
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
				นักเรียนทุกสายจะถูกนำมาเรียงคะแนนรวมด้วยกัน แล้วจัดลงห้องตามลำดับที่กำหนดไว้
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

<Dialog.Root bind:open={showResetAllDialog}>
	<Dialog.Content class="sm:max-w-[420px]">
		<Dialog.Header>
			<Dialog.Title>ล้างการจัดห้องทั้งหมด</Dialog.Title>
			<Dialog.Description>
				การดำเนินการนี้จะ<strong>ลบการจัดห้องทุกคนในรอบนี้</strong>ออกทั้งหมด
				ทั้งที่จัดแบบแยกตามสายและรวมทุกคน ต้องการดำเนินการต่อหรือไม่?
			</Dialog.Description>
		</Dialog.Header>
		<Dialog.Footer class="flex-col sm:flex-row gap-2">
			<Button variant="outline" onclick={() => (showResetAllDialog = false)}>ยกเลิก</Button>
			<Button variant="destructive" onclick={() => { showResetAllDialog = false; handleResetAll(); }}>ล้างทั้งหมด</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
