<script lang="ts">
	import { onMount } from 'svelte';
	import type { PageProps } from './$types';
	import {
		getRound,
		listTracks,
		listSubjects,
		listApplications,
		bulkUpdateScores,
		getExamSeats,
		markAbsent,
		type AdmissionRound,
		type AdmissionTrack,
		type AdmissionExamSubject,
		type ApplicationListItem,
		type ExamRoomGroup,
		getAllScores
	} from '$lib/api/admission';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import * as Card from '$lib/components/ui/card';
	import * as Table from '$lib/components/ui/table';
	import { Switch } from '$lib/components/ui/switch';
	import { toast } from 'svelte-sonner';
	import { ArrowLeft, ClipboardList, Save, Loader2, DoorOpen, UserX } from 'lucide-svelte';

	let { data, params }: PageProps = $props();
	let id = $derived(params.id);

	let round: AdmissionRound | null = $state(null);
	let tracks: AdmissionTrack[] = $state([]);
	let subjects: AdmissionExamSubject[] = $state([]);
	let applications: ApplicationListItem[] = $state([]);
	let seatGroups: ExamRoomGroup[] = $state([]);
	let viewMode = $state<'track' | 'room'>('track');
	let loading = $state(true);
	let saving = $state(false);
	let selectedTrack = $state('');
	let allRawScores: any[] = [];
	let activeSubjectIds: string[] = $state([]);

	let scores: Record<string, Record<string, string>> = $state({});
	let absentIds: Set<string> = $state(new Set());
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

	async function loadAll() {
		if (!id) return;
		loading = true;
		try {
			const [r, t, s, allS, seatData] = await Promise.all([
				getRound(id),
				listTracks(id),
				listSubjects(id),
				getAllScores(id),
				getExamSeats(id)
			]);
			round = r;
			tracks = t;
			subjects = s;
			activeSubjectIds = s.map((sub) => sub.id);
			allRawScores = allS as any[];
			seatGroups = Array.isArray(seatData) ? seatData : [];

			// สร้าง set ของ absent จาก status ใน allRawScores
			const absSet = new Set<string>();
			for (const sc of allRawScores) {
				if (sc.status === 'absent') absSet.add(sc.applicationId);
			}
			absentIds = absSet;

			// default to room view if seats are assigned
			if (seatGroups.length > 0) {
				viewMode = 'room';
				// init scores for all seats
				for (const group of seatGroups) {
					for (const seat of group.seats) {
						if (!scores[seat.applicationId]) scores[seat.applicationId] = {};
						const appScores = allRawScores.filter((sc) => sc.applicationId === seat.applicationId);
						for (const sc of appScores) {
							if (sc.score != null) scores[seat.applicationId][sc.subjectId] = sc.score.toString();
						}
					}
				}
			}

			if (t.length > 0 && !selectedTrack) selectedTrack = t[0].id;
			await loadApps();
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'โหลดไม่สำเร็จ');
		} finally {
			loading = false;
		}
	}

	async function loadApps() {
		if (!id || !selectedTrack) return;
		const allApps = await listApplications(id, { trackId: selectedTrack });
		// Only show applications that are verified, scored, accepted, or absent
		applications = allApps.filter((a) => ['verified', 'scored', 'accepted', 'absent'].includes(a.status));

		for (const app of applications) {
			if (!scores[app.id]) scores[app.id] = {};

			// Map existing scores
			const appScores = allRawScores.filter((s) => s.applicationId === app.id);
			for (const s of appScores) {
				if (s.score !== null && s.score !== undefined) {
					scores[app.id][s.subjectId] = s.score.toString();
				}
			}
		}
	}

	async function saveScores() {
		if (!id) return;
		saving = true;
		try {
			const entries = Object.entries(scores)
				.map(([appId, subScores]) => ({
					applicationId: appId,
					scores: Object.entries(subScores)
						.filter(([, v]) => v !== '')
						.map(([subId, v]) => ({ examSubjectId: subId, score: parseFloat(v) }))
				}))
				.filter((e) => e.scores.length > 0);
			await bulkUpdateScores(id, entries);
			toast.success('บันทึกคะแนนทั้งหมดแล้ว');
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'บันทึกไม่สำเร็จ');
		} finally {
			saving = false;
		}
	}

	async function toggleAbsent(appId: string) {
		const isAbsent = absentIds.has(appId);
		togglingAbsent = { ...togglingAbsent, [appId]: true };
		try {
			await markAbsent(appId, !isAbsent);
			const next = new Set(absentIds);
			if (isAbsent) next.delete(appId);
			else next.add(appId);
			absentIds = next;
			toast.success(isAbsent ? 'ยกเลิกขาดสอบแล้ว' : 'ทำเครื่องหมายขาดสอบแล้ว');
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'ดำเนินการไม่สำเร็จ');
		} finally {
			togglingAbsent = { ...togglingAbsent, [appId]: false };
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

	$effect(() => {
		if (selectedTrack) loadApps();
	});
	onMount(loadAll);
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
			<ClipboardList class="w-6 h-6" /> กรอกคะแนนสอบ
		</h1>
	</div>

	{#if round}
		<p class="text-muted-foreground text-sm">{round.name} — {subjects.length} วิชา</p>
	{/if}

	<!-- View mode + Track selector -->
	<Card.Root>
		<Card.Content class="pt-4 pb-4 flex flex-wrap items-center gap-3">
			{#if seatGroups.length > 0}
				<div class="flex gap-1.5 shrink-0">
					<Button
						variant={viewMode === 'room' ? 'default' : 'outline'}
						size="sm"
						onclick={() => (viewMode = 'room')}
					>
						<DoorOpen class="w-3.5 h-3.5 mr-1.5" />
						ตามห้องสอบ
					</Button>
					<Button
						variant={viewMode === 'track' ? 'default' : 'outline'}
						size="sm"
						onclick={() => (viewMode = 'track')}
					>
						ตามสาย
					</Button>
				</div>
				<div class="w-px h-5 bg-border shrink-0"></div>
			{/if}
			{#if viewMode === 'track'}
				<p class="text-sm font-medium whitespace-nowrap">สายการเรียน:</p>
				<div class="flex gap-2 flex-wrap">
					{#each tracks as track (track.id)}
						<Button
							variant={selectedTrack === track.id ? 'default' : 'outline'}
							size="sm"
							onclick={() => {
								selectedTrack = track.id;
							}}
						>
							{track.name}
							<span class="ml-1 opacity-70">({track.applicationCount ?? 0})</span>
						</Button>
					{/each}
				</div>
			{:else}
				<p class="text-sm text-muted-foreground">
					{seatGroups.length} ห้องสอบ · {appsInRoomOrder.length} คน
				</p>
			{/if}
		</Card.Content>
	</Card.Root>

	{#if loading}
		<Card.Root>
			<Card.Content class="flex justify-center py-16">
				<Loader2 class="w-8 h-8 animate-spin text-primary" />
			</Card.Content>
		</Card.Root>
	{:else if viewMode === 'room' && seatGroups.length === 0}
		<Card.Root>
			<Card.Content class="py-12 text-center text-muted-foreground">
				<p>ยังไม่มีการจัดห้องสอบ</p>
				<p class="text-xs mt-1">กรุณาจัดห้องสอบก่อนใช้มุมมองนี้</p>
			</Card.Content>
		</Card.Root>
	{:else if viewMode === 'track' && applications.length === 0}
		<Card.Root>
			<Card.Content class="py-12 text-center text-muted-foreground">
				<p>ไม่มีผู้สมัครที่ผ่านการตรวจสอบในสายนี้</p>
				<p class="text-xs mt-1">ต้องยืนยันใบสมัคร (status: verified) ก่อนกรอกคะแนน</p>
			</Card.Content>
		</Card.Root>
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
								<Table.Cell
									colspan={4 + subjects.length}
									class="font-semibold py-2 px-4 text-sm"
								>
									<span class="flex items-center gap-2">
										<DoorOpen class="w-4 h-4" />
										{group.roomName}
										{#if group.buildingName}
											<span class="text-muted-foreground font-normal">· {group.buildingName}</span>
										{/if}
										<span class="text-muted-foreground font-normal ml-auto"
											>{group.seats.length} คน</span
										>
									</span>
								</Table.Cell>
							</Table.Row>
							{#each group.seats as seat, seatIdx (seat.applicationId)}
								{@const globalIdx =
									seatGroups.slice(0, seatGroups.indexOf(group)).reduce((acc, g) => acc + g.seats.length, 0) + seatIdx}
								{@const isAbsent = absentIds.has(seat.applicationId)}
								<Table.Row class={isAbsent ? 'opacity-50' : ''}>
									<Table.Cell class="text-center text-muted-foreground"
										>{seat.seatNumber}</Table.Cell
									>
									<Table.Cell class="font-mono text-xs"
										>{seat.examId ?? seat.applicationNumber ?? '-'}</Table.Cell
									>
									<Table.Cell class="font-medium {isAbsent ? 'line-through' : ''}">{seat.fullName}</Table.Cell>
									<Table.Cell class="text-center">
										<Button
											size="sm"
											variant={isAbsent ? 'default' : 'ghost'}
											class="h-7 text-xs gap-1 {isAbsent ? 'bg-red-600 hover:bg-red-700' : 'text-muted-foreground hover:text-red-600'}"
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
											{#if !scores[seat.applicationId]}
												<!-- init on first render -->
												{(scores[seat.applicationId] = {})}
											{/if}
											<Input
												id="score-{seat.applicationId}-{sub.id}"
												type="number"
												min="0"
												max={sub.maxScore}
												step="0.5"
												disabled={!isActive || isAbsent}
												bind:value={scores[seat.applicationId][sub.id]}
												oninput={(e) => {
													const val = parseFloat(e.currentTarget.value);
													if (!isNaN(val) && val > sub.maxScore) {
														scores[seat.applicationId][sub.id] = sub.maxScore.toString();
													}
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
								<Table.Cell class="font-medium {isAbsent ? 'line-through' : ''}">{app.fullName}</Table.Cell>
								<Table.Cell class="text-center">
									<Button
										size="sm"
										variant={isAbsent ? 'default' : 'ghost'}
										class="h-7 text-xs gap-1 {isAbsent ? 'bg-red-600 hover:bg-red-700' : 'text-muted-foreground hover:text-red-600'}"
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
											bind:value={scores[app.id][sub.id]}
											oninput={(e) => {
												const val = parseFloat(e.currentTarget.value);
												if (!isNaN(val) && val > sub.maxScore) {
													scores[app.id][sub.id] = sub.maxScore.toString();
												}
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
			<Button onclick={saveScores} disabled={saving} class="gap-2">
				{#if saving}<Loader2 class="w-4 h-4 animate-spin" />{:else}<Save class="w-4 h-4" />{/if}
				{saving ? 'กำลังบันทึก...' : 'บันทึกคะแนนทั้งหมด'}
			</Button>
		</div>
	{/if}
</div>

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
