<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import {
		getRound,
		listTracks,
		getTrackRanking,
		assignRooms,
		type AdmissionRound,
		type AdmissionTrack,
		type TrackRankingResult
	} from '$lib/api/admission';
	import { Button } from '$lib/components/ui/button';
	import { Badge } from '$lib/components/ui/badge';
	import * as Card from '$lib/components/ui/card';
	import * as Table from '$lib/components/ui/table';
	import { Separator } from '$lib/components/ui/separator';
	import { toast } from 'svelte-sonner';
	import { ArrowLeft, GraduationCap, Trophy, Check, Loader2 } from 'lucide-svelte';

	let { data } = $props();
	let id = $derived($page.params.id);

	let round: AdmissionRound | null = $state(null);
	let tracks: AdmissionTrack[] = $state([]);
	let selectedTrack = $state('');
	let ranking: TrackRankingResult | null = $state(null);
	let loading = $state(false);
	let assigning = $state(false);
	let assigned = $state(false);

	async function loadBase() {
		if (!id) return;
		const [r, t] = await Promise.all([getRound(id), listTracks(id)]);
		round = r;
		tracks = t;
		if (t.length > 0) selectedTrack = t[0].id;
	}

	async function loadRanking() {
		if (!selectedTrack) return;
		loading = true;
		ranking = null;
		assigned = false;
		try {
			ranking = await getTrackRanking(selectedTrack);
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'โหลดผลเรียงคะแนนไม่สำเร็จ');
		} finally {
			loading = false;
		}
	}

	async function handleAssignRooms() {
		if (!id || !selectedTrack) return;
		if (!confirm('ยืนยันการจัดห้อง? การดำเนินการนี้จะลบผลจัดห้องเดิมและจัดใหม่')) return;
		assigning = true;
		try {
			await assignRooms(id, selectedTrack);
			toast.success('จัดห้องสำเร็จ!');
			assigned = true;
			await loadRanking();
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'จัดห้องไม่สำเร็จ');
		} finally {
			assigning = false;
		}
	}

	$effect(() => {
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

	{#if loading}
		<Card.Root>
			<Card.Content class="flex justify-center py-16">
				<Loader2 class="w-8 h-8 animate-spin text-primary" />
			</Card.Content>
		</Card.Root>
	{:else if ranking}
		<!-- Room Summary -->
		{#if ranking.rooms?.length > 0}
			<div class="grid grid-cols-2 md:grid-cols-4 gap-3">
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

		<!-- Ranking Table -->
		<Card.Root>
			<Card.Header class="flex flex-row items-center justify-between pb-3">
				<Card.Title class="flex items-center gap-2">
					<Trophy class="w-5 h-5 text-yellow-500" />
					ผลเรียงคะแนน — {ranking.trackName}
					<Badge variant="secondary">{ranking.applications.length} คน</Badge>
				</Card.Title>
				<Button
					onclick={handleAssignRooms}
					disabled={assigning || ranking.applications.length === 0}
					variant={assigned ? 'outline' : 'default'}
					class="gap-2"
				>
					{#if assigning}
						<Loader2 class="w-4 h-4 animate-spin" />
					{:else}
						<Check class="w-4 h-4" />
					{/if}
					{assigning ? 'กำลังจัดห้อง...' : assigned ? 'จัดห้องแล้ว (จัดใหม่)' : 'บันทึกจัดห้อง'}
				</Button>
			</Card.Header>

			<Table.Root>
				<Table.Header>
					<Table.Row>
						<Table.Head class="w-16 text-center">อันดับ</Table.Head>
						<Table.Head>เลขที่ใบสมัคร</Table.Head>
						<Table.Head>ชื่อ-สกุล</Table.Head>
						<Table.Head class="text-center">คะแนนรวม</Table.Head>
						<Table.Head class="text-center">ห้องที่ได้</Table.Head>
					</Table.Row>
				</Table.Header>
				<Table.Body>
					{#each ranking.applications as app (app.applicationId)}
						<Table.Row>
							<Table.Cell class="text-center">
								<span
									class="inline-flex items-center justify-center w-7 h-7 rounded-full text-xs font-bold {app.rank ===
									1
										? 'bg-yellow-100 text-yellow-700'
										: app.rank <= 3
											? 'bg-gray-100 text-gray-700'
											: 'text-muted-foreground'}"
								>
									{app.rank}
								</span>
							</Table.Cell>
							<Table.Cell class="font-mono text-xs">{app.applicationNumber ?? '-'}</Table.Cell>
							<Table.Cell class="font-medium">{app.fullName}</Table.Cell>
							<Table.Cell class="text-center font-semibold text-primary"
								>{app.totalScore.toFixed(1)}</Table.Cell
							>
							<Table.Cell class="text-center">
								{#if app.assignedRoom}
									<Badge variant="outline">{app.assignedRoom}</Badge>
								{:else}
									<span class="text-xs text-muted-foreground">เกินจำนวนที่รับ</span>
								{/if}
							</Table.Cell>
						</Table.Row>
					{/each}
				</Table.Body>
			</Table.Root>
		</Card.Root>
	{/if}
</div>
