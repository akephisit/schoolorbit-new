<script lang="ts">
	import { untrack } from 'svelte';
	import type { PageProps } from './$types';
	import {
		getRound,
		listTracks,
		listSubjects,
		listApplications,
		bulkUpdateScores,
		getScoreRoomRoster,
		markAbsent,
		type AdmissionRound,
		type AdmissionTrack,
		type AdmissionExamSubject,
		type ApplicationListItem,
		type ScoreRoomGroup,
		type RawScore,
		getAllScores
	} from '$lib/api/admission';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { RegionUpdatingState } from '$lib/components/app-state';
	import { LatestRequest, isAbortError } from '$lib/async/latest-request';
	import * as Card from '$lib/components/ui/card';
	import * as Table from '$lib/components/ui/table';
	import { Switch } from '$lib/components/ui/switch';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import { toast } from 'svelte-sonner';
	import { Save, Loader2, DoorOpen, UserX, RefreshCw } from '@lucide/svelte';
	import { SvelteSet } from 'svelte/reactivity';

	let { data }: PageProps = $props();
	let id = $derived(data.id);
	const canScoreAdmission = $derived($can.has(PERMISSIONS.ADMISSION_SCORES_ALL));
	const roundRequest = new LatestRequest();
	const tracksRequest = new LatestRequest();
	const subjectsRequest = new LatestRequest();
	const scoresRequest = new LatestRequest();
	const rosterRequest = new LatestRequest();
	const appsRequest = new LatestRequest();
	let renderedId = '';

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
	let applications: ApplicationListItem[] = $state([]);
	let appsTrackId = $state('');
	let appsLoaded = $state(false);
	let appsLoading = $state(false);
	let appsError = $state('');
	let seatGroups: ScoreRoomGroup[] = $state([]);
	let rosterLoaded = $state(false);
	let rosterLoading = $state(true);
	let rosterError = $state('');
	let viewMode = $state<'track' | 'room'>('room');
	let saving = $state(false);
	let selectedTrack = $state('');
	let scoresLoaded = $state(false);
	let scoresLoading = $state(true);
	let scoresError = $state('');
	let activeSubjectIds: string[] = $state([]);

	let scores: Record<string, Record<string, string>> = $state({});
	let dirtyScoreKeys = new SvelteSet<string>();
	let absentIds = new SvelteSet<string>();
	let togglingAbsent: Record<string, boolean> = $state({});

	// flat list in room order for Enter navigation
	let appsInRoomOrder = $derived(
		seatGroups.flatMap((g) =>
			g.seats.map((s) => ({
				id: s.applicationId,
				applicationNumber: s.applicationNumber,
				fullName: s.fullName
			}))
		)
	);

	let viewChosenByUser = false;

	function ensureScoreRows(ids: string[]) {
		for (const applicationId of ids) {
			if (!scores[applicationId]) scores[applicationId] = {};
		}
	}

	function applyRawScores(rows: RawScore[]) {
		absentIds.clear();
		for (const row of rows) {
			if (row.status === 'absent') absentIds.add(row.applicationId);
			ensureScoreRows([row.applicationId]);
			if (!dirtyScoreKeys.has(`${row.applicationId}:${row.subjectId}`)) {
				scores[row.applicationId][row.subjectId] = row.score == null ? '' : row.score.toString();
			}
		}
	}

	function applyTracks(rows: AdmissionTrack[]) {
		tracks = rows;
		if (!rows.some((track) => track.id === selectedTrack)) selectedTrack = rows[0]?.id ?? '';
		if (viewMode === 'track' && selectedTrack) void loadApps();
	}

	function applySubjects(rows: AdmissionExamSubject[]) {
		const previous = new Set(subjects.map((subject) => subject.id));
		const inactive = new Set(
			previous.values().filter((subjectId) => !activeSubjectIds.includes(subjectId))
		);
		subjects = rows;
		activeSubjectIds = rows
			.filter((subject) => !inactive.has(subject.id))
			.map((subject) => subject.id);
	}

	function applyRoomRoster(groups: ScoreRoomGroup[]) {
		seatGroups = groups;
		ensureScoreRows(groups.flatMap((group) => group.seats.map((seat) => seat.applicationId)));
		if (!viewChosenByUser) {
			viewMode = groups.length > 0 ? 'room' : 'track';
			if (viewMode === 'track' && selectedTrack) void loadApps();
		}
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
			round = value;
			roundLoaded = true;
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
			applySubjects(value);
			subjectsLoaded = true;
		} catch (cause) {
			if (!isAbortError(cause) && subjectsRequest.isCurrent(revision))
				subjectsError = cause instanceof Error ? cause.message : 'โหลดวิชาสอบไม่สำเร็จ';
		} finally {
			if (subjectsRequest.isCurrent(revision)) subjectsLoading = false;
		}
	}

	async function loadScores() {
		if (!id || !canScoreAdmission) return;
		const sourceId = id;
		const { revision, signal } = scoresRequest.begin();
		scoresLoading = true;
		scoresError = '';
		try {
			const value = await getAllScores(sourceId, { signal });
			if (!scoresRequest.isCurrent(revision) || sourceId !== id) return;
			applyRawScores(value);
			scoresLoaded = true;
		} catch (cause) {
			if (!isAbortError(cause) && scoresRequest.isCurrent(revision))
				scoresError = cause instanceof Error ? cause.message : 'โหลดคะแนนไม่สำเร็จ';
		} finally {
			if (scoresRequest.isCurrent(revision)) scoresLoading = false;
		}
	}

	async function loadRoomRoster() {
		if (!id || !canScoreAdmission) return;
		const sourceId = id;
		const { revision, signal } = rosterRequest.begin();
		rosterLoading = true;
		rosterError = '';
		try {
			const value = await getScoreRoomRoster(sourceId, { signal });
			if (!rosterRequest.isCurrent(revision) || sourceId !== id) return;
			applyRoomRoster(value);
			rosterLoaded = true;
		} catch (cause) {
			if (!isAbortError(cause) && rosterRequest.isCurrent(revision))
				rosterError = cause instanceof Error ? cause.message : 'โหลดรายชื่อห้องสอบไม่สำเร็จ';
		} finally {
			if (rosterRequest.isCurrent(revision)) rosterLoading = false;
		}
	}

	async function loadApps(force = false) {
		if (!id || !selectedTrack || !canScoreAdmission || viewMode !== 'track') return;
		const sourceId = id;
		const sourceTrack = selectedTrack;
		if (!force && appsTrackId === sourceTrack && (appsLoaded || appsLoading)) return;
		const { revision, signal } = appsRequest.begin();
		if (appsTrackId !== sourceTrack) {
			applications = [];
			appsLoaded = false;
		}
		appsTrackId = sourceTrack;
		appsLoading = true;
		appsError = '';
		try {
			const allApps = await listApplications(sourceId, { trackId: sourceTrack }, { signal });
			if (!appsRequest.isCurrent(revision) || sourceId !== id || sourceTrack !== selectedTrack)
				return;
			applications = allApps.filter((app) =>
				['verified', 'scored', 'accepted', 'absent'].includes(app.status)
			);
			ensureScoreRows(applications.map((app) => app.id));
			appsLoaded = true;
		} catch (cause) {
			if (!isAbortError(cause) && appsRequest.isCurrent(revision))
				appsError = cause instanceof Error ? cause.message : 'โหลดผู้สมัครไม่สำเร็จ';
		} finally {
			if (appsRequest.isCurrent(revision)) appsLoading = false;
		}
	}

	function chooseView(mode: 'room' | 'track') {
		viewChosenByUser = true;
		viewMode = mode;
		if (mode === 'track') void loadApps();
		else if (appsLoading) {
			appsRequest.abort();
			appsLoading = false;
		}
	}

	function chooseTrack(trackId: string) {
		selectedTrack = trackId;
		void loadApps();
	}

	async function saveScores() {
		if (!id || !canScoreAdmission || dirtyScoreKeys.size === 0) return;
		const sourceId = id;
		const snapshot: Record<string, string> = {};
		const byApplication: Record<string, { examSubjectId: string; score?: number }[]> = {};
		for (const key of dirtyScoreKeys) {
			const [applicationId, subjectId] = key.split(':');
			const value = scores[applicationId]?.[subjectId] ?? '';
			snapshot[key] = value;
			const parsed = Number(value);
			const entry =
				value.trim() === '' || !Number.isFinite(parsed)
					? { examSubjectId: subjectId }
					: { examSubjectId: subjectId, score: parsed };
			const existing = byApplication[applicationId] ?? [];
			existing.push(entry);
			byApplication[applicationId] = existing;
		}
		const entries = Object.entries(byApplication).map(([applicationId, changedScores]) => ({
			applicationId,
			scores: changedScores
		}));
		saving = true;
		try {
			await bulkUpdateScores(sourceId, entries);
			if (sourceId !== id) return;
			for (const [key, savedValue] of Object.entries(snapshot)) {
				const [applicationId, subjectId] = key.split(':');
				if ((scores[applicationId]?.[subjectId] ?? '') === savedValue) dirtyScoreKeys.delete(key);
			}
			toast.success('บันทึกคะแนนที่เปลี่ยนแล้ว');
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'บันทึกไม่สำเร็จ');
		} finally {
			if (sourceId === id) saving = false;
		}
	}

	async function toggleAbsent(appId: string) {
		if (!canScoreAdmission) return;
		const sourceId = id;
		const isAbsent = absentIds.has(appId);
		togglingAbsent = { ...togglingAbsent, [appId]: true };
		try {
			await markAbsent(appId, !isAbsent);
			if (sourceId !== id) return;
			if (isAbsent) absentIds.delete(appId);
			else absentIds.add(appId);
			toast.success(isAbsent ? 'ยกเลิกขาดสอบแล้ว' : 'ทำเครื่องหมายขาดสอบแล้ว');
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'ดำเนินการไม่สำเร็จ');
		} finally {
			if (sourceId === id) togglingAbsent = { ...togglingAbsent, [appId]: false };
		}
	}

	function handleKeydown(e: KeyboardEvent, appIndex: number, currentSubIndex: number) {
		if (e.key === 'Enter') {
			e.preventDefault();
			const currentApps = viewMode === 'room' ? appsInRoomOrder : applications;

			let nextSubIdx = -1;
			for (let i = currentSubIndex + 1; i < subjects.length; i++) {
				if (activeSubjectIds.includes(subjects[i].id)) {
					nextSubIdx = i;
					break;
				}
			}

			if (nextSubIdx !== -1) {
				const appId = currentApps[appIndex].id;
				const subId = subjects[nextSubIdx].id;
				document.getElementById(`score-${appId}-${subId}`)?.focus();
			} else {
				const nextAppIndex = appIndex + 1;
				if (nextAppIndex < currentApps.length) {
					let firstVisSubIdx = -1;
					for (let i = 0; i < subjects.length; i++) {
						if (activeSubjectIds.includes(subjects[i].id)) {
							firstVisSubIdx = i;
							break;
						}
					}
					if (firstVisSubIdx !== -1) {
						const nextAppId = currentApps[nextAppIndex].id;
						const subId = subjects[firstVisSubIdx].id;
						document.getElementById(`score-${nextAppId}-${subId}`)?.focus();
					}
				}
			}
		}
	}

	$effect.pre(() => {
		const routeId = data.id;
		const routeRound = data.round;
		const routeTracks = data.tracks;
		const routeSubjects = data.subjects;
		const routeScores = data.rawScores;
		const routeRoster = data.roomRoster;
		const { revision: roundRevision } = roundRequest.begin();
		const { revision: tracksRevision } = tracksRequest.begin();
		const { revision: subjectsRevision } = subjectsRequest.begin();
		const { revision: scoresRevision } = scoresRequest.begin();
		const { revision: rosterRevision } = rosterRequest.begin();
		appsRequest.abort();
		untrack(() => {
			if (renderedId !== routeId) {
				renderedId = routeId;
				round = null;
				roundLoaded = false;
				tracks = [];
				tracksLoaded = false;
				subjects = [];
				subjectsLoaded = false;
				applications = [];
				appsTrackId = '';
				appsLoaded = false;
				appsError = '';
				selectedTrack = '';
				seatGroups = [];
				rosterLoaded = false;
				viewMode = 'room';
				viewChosenByUser = false;
				scoresLoaded = false;
				activeSubjectIds = [];
				scores = {};
				dirtyScoreKeys.clear();
				absentIds.clear();
				togglingAbsent = {};
				saving = false;
			}
			appsLoading = false;
			roundLoading = true;
			tracksLoading = true;
			subjectsLoading = true;
			scoresLoading = true;
			rosterLoading = true;
			roundError = '';
			tracksError = '';
			subjectsError = '';
			scoresError = '';
			rosterError = '';
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
					applySubjects(result.data);
					subjectsLoaded = true;
				} else if (!result.ok) subjectsError = result.error;
				subjectsLoading = false;
			});
		});
		void routeScores.then((result) => {
			if (!scoresRequest.isCurrent(scoresRevision)) return;
			untrack(() => {
				if (result.ok && result.data) {
					applyRawScores(result.data);
					scoresLoaded = true;
				} else if (!result.ok) scoresError = result.error;
				scoresLoading = false;
			});
		});
		void routeRoster.then((result) => {
			if (!rosterRequest.isCurrent(rosterRevision)) return;
			untrack(() => {
				if (result.ok && result.data) {
					applyRoomRoster(result.data);
					rosterLoaded = true;
				} else if (!result.ok) rosterError = result.error;
				rosterLoading = false;
			});
		});
		return () => {
			roundRequest.abort();
			tracksRequest.abort();
			subjectsRequest.abort();
			scoresRequest.abort();
			rosterRequest.abort();
			appsRequest.abort();
		};
	});
