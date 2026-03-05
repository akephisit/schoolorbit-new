<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import {
		getRound,
		listTracks,
		listSubjects,
		listApplications,
		bulkUpdateScores,
		type AdmissionRound,
		type AdmissionTrack,
		type AdmissionExamSubject,
		type ApplicationListItem
	} from '$lib/api/admission';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { toast } from 'svelte-sonner';
	import { ArrowLeft, ClipboardList, Save } from 'lucide-svelte';

	let id = $derived($page.params.id);
	let round: AdmissionRound | null = $state(null);
	let tracks: AdmissionTrack[] = $state([]);
	let subjects: AdmissionExamSubject[] = $state([]);
	let applications: ApplicationListItem[] = $state([]);
	let loading = $state(true);
	let saving = $state(false);
	let selectedTrack = $state('');

	// local score map: { [appId]: { [subjectId]: score } }
	let scores: Record<string, Record<string, string>> = $state({});

	async function load() {
		if (!id) return;
		loading = true;
		try {
			const [r, t, s] = await Promise.all([getRound(id), listTracks(id), listSubjects(id)]);
			round = r;
			tracks = t;
			subjects = s;
			if (tracks.length > 0 && !selectedTrack) {
				selectedTrack = tracks[0].id;
			}
			await loadApps();
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'โหลดไม่สำเร็จ');
		} finally {
			loading = false;
		}
	}

	async function loadApps() {
		if (!id || !selectedTrack) return;
		const apps = await listApplications(id, { trackId: selectedTrack, status: 'verified' });
		applications = apps;
		// init scores map
		for (const app of apps) {
			if (!scores[app.id]) {
				scores[app.id] = {};
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
						.map(([subId, v]) => ({
							examSubjectId: subId,
							score: parseFloat(v)
						}))
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

	$effect(() => {
		if (selectedTrack) loadApps();
	});

	onMount(load);
</script>

<svelte:head>
	<title>กรอกคะแนน - SchoolOrbit</title>
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
		<p class="text-muted-foreground text-sm">{round.name} — รอบนี้มี {subjects.length} วิชา</p>
	{/if}

	<!-- Track Selector -->
	<div class="bg-card border border-border rounded-lg p-4 flex items-center gap-4">
		<label class="text-sm font-medium whitespace-nowrap">สายการเรียน:</label>
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
					<span class="ml-1 opacity-70">({track.applicationCount ?? 0})</span>
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
	{:else if applications.length === 0}
		<div class="bg-card border border-border rounded-lg p-10 text-center text-muted-foreground">
			<p>ไม่มีผู้สมัครที่ผ่านการตรวจสอบในสายนี้</p>
			<p class="text-xs mt-1">ต้องยืนยันใบสมัคร (status: verified) ก่อนกรอกคะแนน</p>
		</div>
	{:else}
		<!-- Score Table -->
		<div class="bg-card border border-border rounded-lg overflow-x-auto">
			<table class="w-full min-w-max text-sm">
				<thead class="bg-muted/50 border-b border-border">
					<tr>
						<th class="px-4 py-3 text-left font-medium text-muted-foreground w-10">ที่</th>
						<th class="px-4 py-3 text-left font-medium text-muted-foreground">เลขที่ใบสมัคร</th>
						<th class="px-4 py-3 text-left font-medium text-muted-foreground">ชื่อ-สกุล</th>
						{#each subjects as sub (sub.id)}
							<th class="px-3 py-3 text-center font-medium text-muted-foreground min-w-24">
								{sub.name}
								<span class="block text-xs font-normal text-muted-foreground/70"
									>/{sub.maxScore}</span
								>
							</th>
						{/each}
					</tr>
				</thead>
				<tbody class="divide-y divide-border">
					{#each applications as app, i (app.id)}
						<tr class="hover:bg-accent/20 transition-colors">
							<td class="px-4 py-2.5 text-muted-foreground text-center">{i + 1}</td>
							<td class="px-4 py-2.5 font-mono text-xs">{app.applicationNumber ?? '-'}</td>
							<td class="px-4 py-2.5 font-medium">{app.fullName}</td>
							{#each subjects as sub (sub.id)}
								<td class="px-3 py-2">
									<Input
										type="number"
										min="0"
										max={sub.maxScore}
										step="0.5"
										bind:value={scores[app.id][sub.id]}
										class="h-7 text-center text-sm w-20 mx-auto"
										placeholder="-"
									/>
								</td>
							{/each}
						</tr>
					{/each}
				</tbody>
			</table>
		</div>

		<div class="flex justify-end">
			<Button onclick={saveScores} disabled={saving} class="gap-2">
				<Save class="w-4 h-4" />
				{saving ? 'กำลังบันทึก...' : 'บันทึกคะแนนทั้งหมด'}
			</Button>
		</div>
	{/if}
</div>
