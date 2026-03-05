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
	import { toast } from 'svelte-sonner';
	import { ArrowLeft, GraduationCap, Trophy, Check } from 'lucide-svelte';

	let id = $derived($page.params.id);
	let round: AdmissionRound | null = $state(null);
	let tracks: AdmissionTrack[] = $state([]);
	let selectedTrack = $state('');
	let ranking: TrackRankingResult | null = $state(null);
	let loading = $state(false);
	let assigning = $state(false);
	let assigned = $state(false);

	async function load() {
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
			toast.success('จัดห้องสำเร็จ อัปเดตสถานะ "ได้รับการคัดเลือก" แล้ว');
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

	onMount(load);
</script>

<svelte:head>
	<title>จัดห้องเรียน - SchoolOrbit</title>
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
	<div class="bg-card border border-border rounded-lg p-4 flex items-center gap-4">
		<label class="text-sm font-medium">สาย:</label>
		<div class="flex gap-2 flex-wrap">
			{#each tracks as track (track.id)}
				<button
					onclick={() => {
						selectedTrack = track.id;
					}}
					class="text-sm px-3 py-1.5 rounded-md border transition-colors {selectedTrack === track.id
						? 'bg-primary text-primary-foreground border-primary'
						: 'border-border hover:bg-accent'}"
				>
					{track.name}
				</button>
			{/each}
		</div>
	</div>

	{#if loading}
		<div class="bg-card border border-border rounded-lg p-10 text-center">
			<div
				class="w-8 h-8 border-4 border-primary border-t-transparent rounded-full animate-spin mx-auto"
			></div>
		</div>
	{:else if ranking}
		<!-- Summary -->
		<div class="grid grid-cols-2 md:grid-cols-4 gap-3">
			{#each ranking.rooms as room (room.roomId)}
				<div class="bg-card border border-border rounded-lg p-3 text-center">
					<p class="font-semibold text-foreground">{room.roomName}</p>
					<p class="text-xs text-muted-foreground">รับ {room.capacity} คน</p>
				</div>
			{/each}
		</div>

		<!-- Ranking Table -->
		<div class="bg-card border border-border rounded-lg overflow-hidden">
			<div class="flex items-center justify-between px-5 py-4 border-b border-border">
				<h2 class="font-semibold flex items-center gap-2">
					<Trophy class="w-4 h-4 text-yellow-500" />
					ผลเรียงคะแนน — {ranking.trackName}
					<span class="text-sm font-normal text-muted-foreground"
						>({ranking.applications.length} คน)</span
					>
				</h2>
				<Button
					onclick={handleAssignRooms}
					disabled={assigning || ranking.applications.length === 0}
					class="gap-2"
					variant={assigned ? 'outline' : 'default'}
				>
					<Check class="w-4 h-4" />
					{assigning ? 'กำลังจัดห้อง...' : assigned ? 'จัดห้องแล้ว (จัดใหม่)' : 'บันทึกจัดห้อง'}
				</Button>
			</div>

			<div class="overflow-x-auto">
				<table class="w-full text-sm">
					<thead class="bg-muted/50 border-b border-border">
						<tr>
							<th class="px-4 py-3 text-center text-muted-foreground font-medium w-10">อันดับ</th>
							<th class="px-4 py-3 text-left text-muted-foreground font-medium">เลขที่ใบสมัคร</th>
							<th class="px-4 py-3 text-left text-muted-foreground font-medium">ชื่อ-สกุล</th>
							<th class="px-4 py-3 text-center text-muted-foreground font-medium"
								>คะแนนรวม (เรียง)</th
							>
							<th class="px-4 py-3 text-center text-muted-foreground font-medium"
								>คะแนนรวมทุกวิชา</th
							>
							<th class="px-4 py-3 text-center text-muted-foreground font-medium">ห้องที่ได้</th>
						</tr>
					</thead>
					<tbody class="divide-y divide-border">
						{#each ranking.applications as app (app.applicationId)}
							<tr class="hover:bg-accent/20 transition-colors {app.rank <= 10 ? '' : ''}">
								<td class="px-4 py-2.5 text-center">
									<span
										class="
										inline-flex items-center justify-center w-7 h-7 rounded-full text-xs font-bold
										{app.rank === 1
											? 'bg-yellow-100 text-yellow-700'
											: app.rank <= 3
												? 'bg-gray-100 text-gray-700'
												: 'text-muted-foreground'}
									"
									>
										{app.rank}
									</span>
								</td>
								<td class="px-4 py-2.5 font-mono text-xs">{app.applicationNumber ?? '-'}</td>
								<td class="px-4 py-2.5 font-medium">{app.fullName}</td>
								<td class="px-4 py-2.5 text-center font-semibold text-primary"
									>{app.totalScore.toFixed(1)}</td
								>
								<td class="px-4 py-2.5 text-center text-muted-foreground"
									>{app.fullScore.toFixed(1)}</td
								>
								<td class="px-4 py-2.5 text-center">
									{#if app.assignedRoom}
										<span class="text-xs px-2 py-0.5 bg-blue-50 text-blue-700 rounded-full">
											{app.assignedRoom}
										</span>
									{:else}
										<span class="text-xs text-muted-foreground">เกินจำนวนที่รับ</span>
									{/if}
								</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		</div>
	{/if}
</div>
