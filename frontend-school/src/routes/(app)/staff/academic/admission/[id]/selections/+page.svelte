<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import {
		getRound,
		listTracks,
		listSubjects,
		getTrackRanking,
		assignRooms,
		changeApplicationTrack,
		type AdmissionRound,
		type AdmissionTrack,
		type AdmissionExamSubject,
		type TrackRankingResult,
		type RankingEntry
	} from '$lib/api/admission';
	import { Button } from '$lib/components/ui/button';
	import { Badge } from '$lib/components/ui/badge';
	import * as Card from '$lib/components/ui/card';
	import * as Table from '$lib/components/ui/table';
	import * as Select from '$lib/components/ui/select';
	import * as Dialog from '$lib/components/ui/dialog';
	import { toast } from 'svelte-sonner';
	import { ArrowLeft, GraduationCap, Trophy, Check, LoaderCircle } from 'lucide-svelte';

	let { data } = $props();
	let id = $derived(page.params.id);

	let round: AdmissionRound | null = $state(null);
	let tracks: AdmissionTrack[] = $state([]);
	let subjects: AdmissionExamSubject[] = $state([]);
	let selectedTrack = $state('');
	let selectedSubjectIds: string[] = $state([]);
	let ranking = $state<TrackRankingResult | null>(null);
	let loading = $state(false);
	let assigning = $state(false);
	let assigned = $state(false);
	let moveTargetTrackId: Record<string, string> = $state({});
	let moving: Record<string, boolean> = $state({});
	let showAssignDialog = $state(false);

	let acceptedApps = $derived(ranking?.applications.filter((a) => !a.isOverflow) ?? []);
	let overflowApps = $derived(ranking?.applications.filter((a) => a.isOverflow) ?? []);
	let otherTracks = $derived(tracks.filter((t) => t.id !== selectedTrack));

	async function loadBase() {
		if (!id) return;
		const [r, t, s] = await Promise.all([getRound(id), listTracks(id), listSubjects(id)]);
		round = r;
		tracks = t;
		subjects = s;
		selectedSubjectIds = s.map((x) => x.id);
		if (t.length > 0) selectedTrack = t[0].id;
	}

	async function loadRanking() {
		if (!selectedTrack) return;
		loading = true;
		ranking = null;
		assigned = false;
		try {
			ranking = await getTrackRanking(selectedTrack, selectedSubjectIds);
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
			await assignRooms(id, selectedTrack, selectedSubjectIds);
			toast.success('จัดห้องสำเร็จ!');
			assigned = true;
			await loadRanking();
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'จัดห้องไม่สำเร็จ');
		} finally {
			assigning = false;
		}
	}

	async function moveToTrack(appId: string) {
		const targetId = moveTargetTrackId[appId];
		if (!targetId) return;
		moving = { ...moving, [appId]: true };
		try {
			await changeApplicationTrack(appId, targetId);
			toast.success('ย้ายสายสำเร็จ');
			await loadRanking();
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'ย้ายสายไม่สำเร็จ');
		} finally {
			moving = { ...moving, [appId]: false };
		}
	}

	// reload เมื่อ selectedTrack หรือ selectedSubjectIds เปลี่ยน
	$effect(() => {
		void selectedSubjectIds.length;
		if (selectedTrack) loadRanking();
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
		<Card.Content class="pt-4 pb-4 flex items-center gap-4">
			<p class="text-sm font-medium">สาย:</p>
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
					</Button>
				{/each}
			</div>
		</Card.Content>
	</Card.Root>

	<!-- Subject Picker (pass 1) -->
	{#if subjects.length > 0}
		<Card.Root>
			<Card.Content class="pt-4 pb-3 space-y-2">
				<p class="text-sm font-medium">วิชาที่ใช้คัดเลือก (pass 1)</p>
				<div class="flex flex-wrap gap-3">
					{#each subjects as s (s.id)}
						<label class="flex items-center gap-1.5 text-sm cursor-pointer">
							<input
								type="checkbox"
								value={s.id}
								checked={selectedSubjectIds.includes(s.id)}
								onchange={(e) => {
									if (e.currentTarget.checked) {
										selectedSubjectIds = [...selectedSubjectIds, s.id];
									} else {
										selectedSubjectIds = selectedSubjectIds.filter((x) => x !== s.id);
									}
								}}
							/>
							{s.name}
							<span class="text-xs text-muted-foreground">({s.maxScore})</span>
						</label>
					{/each}
				</div>
				<p class="text-xs text-muted-foreground">
					คนที่ผ่านการคัดเลือกจะถูกเรียงใหม่ด้วยคะแนนรวมทุกวิชา (pass 2) เพื่อจัดห้อง
				</p>
			</Card.Content>
		</Card.Root>
	{/if}

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
						<Card.Content class="pt-4 pb-4 text-center">
							<p class="font-semibold">{room.roomName}</p>
							<p class="text-xs text-muted-foreground">รับ {room.capacity} คน</p>
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
						<Table.Row>
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
							<Table.Cell class="font-medium">{app.fullName}</Table.Cell>
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
								{#if app.assignedRoom}
									<Badge variant="outline">{app.assignedRoom}</Badge>
								{:else}
									<span class="text-xs text-muted-foreground">ยังไม่จัดห้อง</span>
								{/if}
							</Table.Cell>
							<Table.Cell>
								<div class="flex gap-2 items-center">
									<Select.Root
										type="single"
										value={moveTargetTrackId[app.applicationId] ?? ''}
										onValueChange={(v) => {
											moveTargetTrackId = { ...moveTargetTrackId, [app.applicationId]: v };
										}}
									>
										<Select.Trigger class="h-8 text-xs w-40">
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
								<Table.Cell class="font-medium">{app.fullName}</Table.Cell>
								<Table.Cell class="text-center text-blue-600 font-semibold">
									{app.selectionScore.toFixed(1)}
								</Table.Cell>
								<Table.Cell class="text-center font-semibold">
									{app.totalScore.toFixed(1)}
								</Table.Cell>
								<Table.Cell>
									<div class="flex gap-2 items-center">
										<Select.Root
											type="single"
											value={moveTargetTrackId[app.applicationId] ?? ''}
											onValueChange={(v) => {
												moveTargetTrackId = { ...moveTargetTrackId, [app.applicationId]: v };
											}}
										>
											<Select.Trigger class="h-8 text-xs w-40">
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
				การดำเนินการนี้จะลบผลจัดห้องเดิมและจัดใหม่ทั้งหมด ต้องการดำเนินการต่อหรือไม่?
			</Dialog.Description>
		</Dialog.Header>
		<Dialog.Footer class="flex-col sm:flex-row gap-2">
			<Button variant="outline" onclick={() => (showAssignDialog = false)}>ยกเลิก</Button>
			<Button onclick={confirmAssignRooms}>ยืนยัน</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
