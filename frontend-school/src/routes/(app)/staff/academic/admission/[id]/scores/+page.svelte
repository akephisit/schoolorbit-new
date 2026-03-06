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
		type ApplicationListItem,
		getAllScores
	} from '$lib/api/admission';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import * as Card from '$lib/components/ui/card';
	import * as Table from '$lib/components/ui/table';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
	import { toast } from 'svelte-sonner';
	import { ArrowLeft, ClipboardList, Save, Loader2, ListFilter } from 'lucide-svelte';

	let { data } = $props();
	let id = $derived($page.params.id);

	let round: AdmissionRound | null = $state(null);
	let tracks: AdmissionTrack[] = $state([]);
	let subjects: AdmissionExamSubject[] = $state([]);
	let applications: ApplicationListItem[] = $state([]);
	let loading = $state(true);
	let saving = $state(false);
	let selectedTrack = $state('');
	let allRawScores: any[] = [];
	let visibleSubjectIds: string[] = $state([]);

	let scores: Record<string, Record<string, string>> = $state({});

	async function loadAll() {
		if (!id) return;
		loading = true;
		try {
			const [r, t, s, allS] = await Promise.all([
				getRound(id),
				listTracks(id),
				listSubjects(id),
				getAllScores(id)
			]);
			round = r;
			tracks = t;
			subjects = s;
			visibleSubjectIds = s.map((sub) => sub.id);
			allRawScores = allS as any[];

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
		// Only show applications that are verified or scored
		applications = allApps.filter((a) => ['verified', 'scored', 'accepted'].includes(a.status));

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

	function handleKeydown(e: KeyboardEvent, appIndex: number, subId: string) {
		if (e.key === 'Enter') {
			e.preventDefault();
			// Attempt to focus the input in the same column but next row
			const nextAppId = applications[appIndex + 1]?.id;
			if (nextAppId) {
				const nextInput = document.getElementById(`score-${nextAppId}-${subId}`);
				if (nextInput) {
					nextInput.focus();
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

	<!-- Track Selector + Filters -->
	<Card.Root>
		<Card.Content class="pt-4 pb-4 flex flex-col sm:flex-row sm:items-center gap-4 justify-between">
			<div class="flex items-center gap-4">
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
			</div>

			<DropdownMenu.Root>
				<DropdownMenu.Trigger>
					{#snippet child({ props })}
						<Button {...props} variant="outline" size="sm" class="gap-2 shrink-0">
							<ListFilter class="w-4 h-4" /> เลือกวิชา
						</Button>
					{/snippet}
				</DropdownMenu.Trigger>
				<DropdownMenu.Content align="end">
					{#each subjects as sub (sub.id)}
						<DropdownMenu.CheckboxItem
							checked={visibleSubjectIds.includes(sub.id)}
							onCheckedChange={(v) => {
								if (v) visibleSubjectIds = [...visibleSubjectIds, sub.id];
								else visibleSubjectIds = visibleSubjectIds.filter((id) => id !== sub.id);
							}}
						>
							{sub.name}
						</DropdownMenu.CheckboxItem>
					{/each}
				</DropdownMenu.Content>
			</DropdownMenu.Root>
		</Card.Content>
	</Card.Root>

	{#if loading}
		<Card.Root>
			<Card.Content class="flex justify-center py-16">
				<Loader2 class="w-8 h-8 animate-spin text-primary" />
			</Card.Content>
		</Card.Root>
	{:else if applications.length === 0}
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
						<Table.Head>เลขที่ใบสมัคร</Table.Head>
						<Table.Head>ชื่อ-สกุล</Table.Head>
						{#each subjects.filter((s) => visibleSubjectIds.includes(s.id)) as sub (sub.id)}
							<Table.Head class="text-center min-w-24">
								{sub.name}
								<span class="block text-xs font-normal text-muted-foreground">/{sub.maxScore}</span>
							</Table.Head>
						{/each}
					</Table.Row>
				</Table.Header>
				<Table.Body>
					{#each applications as app, i (app.id)}
						<Table.Row>
							<Table.Cell class="text-center text-muted-foreground">{i + 1}</Table.Cell>
							<Table.Cell class="font-mono text-xs">{app.applicationNumber ?? '-'}</Table.Cell>
							<Table.Cell class="font-medium">{app.fullName}</Table.Cell>
							{#each subjects.filter((s) => visibleSubjectIds.includes(s.id)) as sub (sub.id)}
								<Table.Cell class="px-2 py-1.5">
									<Input
										id="score-{app.id}-{sub.id}"
										type="number"
										min="0"
										max={sub.maxScore}
										step="0.5"
										bind:value={scores[app.id][sub.id]}
										onkeydown={(e) => handleKeydown(e, i, sub.id)}
										class="h-7 text-center text-sm w-20 mx-auto"
										placeholder="-"
									/>
								</Table.Cell>
							{/each}
						</Table.Row>
					{/each}
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
