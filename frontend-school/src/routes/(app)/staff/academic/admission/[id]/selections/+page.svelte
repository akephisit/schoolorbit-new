<script lang="ts">
	import { untrack } from 'svelte';
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
		resetAllRoomAssignments,
		type AdmissionRound,
		type AdmissionTrack,
		type AdmissionExamSubject,
		type TrackRankingResult,
		type GlobalRankingResult,
		type RoomBasic
	} from '#lib/api/admission.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import MobileDragDropPolyfill from '#lib/components/MobileDragDropPolyfill.svelte';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageSkeleton, PageState, RegionUpdatingState } from '#lib/components/app-state/index.js';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import * as RadioGroup from '#lib/components/ui/radio-group/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';
	import { toast } from 'svelte-sonner';
	import {
		Trophy,
		Check,
		CheckCircle2,
		LoaderCircle,
		RotateCcw,
		Trash2,
		GripVertical,
		Layers,
		Columns2
	} from '@lucide/svelte';

	let { data }: PageProps = $props();
	let id = $derived(data.id);
	const canScoreAdmission = $derived($can.has(PERMISSIONS.ADMISSION_SCORES_ALL));
	const roundRequest = new LatestRequest();
	const tracksRequest = new LatestRequest();
	const subjectsRequest = new LatestRequest();
	const rankingRequest = new LatestRequest();
	const globalRequest = new LatestRequest();
	const roomsRequest = new LatestRequest();
	let renderedId = '';
	let modeChosenByUser = false;

	let round: AdmissionRound | null = $state(null);
	let roundLoaded = $state(false);
	let roundLoading = $state(true);
	let roundError = $state('');
	let tracks: AdmissionTrack[] = $state([]);
	let tracksLoaded = $state(false);
	let tracksLoading = $state(true);
	let tracksError = $state('');
	let subjects: AdmissionExamSubject[] = $state([]);
	let subjectsLoaded = $state(false);
	let subjectsLoading = $state(true);
	let subjectsError = $state('');
	let selectedTrack = $state('');
	let selectedSubjectIdsByTrack: Record<string, string[]> = $state({});
	let currentSubjectIds = $derived(
		selectedSubjectIdsByTrack[selectedTrack] ?? subjects.map((s) => s.id)
	);
	let roomAssignmentMethodByTrack: Record<string, 'sequential' | 'round_robin'> = $state({});
	let currentMethod = $derived(roomAssignmentMethodByTrack[selectedTrack] ?? 'sequential');
	let ranking = $state<TrackRankingResult | null>(null);
	let loading = $state(false);
	let rankingLoaded = $state(false);
	let rankingError = $state('');
	let rankingKey = '';
	let assigning = $state(false);
	let assigned = $state(false);
	let assignedTracks = $state(new Set<string>());
	let moveTargetTrackId: Record<string, string> = $state({});
	let moving: Record<string, boolean> = $state({});
	let moveTargetRoomId: Record<string, string> = $state({});
	let movingRoom: Record<string, boolean> = $state({});
	let assignmentMode = $state<'per_track' | 'global'>('per_track');

	// Global mode state
	let globalRanking = $state<GlobalRankingResult | null>(null);
	let loadingGlobal = $state(false);
	let globalLoaded = $state(false);
	let globalError = $state('');
	let globalViewTab = $state('all'); // 'all' | roomId | 'overflow'
	let roomOrderForGlobal = $state<RoomBasic[]>([]);
	let loadingRooms = $state(false);
	let roomsLoaded = $state(false);
	let roomsError = $state('');
	let dragIndex = $state<number | null>(null);

	// Dialog state
	let showAssignDialog = $state(false);
	let showAssignAllDialog = $state(false);
	let showAssignGlobalDialog = $state(false);
	let resettingAll = $state(false);
	let showResetAllDialog = $state(false);
	let assigningAll = $state(false);
	let assigningGlobal = $state(false);
	let assignAllProgress = $state({ done: 0, total: 0 });
	let settingsLoaded = $state(false);
	let saveSettingsTimer: ReturnType<typeof setTimeout> | null = null;
	let pendingSettings: {
		roundId: string;
		patch: Parameters<typeof updateSelectionSettings>[1];
	} | null = null;
	let settingsWrite: Promise<void> = Promise.resolve();
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
		return [...rooms].sort(
			(a, b) => (orderMap.get(a.roomId) ?? 999) - (orderMap.get(b.roomId) ?? 999)
		);
	});

	function applyRound(value: AdmissionRound) {
		round = value;
		const saved = value.selectionSettings;
		selectedSubjectIdsByTrack = saved?.subjectsByTrack ?? {};
		roomAssignmentMethodByTrack = (saved?.methodByTrack ?? {}) as Record<
			string,
			'sequential' | 'round_robin'
		>;
		if (!modeChosenByUser)
			assignmentMode = saved?.assignmentMode === 'global' ? 'global' : 'per_track';
		settingsLoaded = true;
	}

	function applyTracks(rows: AdmissionTrack[]) {
		tracks = rows;
		if (!rows.some((track) => track.id === selectedTrack)) selectedTrack = rows[0]?.id ?? '';
	}

	async function loadRound() {
		if (!id || !canScoreAdmission) return;
		const sourceId = id;
		const { revision, signal } = roundRequest.begin();
		roundLoading = true;
		roundError = '';
		try {
			const value = await getRound(sourceId, { signal });
			if (!roundRequest.isCurrent(revision) || sourceId !== id) return;
			applyRound(value);
			roundLoaded = true;
			if (assignmentMode === 'global') {
				if (!roomsLoaded && !loadingRooms) void loadRoomsForGlobal();
				if (!globalLoaded && !loadingGlobal) void loadGlobalRanking();
			} else if (tracksLoaded && subjectsLoaded) void loadRanking();
		} catch (cause) {
			if (!isAbortError(cause) && roundRequest.isCurrent(revision))
				roundError = cause instanceof Error ? cause.message : 'โหลดรอบรับสมัครไม่สำเร็จ';
		} finally {
			if (roundRequest.isCurrent(revision)) roundLoading = false;
		}
	}

	async function loadTracks() {
		if (!id || !canScoreAdmission) return;
		const sourceId = id;
		const { revision, signal } = tracksRequest.begin();
		tracksLoading = true;
		tracksError = '';
		try {
			const value = await listTracks(sourceId, { signal });
			if (!tracksRequest.isCurrent(revision) || sourceId !== id) return;
			applyTracks(value);
			tracksLoaded = true;
			if (roundLoaded && subjectsLoaded && assignmentMode === 'per_track') void loadRanking();
		} catch (cause) {
			if (!isAbortError(cause) && tracksRequest.isCurrent(revision))
				tracksError = cause instanceof Error ? cause.message : 'โหลดสายการเรียนไม่สำเร็จ';
		} finally {
			if (tracksRequest.isCurrent(revision)) tracksLoading = false;
		}
	}

	async function loadSubjects() {
		if (!id || !canScoreAdmission) return;
		const sourceId = id;
		const { revision, signal } = subjectsRequest.begin();
		subjectsLoading = true;
		subjectsError = '';
		try {
			const value = await listSubjects(sourceId, { signal });
			if (!subjectsRequest.isCurrent(revision) || sourceId !== id) return;
			subjects = value;
			subjectsLoaded = true;
			if (roundLoaded && tracksLoaded && assignmentMode === 'per_track') void loadRanking();
		} catch (cause) {
			if (!isAbortError(cause) && subjectsRequest.isCurrent(revision))
				subjectsError = cause instanceof Error ? cause.message : 'โหลดวิชาสอบไม่สำเร็จ';
		} finally {
			if (subjectsRequest.isCurrent(revision)) subjectsLoading = false;
		}
	}

	function applyRanking(value: TrackRankingResult, trackId: string) {
		ranking = value;
		assigned = value.applications.some((app) => app.roomSaved);
		if (assigned) assignedTracks = new Set([...assignedTracks, trackId]);
	}

	async function loadRanking() {
		if (!selectedTrack || !canScoreAdmission || assignmentMode !== 'per_track') return;
		const sourceId = id;
		const trackId = selectedTrack;
		const subjectIds = [...currentSubjectIds];
		const method = currentMethod;
		const key = `${sourceId}:${trackId}:${subjectIds.join(',')}:${method}`;
		const { revision, signal } = rankingRequest.begin();
		if (rankingKey !== key) {
			ranking = null;
			rankingLoaded = false;
			assigned = false;
		}
		rankingKey = key;
		loading = true;
		rankingError = '';
		try {
			const value = await getTrackRanking(trackId, subjectIds, method, { signal });
			if (
				!rankingRequest.isCurrent(revision) ||
				sourceId !== id ||
				trackId !== selectedTrack ||
				assignmentMode !== 'per_track'
			)
				return;
			applyRanking(value, trackId);
			rankingLoaded = true;
		} catch (cause) {
			if (!isAbortError(cause) && rankingRequest.isCurrent(revision))
				rankingError = cause instanceof Error ? cause.message : 'โหลดผลเรียงคะแนนไม่สำเร็จ';
		} finally {
			if (rankingRequest.isCurrent(revision)) loading = false;
		}
	}

	async function loadRoomsForGlobal() {
		if (!id || !canScoreAdmission || assignmentMode !== 'global') return;
		const sourceId = id;
		const { revision, signal } = roomsRequest.begin();
		loadingRooms = true;
		roomsError = '';
		try {
			const value = await getRoomsForRound(sourceId, { signal });
			if (!roomsRequest.isCurrent(revision) || sourceId !== id || assignmentMode !== 'global')
				return;
			roomOrderForGlobal = value;
			roomsLoaded = true;
		} catch (cause) {
			if (!isAbortError(cause) && roomsRequest.isCurrent(revision))
				roomsError = cause instanceof Error ? cause.message : 'โหลดรายการห้องไม่สำเร็จ';
		} finally {
			if (roomsRequest.isCurrent(revision)) loadingRooms = false;
		}
	}

	async function loadGlobalRanking() {
		if (!id || !canScoreAdmission || assignmentMode !== 'global') return;
		const sourceId = id;
		const { revision, signal } = globalRequest.begin();
		loadingGlobal = true;
		globalError = '';
		try {
			const value = await getGlobalRanking(sourceId, { signal });
			if (!globalRequest.isCurrent(revision) || sourceId !== id || assignmentMode !== 'global')
				return;
			globalRanking = value;
			globalLoaded = true;
		} catch (cause) {
			if (!isAbortError(cause) && globalRequest.isCurrent(revision))
				globalError = cause instanceof Error ? cause.message : 'โหลดผลรวมไม่สำเร็จ';
		} finally {
			if (globalRequest.isCurrent(revision)) loadingGlobal = false;
		}
	}

	function switchMode(mode: 'per_track' | 'global') {
		if (!canScoreAdmission || assignmentMode === mode) return;
		modeChosenByUser = true;
		assignmentMode = mode;
		if (mode === 'global') {
			rankingRequest.abort();
			loading = false;
			if (!roomsLoaded && !loadingRooms) void loadRoomsForGlobal();
			if (!globalLoaded && !loadingGlobal) void loadGlobalRanking();
		} else {
			roomsRequest.abort();
			globalRequest.abort();
			loadingRooms = false;
			loadingGlobal = false;
			if (!rankingLoaded && selectedTrack && subjectsLoaded) void loadRanking();
		}
	}

	function queueSettingsUpdate(
		roundId: string,
		patch: Parameters<typeof updateSelectionSettings>[1]
	) {
		settingsWrite = settingsWrite
			.then(() => updateSelectionSettings(roundId, patch))
			.catch((cause) => {
				if (roundId === id)
					toast.error(cause instanceof Error ? cause.message : 'บันทึกการตั้งค่าไม่สำเร็จ');
			});
		return settingsWrite;
	}

	function flushSettingsSave() {
		if (saveSettingsTimer) clearTimeout(saveSettingsTimer);
		saveSettingsTimer = null;
		const pending = pendingSettings;
		pendingSettings = null;
		if (pending) void queueSettingsUpdate(pending.roundId, pending.patch);
	}

	function scheduleSettingsSave() {
		if (!settingsLoaded || !id) return;
		if (saveSettingsTimer) clearTimeout(saveSettingsTimer);
		pendingSettings = {
			roundId: id,
			patch: {
				subjectsByTrack: { ...selectedSubjectIdsByTrack },
				methodByTrack: { ...roomAssignmentMethodByTrack }
			}
		};
		saveSettingsTimer = setTimeout(flushSettingsSave, 500);
	}

	function persistAssignmentMode(mode: 'per_track' | 'global') {
		flushSettingsSave();
		if (id) void queueSettingsUpdate(id, { assignmentMode: mode });
	}

	function chooseTrack(trackId: string) {
		if (selectedTrack === trackId) return;
		selectedTrack = trackId;
		void loadRanking();
	}

	function setSubjectSelection(subjectId: string, checked: boolean) {
		const current =
			selectedSubjectIdsByTrack[selectedTrack] ?? subjects.map((subject) => subject.id);
		selectedSubjectIdsByTrack = {
			...selectedSubjectIdsByTrack,
			[selectedTrack]: checked ? [...current, subjectId] : current.filter((id) => id !== subjectId)
		};
		scheduleSettingsSave();
		void loadRanking();
	}

	function setRoomMethod(method: 'sequential' | 'round_robin') {
		roomAssignmentMethodByTrack = { ...roomAssignmentMethodByTrack, [selectedTrack]: method };
		scheduleSettingsSave();
		void loadRanking();
	}

	async function confirmAssignRooms() {
		showAssignDialog = false;
		if (!id || !selectedTrack || !canScoreAdmission) return;
		const sourceId = id;
		const sourceTrack = selectedTrack;
		const sourceSubjects = [...currentSubjectIds];
		const sourceMethod = currentMethod;
		assigning = true;
		try {
			await assignRooms(sourceId, sourceTrack, sourceSubjects, sourceMethod);
			if (sourceId !== id) return;
			toast.success('จัดห้องสำเร็จ!');
			assignedTracks = new Set([...assignedTracks, sourceTrack]);
			persistAssignmentMode('per_track');
			if (sourceTrack === selectedTrack && assignmentMode === 'per_track') {
				assigned = true;
				await loadRanking();
			}
		} catch (e) {
			if (sourceId === id) toast.error(e instanceof Error ? e.message : 'จัดห้องไม่สำเร็จ');
		} finally {
			if (sourceId === id) assigning = false;
		}
	}

	async function confirmAssignAll() {
		showAssignAllDialog = false;
		if (!id || tracks.length === 0 || !canScoreAdmission) return;
		const sourceId = id;
		const sourceTracks = [...tracks];
		const sourceSubjects = { ...selectedSubjectIdsByTrack };
		const sourceMethods = { ...roomAssignmentMethodByTrack };
		const allSubjectIds = subjects.map((subject) => subject.id);
		assigningAll = true;
		assignAllProgress = { done: 0, total: sourceTracks.length };
		let failed = 0;
		for (const track of sourceTracks) {
			if (sourceId !== id) return;
			try {
				await assignRooms(
					sourceId,
					track.id,
					sourceSubjects[track.id] ?? allSubjectIds,
					sourceMethods[track.id] ?? 'sequential'
				);
				if (sourceId !== id) return;
				assignAllProgress = { ...assignAllProgress, done: assignAllProgress.done + 1 };
			} catch {
				failed++;
			}
		}
		if (sourceId !== id) return;
		assigningAll = false;
		if (failed === 0) {
			toast.success(`จัดห้องทุกสายสำเร็จ (${sourceTracks.length} สาย)`);
			persistAssignmentMode('per_track');
		} else {
			toast.warning(`จัดห้องสำเร็จ ${sourceTracks.length - failed} สาย, ล้มเหลว ${failed} สาย`);
		}
		if (assignmentMode === 'per_track') await loadRanking();
	}

	async function confirmAssignGlobal() {
		showAssignGlobalDialog = false;
		if (!id || !canScoreAdmission) return;
		const sourceId = id;
		const sourceRoomIds = roomOrderForGlobal.map((room) => room.roomId);
		assigningGlobal = true;
		try {
			await assignRoomsGlobal(sourceId, 'sequential', sourceRoomIds);
			if (sourceId !== id) return;
			toast.success('จัดห้องรวมทุกสายสำเร็จ!');
			assigned = false;
			globalViewTab = 'all';
			persistAssignmentMode('global');
			if (assignmentMode === 'global') await loadGlobalRanking();
		} catch (e) {
			if (sourceId === id)
				toast.error(e instanceof Error ? e.message : 'จัดห้องรวมทุกสายไม่สำเร็จ');
		} finally {
			if (sourceId === id) assigningGlobal = false;
		}
	}

	async function moveToTrack(appId: string) {
		if (!canScoreAdmission) return;
		const sourceId = id;
		const sourceTrack = selectedTrack;
		const targetId = moveTargetTrackId[appId];
		if (!targetId) return;
		moving = { ...moving, [appId]: true };
		try {
			await changeApplicationTrack(appId, targetId);
			if (sourceId !== id || sourceTrack !== selectedTrack) return;
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
			if (sourceId === id) toast.error(e instanceof Error ? e.message : 'ย้ายสายไม่สำเร็จ');
		} finally {
			if (sourceId === id) moving = { ...moving, [appId]: false };
		}
	}

	async function revertTrack(appId: string) {
		if (!canScoreAdmission) return;
		const sourceId = id;
		const sourceTrack = selectedTrack;
		reverting = { ...reverting, [appId]: true };
		try {
			await changeApplicationTrack(appId, null);
			if (sourceId !== id || sourceTrack !== selectedTrack) return;
			if (ranking) {
				ranking = {
					...ranking,
					applications: ranking.applications.filter((a) => a.applicationId !== appId)
				};
				assigned = false;
			}
			toast.success('ย้อนกลับสายเดิมสำเร็จ');
		} catch (e) {
			if (sourceId === id) toast.error(e instanceof Error ? e.message : 'ย้อนกลับไม่สำเร็จ');
		} finally {
			if (sourceId === id) reverting = { ...reverting, [appId]: false };
		}
	}

	async function moveRoomGlobal(appId: string) {
		if (!canScoreAdmission) return;
		const sourceId = id;
		const targetRoomId = moveTargetRoomId[appId];
		if (!targetRoomId || !globalRanking) return;
		movingRoom = { ...movingRoom, [appId]: true };
		try {
			await moveApplicationRoom(appId, targetRoomId);
			if (sourceId !== id || assignmentMode !== 'global' || !globalRanking) return;
			moveTargetRoomId = { ...moveTargetRoomId, [appId]: '' };
			toast.success('ย้ายห้องสำเร็จ');
			// optimistic update — ไม่ต้อง fetch ใหม่
			const movedApp = globalRanking.applications.find((a) => a.applicationId === appId);
			const oldRoomId = movedApp?.assignedRoomId;
			const isMale = movedApp?.gender?.toLowerCase() === 'male' || movedApp?.gender === 'ชาย';
			const isFemale = movedApp?.gender?.toLowerCase() === 'female' || movedApp?.gender === 'หญิง';
			const targetRoom = roomOrderForGlobal.find((r) => r.roomId === targetRoomId);
			// อัปเดต assignedRoomId ของคนที่ย้าย
			const appsAfterMove = globalRanking.applications.map((a) =>
				a.applicationId === appId
					? {
							...a,
							assignedRoomId: targetRoomId,
							assignedRoom: targetRoom?.roomName ?? targetRoomId
						}
					: a
			);
			// recalculate rankInRoom สำหรับห้องเก่าและห้องใหม่
			const affectedRooms = new Set([oldRoomId, targetRoomId]);
			const roomRankCounters: Record<string, number> = {};
			const apps = appsAfterMove.map((a) => {
				if (!a.assignedRoomId || !affectedRooms.has(a.assignedRoomId)) return a;
				roomRankCounters[a.assignedRoomId] = (roomRankCounters[a.assignedRoomId] ?? 0) + 1;
				return { ...a, rankInRoom: roomRankCounters[a.assignedRoomId] };
			});
			const rooms = globalRanking.rooms.map((r) => {
				if (r.roomId === oldRoomId)
					return {
						...r,
						studentCount: Math.max(0, r.studentCount - 1),
						maleCount: isMale ? Math.max(0, r.maleCount - 1) : r.maleCount,
						femaleCount: isFemale ? Math.max(0, r.femaleCount - 1) : r.femaleCount
					};
				if (r.roomId === targetRoomId)
					return {
						...r,
						studentCount: r.studentCount + 1,
						maleCount: isMale ? r.maleCount + 1 : r.maleCount,
						femaleCount: isFemale ? r.femaleCount + 1 : r.femaleCount
					};
				return r;
			});
			globalRanking = { ...globalRanking, applications: apps, rooms };
		} catch (e) {
			if (sourceId === id) toast.error(e instanceof Error ? e.message : 'ย้ายห้องไม่สำเร็จ');
		} finally {
			if (sourceId === id) movingRoom = { ...movingRoom, [appId]: false };
		}
	}

	async function handleResetAll() {
		if (!id || !canScoreAdmission) return;
		const sourceId = id;
		resettingAll = true;
		try {
			await resetAllRoomAssignments(sourceId);
			if (sourceId !== id) return;
			toast.success('ล้างการจัดห้องทั้งหมดสำเร็จ');
			ranking = null;
			rankingLoaded = false;
			globalRanking = null;
			globalLoaded = false;
			assigned = false;
			assignedTracks = new Set();
			if (assignmentMode === 'global') await loadGlobalRanking();
			else await loadRanking();
		} catch (e) {
			if (sourceId === id) toast.error(e instanceof Error ? e.message : 'ล้างการจัดห้องไม่สำเร็จ');
		} finally {
			if (sourceId === id) resettingAll = false;
		}
	}

	async function moveRoom(appId: string) {
		if (!canScoreAdmission) return;
		const sourceId = id;
		const sourceTrack = selectedTrack;
		const targetRoomId = moveTargetRoomId[appId];
		if (!targetRoomId || !ranking) return;
		movingRoom = { ...movingRoom, [appId]: true };
		try {
			await moveApplicationRoom(appId, targetRoomId);
			if (
				sourceId !== id ||
				sourceTrack !== selectedTrack ||
				assignmentMode !== 'per_track' ||
				!ranking
			)
				return;
			moveTargetRoomId = { ...moveTargetRoomId, [appId]: '' };
			toast.success('ย้ายห้องสำเร็จ');
			// optimistic update — ไม่ต้อง fetch ใหม่
			const movedApp = ranking.applications.find((a) => a.applicationId === appId);
			const oldRoomId = movedApp?.assignedRoomId;
			const isMale = movedApp?.gender?.toLowerCase() === 'male' || movedApp?.gender === 'ชาย';
			const isFemale = movedApp?.gender?.toLowerCase() === 'female' || movedApp?.gender === 'หญิง';
			const targetRoom = ranking.rooms.find((r) => r.roomId === targetRoomId);
			const apps = ranking.applications.map((a) =>
				a.applicationId === appId
					? {
							...a,
							assignedRoomId: targetRoomId,
							assignedRoom: targetRoom?.roomName ?? targetRoomId
						}
					: a
			);
			const rooms = ranking.rooms.map((r) => {
				if (r.roomId === oldRoomId)
					return {
						...r,
						studentCount: Math.max(0, r.studentCount - 1),
						maleCount: isMale ? Math.max(0, r.maleCount - 1) : r.maleCount,
						femaleCount: isFemale ? Math.max(0, r.femaleCount - 1) : r.femaleCount
					};
				if (r.roomId === targetRoomId)
					return {
						...r,
						studentCount: r.studentCount + 1,
						maleCount: isMale ? r.maleCount + 1 : r.maleCount,
						femaleCount: isFemale ? r.femaleCount + 1 : r.femaleCount
					};
				return r;
			});
			ranking = { ...ranking, applications: apps, rooms };
		} catch (e) {
			if (sourceId === id) toast.error(e instanceof Error ? e.message : 'ย้ายห้องไม่สำเร็จ');
		} finally {
			if (sourceId === id) movingRoom = { ...movingRoom, [appId]: false };
		}
	}

	$effect.pre(() => {
		const routeId = data.id;
		const routeRound = data.round;
		const routeTracks = data.tracks;
		const routeSubjects = data.subjects;
		const routeTrackRanking = data.trackRanking;
		const routeGlobalRanking = data.globalRanking;
		const routeRoomOrder = data.roomOrder;
		const { revision: roundRevision } = roundRequest.begin();
		const { revision: tracksRevision } = tracksRequest.begin();
		const { revision: subjectsRevision } = subjectsRequest.begin();
		const { revision: rankingRevision } = rankingRequest.begin();
		const { revision: globalRevision } = globalRequest.begin();
		const { revision: roomsRevision } = roomsRequest.begin();
		untrack(() => {
			flushSettingsSave();
			if (renderedId !== routeId) {
				renderedId = routeId;
				round = null;
				roundLoaded = false;
				tracks = [];
				tracksLoaded = false;
				subjects = [];
				subjectsLoaded = false;
				selectedTrack = '';
				selectedSubjectIdsByTrack = {};
				roomAssignmentMethodByTrack = {};
				assignmentMode = 'per_track';
				modeChosenByUser = false;
				settingsLoaded = false;
				ranking = null;
				rankingLoaded = false;
				rankingKey = '';
				globalRanking = null;
				globalLoaded = false;
				roomOrderForGlobal = [];
				roomsLoaded = false;
				globalViewTab = 'all';
				assigned = false;
				assignedTracks = new Set();
				moveTargetTrackId = {};
				moveTargetRoomId = {};
				moving = {};
				movingRoom = {};
				reverting = {};
				assigning = false;
				assigningAll = false;
				assigningGlobal = false;
				resettingAll = false;
				showAssignDialog = false;
				showAssignAllDialog = false;
				showAssignGlobalDialog = false;
				showResetAllDialog = false;
			}
			roundLoading = true;
			tracksLoading = true;
			subjectsLoading = true;
			loading = true;
			loadingGlobal = true;
			loadingRooms = true;
			roundError = '';
			tracksError = '';
			subjectsError = '';
			rankingError = '';
			globalError = '';
			roomsError = '';
		});
		void routeRound.then((result) => {
			if (!roundRequest.isCurrent(roundRevision)) return;
			untrack(() => {
				if (result.ok && result.data) {
					applyRound(result.data);
					roundLoaded = true;
				} else if (!result.ok) roundError = result.error;
				roundLoading = false;
			});
		});
		void routeTracks.then((result) => {
			if (!tracksRequest.isCurrent(tracksRevision)) return;
			untrack(() => {
				if (result.ok && result.data) {
					applyTracks(result.data);
					tracksLoaded = true;
				} else if (!result.ok) tracksError = result.error;
				tracksLoading = false;
			});
		});
		void routeSubjects.then((result) => {
			if (!subjectsRequest.isCurrent(subjectsRevision)) return;
			untrack(() => {
				if (result.ok && result.data) {
					subjects = result.data;
					subjectsLoaded = true;
				} else if (!result.ok) subjectsError = result.error;
				subjectsLoading = false;
			});
		});
		void routeTrackRanking.then((result) => {
			if (!rankingRequest.isCurrent(rankingRevision)) return;
			untrack(() => {
				if (result.ok && result.data && assignmentMode === 'per_track') {
					selectedTrack = result.data.trackId;
					rankingKey = `${routeId}:${result.data.trackId}:${(selectedSubjectIdsByTrack[result.data.trackId] ?? subjects.map((s) => s.id)).join(',')}:${roomAssignmentMethodByTrack[result.data.trackId] ?? 'sequential'}`;
					applyRanking(result.data.result, result.data.trackId);
					rankingLoaded = true;
				} else if (!result.ok && assignmentMode === 'per_track') rankingError = result.error;
				loading = false;
			});
		});
		void routeGlobalRanking.then((result) => {
			if (!globalRequest.isCurrent(globalRevision)) return;
			untrack(() => {
				if (result.ok && result.data && assignmentMode === 'global') {
					globalRanking = result.data;
					globalLoaded = true;
				} else if (!result.ok && assignmentMode === 'global') globalError = result.error;
				loadingGlobal = false;
			});
		});
		void routeRoomOrder.then((result) => {
			if (!roomsRequest.isCurrent(roomsRevision)) return;
			untrack(() => {
				if (result.ok && result.data && assignmentMode === 'global') {
					roomOrderForGlobal = result.data;
					roomsLoaded = true;
				} else if (!result.ok && assignmentMode === 'global') roomsError = result.error;
				loadingRooms = false;
			});
		});
		return () => {
			roundRequest.abort();
			tracksRequest.abort();
			subjectsRequest.abort();
			rankingRequest.abort();
			globalRequest.abort();
			roomsRequest.abort();
			flushSettingsSave();
		};
	});
