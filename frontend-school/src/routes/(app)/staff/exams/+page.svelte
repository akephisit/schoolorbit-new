<script lang="ts">
	import type { PageProps } from './$types';
	import { onDestroy, untrack } from 'svelte';
	import {
		listStaffExamSchedules,
		type StaffPublishedExamScheduleRound
	} from '$lib/api/examSchedule';
	import { LatestRequest, isAbortError } from '$lib/async/latest-request';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState, RegionUpdatingState } from '$lib/components/app-state';
	import StaffExamScheduleDashboard from '$lib/components/academic/exam-schedule/StaffExamScheduleDashboard.svelte';
	import { Button } from '$lib/components/ui/button';
	import { authStore } from '$lib/stores/auth';
	import { RefreshCw } from '@lucide/svelte';

	let { data }: PageProps = $props();
	const academicTermId = $derived(data.academicTermId);
	let loading = $state(true);
	let roundsLoaded = $state(false);
	let error = $state('');
	let rounds = $state<StaffPublishedExamScheduleRound[]>([]);
	let currentStaffId = $derived($authStore.user?.id ?? '');
	const request = new LatestRequest();
	let activeContextKey = '';
	let interactionRevision = 0;
	onDestroy(() => request.abort());

	async function loadSchedules() {
		const termId = academicTermId;
		if (!termId) return;
		const { revision, signal } = request.begin();
		interactionRevision += 1;
		loading = true;
		error = '';
		try {
			const loaded = await listStaffExamSchedules(termId, { signal });
			if (request.isCurrent(revision) && academicTermId === termId) {
				rounds = loaded;
				roundsLoaded = true;
			}
		} catch (loadError: unknown) {
			if (!isAbortError(loadError) && request.isCurrent(revision))
				error = loadError instanceof Error ? loadError.message : 'โหลดตารางสอบไม่สำเร็จ';
		} finally {
			if (request.isCurrent(revision)) loading = false;
		}
	}

	$effect.pre(() => {
		const contextKey = `${data.academicYearId}:${data.academicTermId}`;
		const routeRounds = data.rounds;
		const initialRevision = interactionRevision;
		let current = true;
		untrack(() => {
			if (activeContextKey !== contextKey) {
				activeContextKey = contextKey;
				rounds = [];
				roundsLoaded = false;
			}
			request.abort();
			loading = Boolean(routeRounds);
			error = '';
		});
		if (routeRounds) {
			void routeRounds.then((result) => {
				if (!current) return;
				untrack(() => {
					if (interactionRevision === initialRevision) {
						if (result.ok) {
							rounds = result.data;
							roundsLoaded = true;
						} else error = result.error;
						loading = false;
					}
				});
			});
		}
		return () => {
			current = false;
		};
	});
</script>

<PageShell title={data.title} description="ภาพรวมตารางสอบและการคุมสอบที่ประกาศแล้ว">
	{#snippet actions()}
		<Button
			variant="outline"
			size="sm"
			disabled={loading || !academicTermId}
			onclick={loadSchedules}
		>
			<RefreshCw class="size-4" /> รีเฟรช
		</Button>
	{/snippet}
	{#if !academicTermId}
		<PageState
			title="เลือกภาคเรียนก่อน"
			description="ใช้ตัวเลือกปีการศึกษาและภาคเรียนบนแถบด้านบน"
		/>
	{:else if loading && !roundsLoaded}
		<PageSkeleton variant="table" rows={7} columns={7} />
	{:else if error && !roundsLoaded}
		<PageState
			variant="error"
			title="โหลดตารางสอบไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={loadSchedules}
		/>
	{:else}
		<div class="relative space-y-4" aria-busy={loading} data-testid="staff-exams-ready">
			{#if loading}<RegionUpdatingState label="กำลังอัปเดตตารางสอบ..." />{/if}
			{#if error}
				<div
					role="alert"
					class="flex flex-wrap items-center gap-3 rounded-xl border border-destructive/30 bg-destructive/5 p-3 text-sm text-destructive"
				>
					<span>{error}</span>
					<Button variant="outline" size="sm" onclick={loadSchedules}>ลองใหม่</Button>
				</div>
			{/if}
			{#key academicTermId}
				<StaffExamScheduleDashboard {rounds} {currentStaffId} />
			{/key}
		</div>
	{/if}
</PageShell>