</script>

<PageShell
	title="กรอกคะแนนสอบ"
	description={round
		? `${round.name} | ${subjects.length} วิชา`
		: 'กรอกและบันทึกคะแนนสอบของผู้สมัคร'}
	backHref={`/staff/academic/admission/${id}`}
>
	{#if !canScoreAdmission}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์กรอกคะแนนสอบ"
			description="หน้านี้ต้องใช้สิทธิ์จัดการคะแนนรับสมัครก่อนจึงจะแสดงรายชื่อและบันทึกคะแนนได้"
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
		{:else if roundLoading && !roundLoaded}
			<PageSkeleton variant="cards" rows={1} />
		{:else if roundLoading}
			<RegionUpdatingState class="static" label="กำลังอัปเดตรอบรับสมัคร..." />
		{/if}
		<!-- View mode + Track selector -->
		<Card.Root class="gap-0 py-0" aria-busy={tracksLoading || rosterLoading}>
			<Card.Content class="flex flex-wrap items-center gap-3 p-3 sm:p-4">
				{#if rosterLoading || rosterError || seatGroups.length > 0}
					<div class="flex gap-1.5 shrink-0">
						<Button
							variant={viewMode === 'room' ? 'default' : 'outline'}
							size="sm"
							onclick={() => chooseView('room')}
						>
							<DoorOpen class="w-3.5 h-3.5 mr-1.5" />
							ตามห้องสอบ
						</Button>
						<Button
							variant={viewMode === 'track' ? 'default' : 'outline'}
							size="sm"
							onclick={() => chooseView('track')}
						>
							ตามสาย
						</Button>
					</div>
					<div class="w-px h-5 bg-border shrink-0"></div>
				{/if}
				{#if viewMode === 'track'}
					{#if tracksLoading && !tracksLoaded}
						<span class="text-sm text-muted-foreground">กำลังโหลดสายการเรียน...</span>
					{:else if tracksError && !tracksLoaded}
						<span role="alert" class="text-sm text-destructive">{tracksError}</span>
						<Button size="sm" variant="outline" onclick={loadTracks}>ลองใหม่</Button>
					{:else}
						<p class="text-sm font-medium whitespace-nowrap">สายการเรียน:</p>
						<div class="flex gap-2 flex-wrap">
							{#each tracks as track (track.id)}
								<Button
									variant={selectedTrack === track.id ? 'default' : 'outline'}
									size="sm"
									onclick={() => chooseTrack(track.id)}
								>
									{track.name}
									<span class="ml-1 opacity-70">({track.applicationCount ?? 0})</span>
								</Button>
							{/each}
						</div>
						<Button
							size="icon"
							variant="outline"
							onclick={loadTracks}
							disabled={tracksLoading}
							aria-label="โหลดสายการเรียนใหม่"><RefreshCw class="h-4 w-4" /></Button
						>
						{#if tracksLoading}<RegionUpdatingState
								class="static"
								label="กำลังอัปเดตสายการเรียน..."
							/>{/if}
						{#if tracksError}<span role="alert" class="text-sm text-destructive">{tracksError}</span
							>{/if}
					{/if}
				{:else}
					{#if rosterLoading && !rosterLoaded}
						<p class="text-sm text-muted-foreground">กำลังโหลดรายชื่อห้องสอบ...</p>
					{:else}
						<p class="text-sm text-muted-foreground">
							{seatGroups.length} ห้องสอบ · {appsInRoomOrder.length} คน
						</p>
						{#each seatGroups.slice(0, 3) as group (group.examRoomId)}
							<span class="rounded-md border px-2 py-1 text-xs">{group.roomName}</span>
						{/each}
						{#if seatGroups.length > 3}<span class="text-xs text-muted-foreground"
								>+{seatGroups.length - 3} ห้อง</span
							>{/if}
					{/if}
				{/if}
				{#if viewMode === 'room'}
					<Button
						size="icon"
						variant="outline"
						onclick={loadRoomRoster}
						disabled={rosterLoading}
						aria-label="โหลดรายชื่อห้องสอบใหม่"><RefreshCw class="h-4 w-4" /></Button
					>
				{/if}
			</Card.Content>
		</Card.Root>
		<div class="flex flex-wrap items-center gap-2" aria-busy={subjectsLoading || scoresLoading}>
			<Button
				size="icon"
				variant="outline"
				onclick={loadSubjects}
				disabled={subjectsLoading}
				aria-label="โหลดวิชาสอบใหม่"><RefreshCw class="h-4 w-4" /></Button
			>
			<Button
				size="icon"
				variant="outline"
				onclick={loadScores}
				disabled={scoresLoading}
				aria-label="โหลดคะแนนใหม่"><RefreshCw class="h-4 w-4" /></Button
			>
			{#if viewMode === 'track' && appsLoaded}
				<Button
					size="icon"
					variant="outline"
					onclick={() => loadApps(true)}
					disabled={appsLoading}
					aria-label="โหลดผู้สมัครในสายใหม่"><RefreshCw class="h-4 w-4" /></Button
				>
			{/if}
			{#if subjectsLoading && subjectsLoaded}<RegionUpdatingState
					class="static"
					label="กำลังอัปเดตวิชาสอบ..."
				/>{/if}
			{#if scoresLoading && scoresLoaded}<RegionUpdatingState
					class="static"
					label="กำลังอัปเดตคะแนน..."
				/>{/if}
			{#if rosterLoading && rosterLoaded && viewMode === 'room'}<RegionUpdatingState
					class="static"
					label="กำลังอัปเดตรายชื่อห้องสอบ..."
				/>{/if}
			{#if appsLoading && appsLoaded && viewMode === 'track'}<RegionUpdatingState
					class="static"
					label="กำลังอัปเดตผู้สมัคร..."
				/>{/if}
			{#each [subjectsError, scoresError, viewMode === 'room' ? rosterError : appsError].filter(Boolean) as regionError, index (index)}
				<span role="alert" class="text-sm text-destructive">{regionError}</span>
			{/each}
		</div>

		{#if (subjectsLoading && !subjectsLoaded) || (scoresLoading && !scoresLoaded) || (viewMode === 'room' && rosterLoading && !rosterLoaded) || (viewMode === 'track' && ((tracksLoading && !tracksLoaded) || (appsLoading && !appsLoaded)))}
			<PageSkeleton variant="table" rows={6} columns={6} />
		{:else if subjectsError && !subjectsLoaded}
			<PageState
				variant="error"
				title="โหลดวิชาสอบไม่สำเร็จ"
				description={subjectsError}
				actionLabel="ลองอีกครั้ง"
				onaction={loadSubjects}
			/>
		{:else if scoresError && !scoresLoaded}
			<PageState
				variant="error"
				title="โหลดคะแนนไม่สำเร็จ"
				description={scoresError}
				actionLabel="ลองอีกครั้ง"
				onaction={loadScores}
			/>
		{:else if viewMode === 'room' && rosterError && !rosterLoaded}
			<PageState
				variant="error"
				title="โหลดรายชื่อห้องสอบไม่สำเร็จ"
				description={rosterError}
				actionLabel="ลองอีกครั้ง"
				onaction={loadRoomRoster}
			/>
		{:else if viewMode === 'track' && appsError && !appsLoaded}
			<PageState
				variant="error"
				title="โหลดผู้สมัครไม่สำเร็จ"
				description={appsError}
				actionLabel="ลองอีกครั้ง"
				onaction={() => loadApps(true)}
			/>
		{:else if viewMode === 'room' && seatGroups.length === 0}
			<PageState
				title="ยังไม่มีการจัดห้องสอบ"
				description="กรุณาจัดห้องสอบก่อนใช้มุมมองนี้"
				actionLabel="ไปหน้าห้องสอบ"
				href={`/staff/academic/admission/${id}/exam-rooms`}
			/>
		{:else if viewMode === 'track' && tracks.length === 0}
			<PageState title="ยังไม่มีสายการเรียน" description="เพิ่มสายการเรียนก่อนกรอกคะแนน" />
		{:else if viewMode === 'track' && applications.length === 0}
			<PageState
				title="ไม่มีผู้สมัครที่พร้อมกรอกคะแนน"
				description="ต้องยืนยันใบสมัครก่อนกรอกคะแนน"
			/>
		{:else}
			<Card.Root class="overflow-x-auto">
				<Table.Root>
					<Table.Header>
						<Table.Row>
							<Table.Head class="w-10">ที่</Table.Head>
							<Table.Head>{viewMode === 'room' ? 'เลขที่สอบ' : 'เลขที่ใบสมัคร'}</Table.Head>
							<Table.Head>ชื่อ-สกุล</Table.Head>
							<Table.Head class="text-center w-20">ขาดสอบ</Table.Head>
							{#each subjects as sub (sub.id)}
								{@const isActive = activeSubjectIds.includes(sub.id)}
								<Table.Head
									class="text-center min-w-[120px] pb-4 transition-all duration-300 {isActive
										? ''
										: 'bg-muted/40 shadow-inner'}"
								>
									<div class="flex flex-col items-center gap-2">
										<Switch
											checked={isActive}
											onCheckedChange={(v) => {
												if (v) activeSubjectIds = [...activeSubjectIds, sub.id];
												else activeSubjectIds = activeSubjectIds.filter((id) => id !== sub.id);
											}}
										/>
										<div class={isActive ? '' : 'opacity-50'}>
											{sub.name}
											<span class="block text-xs font-normal text-muted-foreground"
												>/{sub.maxScore}</span
											>
										</div>
									</div>
								</Table.Head>
							{/each}
						</Table.Row>
					</Table.Header>
					<Table.Body>
						{#if viewMode === 'room'}
							{#each seatGroups as group (group.examRoomId)}
								<!-- Room header row -->
								<Table.Row class="bg-muted/50 hover:bg-muted/50">
									<Table.Cell colspan={4 + subjects.length} class="font-semibold py-2 px-4 text-sm">
										<span class="flex items-center gap-2">
											<DoorOpen class="w-4 h-4" />
											{group.roomName}
											{#if group.buildingName}
												<span class="text-muted-foreground font-normal">· {group.buildingName}</span
												>
											{/if}
											<span class="text-muted-foreground font-normal ml-auto"
												>{group.seats.length} คน</span
											>
										</span>
									</Table.Cell>
								</Table.Row>
								{#each group.seats as seat, seatIdx (seat.applicationId)}
									{@const globalIdx =
										seatGroups
											.slice(0, seatGroups.indexOf(group))
											.reduce((acc, g) => acc + g.seats.length, 0) + seatIdx}
									{@const isAbsent = absentIds.has(seat.applicationId)}
									<Table.Row class={isAbsent ? 'opacity-50' : ''}>
										<Table.Cell class="text-center text-muted-foreground"
											>{seat.seatNumber}</Table.Cell
										>
										<Table.Cell class="font-mono text-xs"
											>{seat.examId ?? seat.applicationNumber ?? '-'}</Table.Cell
										>
										<Table.Cell class="font-medium {isAbsent ? 'line-through' : ''}"
											>{seat.fullName}</Table.Cell
										>
										<Table.Cell class="text-center">
											<Button
												size="sm"
												variant={isAbsent ? 'default' : 'ghost'}
												class="h-7 text-xs gap-1 {isAbsent
													? 'bg-red-600 hover:bg-red-700'
													: 'text-muted-foreground hover:text-red-600'}"
												disabled={togglingAbsent[seat.applicationId]}
												onclick={() => toggleAbsent(seat.applicationId)}
											>
												<UserX class="w-3 h-3" />
												{isAbsent ? 'ขาด' : ''}
											</Button>
										</Table.Cell>
										{#each subjects as sub, subIdx (sub.id)}
											{@const isActive = activeSubjectIds.includes(sub.id)}
											<Table.Cell
												class="px-2 py-1.5 transition-all duration-300 {isActive
													? ''
													: 'bg-muted/40'}"
											>
												<Input
													id="score-{seat.applicationId}-{sub.id}"
													type="number"
													min="0"
													max={sub.maxScore}
													step="0.5"
													disabled={!isActive || isAbsent}
													value={scores[seat.applicationId]?.[sub.id] ?? ''}
													oninput={(e) => {
														const value = e.currentTarget.value;
														const parsed = Number(value);
														scores[seat.applicationId][sub.id] =
															value !== '' && parsed > sub.maxScore
																? sub.maxScore.toString()
																: value;
														dirtyScoreKeys.add(`${seat.applicationId}:${sub.id}`);
													}}
													onkeydown={(e) => handleKeydown(e, globalIdx, subIdx)}
													class="h-7 text-center text-sm w-20 mx-auto {isActive
														? ''
														: 'opacity-50 cursor-not-allowed'}"
													placeholder="-"
												/>
											</Table.Cell>
										{/each}
									</Table.Row>
								{/each}
							{/each}
						{:else}
							{#each applications as app, i (app.id)}
								{@const isAbsent = absentIds.has(app.id)}
								<Table.Row class={isAbsent ? 'opacity-50' : ''}>
									<Table.Cell class="text-center text-muted-foreground">{i + 1}</Table.Cell>
									<Table.Cell class="font-mono text-xs">{app.applicationNumber ?? '-'}</Table.Cell>
									<Table.Cell class="font-medium {isAbsent ? 'line-through' : ''}"
										>{app.fullName}</Table.Cell
									>
									<Table.Cell class="text-center">
										<Button
											size="sm"
											variant={isAbsent ? 'default' : 'ghost'}
											class="h-7 text-xs gap-1 {isAbsent
												? 'bg-red-600 hover:bg-red-700'
												: 'text-muted-foreground hover:text-red-600'}"
											disabled={togglingAbsent[app.id]}
											onclick={() => toggleAbsent(app.id)}
										>
											<UserX class="w-3 h-3" />
											{isAbsent ? 'ขาด' : ''}
										</Button>
									</Table.Cell>
									{#each subjects as sub, subIdx (sub.id)}
										{@const isActive = activeSubjectIds.includes(sub.id)}
										<Table.Cell
											class="px-2 py-1.5 transition-all duration-300 {isActive
												? ''
												: 'bg-muted/40'}"
										>
											<Input
												id="score-{app.id}-{sub.id}"
												type="number"
												min="0"
												max={sub.maxScore}
												step="0.5"
												disabled={!isActive || isAbsent}
												value={scores[app.id]?.[sub.id] ?? ''}
												oninput={(e) => {
													const value = e.currentTarget.value;
													const parsed = Number(value);
													scores[app.id][sub.id] =
														value !== '' && parsed > sub.maxScore ? sub.maxScore.toString() : value;
													dirtyScoreKeys.add(`${app.id}:${sub.id}`);
												}}
												onkeydown={(e) => handleKeydown(e, i, subIdx)}
												class="h-7 text-center text-sm w-20 mx-auto {isActive
													? ''
													: 'opacity-50 cursor-not-allowed'}"
												placeholder="-"
											/>
										</Table.Cell>
									{/each}
								</Table.Row>
							{/each}
						{/if}
					</Table.Body>
				</Table.Root>
			</Card.Root>

			<div class="flex justify-end">
				<Button onclick={saveScores} disabled={saving || dirtyScoreKeys.size === 0} class="gap-2">
					{#if saving}<Loader2 class="w-4 h-4 animate-spin" />{:else}<Save class="w-4 h-4" />{/if}
					{saving ? 'กำลังบันทึก...' : `บันทึกคะแนนที่เปลี่ยน (${dirtyScoreKeys.size})`}
				</Button>
			</div>
		{/if}
	{/if}
</PageShell>

<style>
	/* ซ่อนลูกศรขึ้น/ลง ของช่องใส่ตัวเลข (number input spinners) */
	:global(input[type='number']::-webkit-outer-spin-button) {
		-webkit-appearance: none;
		margin: 0;
	}
	:global(input[type='number']::-webkit-inner-spin-button) {
		-webkit-appearance: none;
		margin: 0;
	}
	:global(input[type='number']) {
		-moz-appearance: textfield; /* สำหรับ Firefox */
		appearance: textfield;
	}
</style>