</script>

<MobileDragDropPolyfill />

<PageShell
	title="จัดห้องเรียน (เรียงคะแนน)"
	description={round?.name ?? 'จัดผลคัดเลือกและกำหนดห้องเรียน'}
	backHref={`/staff/academic/admission/${id}`}
>
	{#if !canScoreAdmission}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์จัดผลคัดเลือก"
			description="หน้านี้ต้องใช้สิทธิ์จัดการคะแนนและผลคัดเลือกของงานรับสมัคร"
		/>
	{:else}
		<!-- Step 1: เลือกโหมดจัดห้อง -->
		{#if roundError}
			<PageState
				variant="error"
				title="โหลดรอบรับสมัครไม่สำเร็จ"
				description={roundError}
				actionLabel="ลองอีกครั้ง"
				onaction={loadRound}
			/>
		{/if}
		{#if roundLoading && !roundLoaded}
			<PageSkeleton variant="cards" rows={2} />
		{:else if roundLoaded}
			{#if roundLoading}<RegionUpdatingState
					class="static"
					label="กำลังอัปเดตรอบรับสมัคร..."
				/>{/if}
			<Card.Root class="gap-0 py-0">
				<Card.Content class="pt-4 pb-4 space-y-3">
					<div class="flex items-center justify-between">
						<p class="text-sm font-semibold text-muted-foreground uppercase tracking-wide">
							โหมดจัดห้อง
						</p>
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
							class="flex items-start gap-3 rounded-lg border-2 p-4 text-left transition-colors {assignmentMode ===
							'per_track'
								? 'border-primary bg-primary/5'
								: 'border-border hover:border-muted-foreground/40'}"
						>
							<Columns2
								class="w-5 h-5 mt-0.5 shrink-0 {assignmentMode === 'per_track'
									? 'text-primary'
									: 'text-muted-foreground'}"
							/>
							<div>
								<p class="font-semibold text-sm">แยกตามสาย</p>
								<p class="text-xs text-muted-foreground mt-0.5">
									แต่ละสายคัดเลือกและจัดห้องเองแยกกัน ใช้วิชาและเกณฑ์ของแต่ละสาย
								</p>
							</div>
						</button>
						<button
							onclick={() => switchMode('global')}
							class="flex items-start gap-3 rounded-lg border-2 p-4 text-left transition-colors {assignmentMode ===
							'global'
								? 'border-primary bg-primary/5'
								: 'border-border hover:border-muted-foreground/40'}"
						>
							<Layers
								class="w-5 h-5 mt-0.5 shrink-0 {assignmentMode === 'global'
									? 'text-primary'
									: 'text-muted-foreground'}"
							/>
							<div>
								<p class="font-semibold text-sm">รวมทุกคน</p>
								<p class="text-xs text-muted-foreground mt-0.5">
									นำคะแนนทุกคนทุกสายมาเรียงรวมกัน แล้วจัดลงห้องทั้งหมดโดยไม่แยกสาย
								</p>
							</div>
						</button>
					</div>
				</Card.Content>
			</Card.Root>

			<!-- ========== โหมด: แยกตามสาย ========== -->
			{#if assignmentMode === 'per_track'}
				<!-- Track Selector -->
				{#if tracksError}
					<PageState
						variant="error"
						title="โหลดสายการเรียนไม่สำเร็จ"
						description={tracksError}
						actionLabel="ลองอีกครั้ง"
						onaction={loadTracks}
					/>
				{/if}
				{#if tracksLoading && !tracksLoaded}
					<PageSkeleton variant="cards" rows={1} />
				{:else if tracksLoaded}
					{#if tracksLoading}<RegionUpdatingState
							class="static"
							label="กำลังอัปเดตสายการเรียน..."
						/>{/if}
					<Card.Root class="gap-0 py-0">
						<Card.Content class="pt-4 pb-4 flex items-center gap-4 flex-wrap">
							<p class="text-sm font-medium shrink-0">สาย:</p>
							<div class="flex gap-2 flex-wrap flex-1">
								{#each tracks as track (track.id)}
									<Button
										variant={selectedTrack === track.id ? 'default' : 'outline'}
										size="sm"
										onclick={() => chooseTrack(track.id)}
										class="gap-1.5"
									>
										{track.name}
										{#if assignedTracks.has(track.id)}
											<CheckCircle2
												class="w-3.5 h-3.5 {selectedTrack === track.id
													? 'text-white/80'
													: 'text-green-500'}"
											/>
										{/if}
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
				{/if}

				<!-- วิชาที่ใช้คัดผ่าน-ไม่ผ่าน -->
				{#if subjectsError}
					<PageState
						variant="error"
						title="โหลดวิชาสอบไม่สำเร็จ"
						description={subjectsError}
						actionLabel="ลองอีกครั้ง"
						onaction={loadSubjects}
					/>
				{/if}
				{#if subjectsLoading && !subjectsLoaded}
					<PageSkeleton variant="cards" rows={1} />
				{:else if subjects.length > 0}
					{#if subjectsLoading}<RegionUpdatingState
							class="static"
							label="กำลังอัปเดตวิชาสอบ..."
						/>{/if}
					<Card.Root class="gap-0 py-0">
						<Card.Content class="pt-4 pb-3 space-y-2">
							<p class="text-sm font-medium">วิชาที่ใช้คัดผ่าน-ไม่ผ่าน</p>
							<div class="flex flex-wrap gap-3">
								{#each subjects as s (s.id)}
									<div class="flex items-center gap-1.5">
										<Checkbox
											id="subj-{s.id}-{selectedTrack}"
											checked={currentSubjectIds.includes(s.id)}
											onCheckedChange={(v) => setSubjectSelection(s.id, v === true)}
										/>
										<Label
											for="subj-{s.id}-{selectedTrack}"
											class="font-normal cursor-pointer text-sm"
										>
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
				<Card.Root class="gap-0 py-0">
					<Card.Content class="pt-4 pb-3 space-y-2">
						<p class="text-sm font-medium">วิธีจัดห้อง</p>
						<RadioGroup.Root
							value={currentMethod}
							onValueChange={(v) => setRoomMethod(v as 'sequential' | 'round_robin')}
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
									<span class="text-xs text-muted-foreground"
										>(สลับห้องตามลำดับ ทุกห้องได้คนคะแนนสูง-ต่ำปนกัน)</span
									>
								</Label>
							</div>
						</RadioGroup.Root>
					</Card.Content>
				</Card.Root>

				<!-- ผลเรียงคะแนน -->
				{#if rankingError}
					<PageState
						variant="error"
						title="โหลดผลเรียงคะแนนไม่สำเร็จ"
						description={rankingError}
						actionLabel="ลองอีกครั้ง"
						onaction={loadRanking}
					/>
				{/if}
				{#if loading && !rankingLoaded}
					<PageSkeleton variant="table" rows={6} columns={7} />
				{:else if ranking}
					{#if loading}<RegionUpdatingState
							class="static"
							label="กำลังอัปเดตผลเรียงคะแนน..."
						/>{/if}
					<!-- Room Summary -->
					{#if ranking.rooms?.length > 0}
						<div class="grid grid-cols-2 sm:grid-cols-4 gap-3">
							{#each ranking.rooms as room (room.roomId)}
								<Card.Root class="gap-0 py-0">
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
								{#if assigned}
									<span class="flex items-center gap-1.5 text-sm text-green-600 font-medium px-2">
										<CheckCircle2 class="w-4 h-4" /> จัดห้องแล้ว
									</span>
								{:else}
									<Button
										onclick={() => (showAssignDialog = true)}
										disabled={assigning || acceptedApps.length === 0}
										class="gap-2"
									>
										{#if assigning}
											<LoaderCircle class="w-4 h-4 animate-spin" />
										{:else}
											<Check class="w-4 h-4" />
										{/if}
										{assigning ? 'กำลังจัดห้อง...' : 'บันทึกจัดห้อง'}
									</Button>
								{/if}
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
											<Table.Cell class="font-mono text-xs"
												>{app.applicationNumber ?? '-'}</Table.Cell
											>
											<Table.Cell class="font-medium">
												{app.fullName}
												{#if app.isTrackOverridden && app.originalTrackName}
													<span
														class="ml-1.5 inline-flex items-center gap-1 rounded-full bg-orange-100 px-2 py-0.5 text-xs font-medium text-orange-700"
													>
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
																{ranking.rooms.find(
																	(r) => r.roomId === moveTargetRoomId[app.applicationId]
																)?.roomName ?? 'ย้าย'}
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
														disabled={!moveTargetTrackId[app.applicationId] ||
															moving[app.applicationId]}
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
												<Table.Cell class="font-mono text-xs"
													>{app.applicationNumber ?? '-'}</Table.Cell
												>
												<Table.Cell class="font-medium">
													{app.fullName}
													{#if app.isTrackOverridden && app.originalTrackName}
														<span
															class="ml-1.5 inline-flex items-center gap-1 rounded-full bg-orange-200 px-2 py-0.5 text-xs font-medium text-orange-800"
														>
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
																moveTargetTrackId = {
																	...moveTargetTrackId,
																	[app.applicationId]: v
																};
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
															disabled={!moveTargetTrackId[app.applicationId] ||
																moving[app.applicationId]}
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
				{:else if !rankingError && tracksLoaded && subjectsLoaded}
					<PageState
						variant="empty"
						title="ยังไม่มีผลเรียงคะแนน"
						description="เลือกสายและวิชาที่ใช้คัดเลือกเพื่อตรวจผล"
					/>
				{/if}

				<!-- ========== โหมด: รวมทุกคน ========== -->
			{:else}
				<!-- DnD Room Ordering -->
				<Card.Root class="gap-0 py-0">
					<Card.Content class="pt-4 pb-4 space-y-3">
						<div class="flex items-center justify-between">
							<div>
								<p class="text-sm font-medium">ลำดับห้อง</p>
								<p class="text-xs text-muted-foreground">
									ลาก-วางเพื่อกำหนดว่าห้องไหนได้นักเรียนคะแนนสูงก่อน
								</p>
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

						{#if roomsError}
							<PageState
								variant="error"
								title="โหลดรายการห้องไม่สำเร็จ"
								description={roomsError}
								actionLabel="ลองอีกครั้ง"
								onaction={loadRoomsForGlobal}
							/>
						{/if}
						{#if loadingRooms && !roomsLoaded}
							<PageSkeleton variant="cards" rows={3} />
						{:else if roomOrderForGlobal.length === 0 && !roomsError}
							<p class="text-sm text-muted-foreground text-center py-6">ไม่พบห้องเรียนในรอบนี้</p>
						{:else if roomOrderForGlobal.length > 0}
							{#if loadingRooms}<RegionUpdatingState
									class="static"
									label="กำลังอัปเดตรายการห้อง..."
								/>{/if}
							<div class="space-y-2" role="list">
								{#each roomOrderForGlobal as room, i (room.roomId)}
									<div
										role="listitem"
										draggable={true}
										ondragstart={() => {
											dragIndex = i;
										}}
										ondragenter={() => {
											if (dragIndex === null || dragIndex === i) return;
											const newOrder = [...roomOrderForGlobal];
											const [moved] = newOrder.splice(dragIndex, 1);
											newOrder.splice(i, 0, moved);
											roomOrderForGlobal = newOrder;
											dragIndex = i;
										}}
										ondragend={() => {
											dragIndex = null;
										}}
										ondragover={(e) => e.preventDefault()}
										class="flex items-center gap-3 rounded-lg border bg-background px-3 py-2.5 transition-opacity {dragIndex ===
										i
											? 'opacity-50'
											: ''} cursor-grab active:cursor-grabbing touch-none"
										style="touch-action: none;"
									>
										<GripVertical class="w-4 h-4 text-muted-foreground shrink-0" />
										<span class="text-xs text-muted-foreground w-5 text-center shrink-0"
											>{i + 1}</span
										>
										<span class="font-medium text-sm flex-1">{room.roomName}</span>
										<span class="text-xs text-muted-foreground">รับ {room.capacity} คน</span>
									</div>
								{/each}
							</div>
						{/if}
					</Card.Content>
				</Card.Root>

				<!-- ผลจัดห้อง (tabs) -->
				{#if globalError}
					<PageState
						variant="error"
						title="โหลดผลรวมไม่สำเร็จ"
						description={globalError}
						actionLabel="ลองอีกครั้ง"
						onaction={loadGlobalRanking}
					/>
				{/if}
				{#if loadingGlobal && !globalLoaded}
					<PageSkeleton variant="table" rows={6} columns={7} />
				{:else if globalRanking}
					{#if loadingGlobal}<RegionUpdatingState class="static" label="กำลังอัปเดตผลรวม..." />{/if}
					<!-- Room summary cards -->
					{#if globalRanking.rooms.length > 0}
						<div class="grid grid-cols-2 sm:grid-cols-4 gap-3">
							{#each globalRoomsSorted() as room (room.roomId)}
								{@const isOver = room.studentCount > room.capacity}
								<Card.Root class={isOver ? 'gap-0 border-red-300 py-0' : 'gap-0 py-0'}>
									<Card.Content class="pt-3 pb-3 text-center space-y-1">
										<p class="font-semibold">{room.roomName}</p>
										{#if room.studentCount > 0}
											<p class="text-sm font-medium {isOver ? 'text-red-600' : ''}">
												{room.studentCount} / {room.capacity} คน
												{#if isOver}<span class="text-xs"
														>(เกิน {room.studentCount - room.capacity})</span
													>{/if}
											</p>
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
							{@const isOver = room.studentCount > room.capacity}
							<Button
								variant={globalViewTab === room.roomId ? 'default' : 'outline'}
								size="sm"
								onclick={() => (globalViewTab = room.roomId)}
								class={isOver && globalViewTab !== room.roomId
									? 'border-red-300 text-red-600 hover:bg-red-50'
									: ''}
							>
								{room.roomName} ({room.studentCount}/{room.capacity}){isOver ? ' ⚠' : ''}
							</Button>
						{/each}
						{#if globalOverflow.length > 0}
							<Button
								variant={globalViewTab === 'overflow' ? 'destructive' : 'outline'}
								size="sm"
								onclick={() => (globalViewTab = 'overflow')}
								class={globalViewTab !== 'overflow'
									? 'border-orange-300 text-orange-600 hover:bg-orange-50'
									: ''}
							>
								เกินโควต้า ({globalOverflow.length})
							</Button>
						{/if}
					</div>

					<!-- Table -->
					{#snippet appRow(app: (typeof globalAccepted)[0], isOrange: boolean)}
						<Table.Row class={isOrange ? 'bg-orange-50' : ''}>
							<Table.Cell class="text-center">
								<span
									class="inline-flex items-center justify-center w-7 h-7 rounded-full text-xs font-bold {app.globalRank ===
									1
										? 'bg-yellow-100 text-yellow-700'
										: app.globalRank <= 3
											? 'bg-gray-100 text-gray-700'
											: 'text-muted-foreground'}"
								>
									{app.globalRank}
								</span>
							</Table.Cell>
							{#if globalViewTab !== 'all' && globalViewTab !== 'overflow'}
								<Table.Cell class="text-center text-sm text-muted-foreground">
									{app.rankInRoom != null ? '#' + app.rankInRoom : '-'}
								</Table.Cell>
							{/if}
							<Table.Cell class="font-mono text-xs">{app.applicationNumber ?? '-'}</Table.Cell>
							<Table.Cell class="font-medium">{app.fullName}</Table.Cell>
							<Table.Cell class="text-sm text-muted-foreground"
								>{app.originalTrackName ?? '-'}</Table.Cell
							>
							<Table.Cell class="text-center font-semibold text-primary"
								>{app.totalScore.toFixed(1)}</Table.Cell
							>
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
													{globalRanking.rooms.find(
														(r) => r.roomId === moveTargetRoomId[app.applicationId]
													)?.roomName ?? 'ย้าย'}
												</Select.Trigger>
												<Select.Content>
													{#each globalRanking.rooms.filter((r) => r.roomName !== app.assignedRoom) as room (room.roomId)}
														<Select.Item value={room.roomId}>
															{room.roomName} ({room.studentCount}/{room.capacity}){room.studentCount >=
															room.capacity
																? ' ⚠'
																: ''}
														</Select.Item>
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
					{/snippet}
					{@const isRoomTab = globalViewTab !== 'all' && globalViewTab !== 'overflow'}
					{@const currentRoom = isRoomTab
						? globalRanking.rooms.find((r) => r.roomId === globalViewTab)
						: null}
					{@const roomStudents = isRoomTab
						? globalAccepted
								.filter((a) => a.assignedRoomId === globalViewTab)
								.sort((a, b) => a.globalRank - b.globalRank)
						: []}
					{@const roomNormal = isRoomTab
						? roomStudents.slice(0, currentRoom?.capacity ?? roomStudents.length)
						: []}
					{@const roomOver = isRoomTab
						? roomStudents.slice(currentRoom?.capacity ?? roomStudents.length)
						: []}
					{@const tabApps =
						globalViewTab === 'overflow'
							? globalOverflow
							: globalViewTab === 'all'
								? globalAccepted
								: roomNormal}
					<Card.Root class={globalViewTab === 'overflow' ? 'border-orange-200' : ''}>
						<Card.Header class="pb-3">
							<Card.Title class="flex items-center gap-2">
								{#if globalViewTab === 'overflow'}
									<span class="text-orange-600">เกินโควต้า (ไม่ได้รับห้อง)</span>
								{:else if globalViewTab === 'all'}
									<Trophy class="w-5 h-5 text-yellow-500" />
									ผลรวมทุกสาย
								{:else}
									{currentRoom?.roomName ?? ''} — {roomNormal.length} / {currentRoom?.capacity ?? 0} คน
								{/if}
							</Card.Title>
						</Card.Header>
						<div class="overflow-x-auto">
							<Table.Root>
								<Table.Header>
									<Table.Row>
										<Table.Head class="w-16 text-center">อันดับรวม</Table.Head>
										{#if globalViewTab !== 'all' && globalViewTab !== 'overflow'}
											<Table.Head class="w-14 text-center">อันดับในห้อง</Table.Head>
										{/if}
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
									{#each tabApps as app (app.applicationId)}
										{@render appRow(app, globalViewTab === 'overflow')}
									{/each}
									{#if isRoomTab && roomOver.length > 0}
										<Table.Row>
											<Table.Cell
												colspan={globalViewTab !== 'all' && globalViewTab !== 'overflow' ? 8 : 7}
												class="bg-orange-100 py-1.5 px-4"
											>
												<span class="text-xs font-semibold text-orange-700"
													>เกินโควต้าห้องนี้ ({roomOver.length} คน) — ย้ายไปห้องอื่น</span
												>
											</Table.Cell>
										</Table.Row>
										{#each roomOver as app (app.applicationId)}
											{@render appRow(app, true)}
										{/each}
									{/if}
								</Table.Body>
							</Table.Root>
						</div>
					</Card.Root>
				{/if}
			{/if}
		{/if}
	{/if}
</PageShell>

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
				จะจัดห้องให้ทุกสาย ({tracks.length} สาย) พร้อมกัน แต่ละสายใช้วิชาและวิธีจัดห้องของตัวเอง ผลจัดห้องเดิมของทุกสายจะถูกแทนที่
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
			<Button
				variant="destructive"
				onclick={() => {
					showResetAllDialog = false;
					handleResetAll();
				}}>ล้างทั้งหมด</Button
			>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
