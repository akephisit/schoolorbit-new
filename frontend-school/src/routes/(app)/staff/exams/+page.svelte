<script lang="ts">
	import type { PageProps } from './$types';
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';
	import { getAcademicContextStore } from '$lib/academic-context/store';
	import {
		listStaffExamSchedules,
		type StaffPublishedExamScheduleRound
	} from '$lib/api/examSchedule';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import StaffExamScheduleDashboard from '$lib/components/academic/exam-schedule/StaffExamScheduleDashboard.svelte';
	import { authStore } from '$lib/stores/auth';

	let { data }: PageProps = $props();
	const academicContext = getAcademicContextStore();
	const academicTermId = $derived($academicContext.selected.academicTermId);
	let loading = $state(true);
	let error = $state('');
	let rounds = $state<StaffPublishedExamScheduleRound[]>([]);
	let currentStaffId = $derived($authStore.user?.id ?? '');

	async function loadSchedules(termId: string) {
		loading = true;
		error = '';
		try {
			rounds = await listStaffExamSchedules(termId);
		} catch (loadError: unknown) {
			console.error(loadError);
			error = loadError instanceof Error ? loadError.message : 'โหลดตารางสอบไม่สำเร็จ';
			toast.error(error);
		} finally {
			loading = false;
		}
	}

	onMount(() => {
		let loadedTermId: string | null = null;
		return academicContext.subscribe((state) => {
			const termId = state.selected.academicTermId;
			if (termId && termId !== loadedTermId) {
				loadedTermId = termId;
				void loadSchedules(termId);
			}
		});
	});
</script>

<PageShell title={data.title} description="ภาพรวมตารางสอบและการคุมสอบที่ประกาศแล้ว">
	{#if !academicTermId}
		<PageState
			title="เลือกภาคเรียนก่อน"
			description="ใช้ตัวเลือกปีการศึกษาและภาคเรียนบนแถบด้านบน"
		/>
	{:else if loading}
		<PageSkeleton variant="table" rows={7} columns={7} />
	{:else if error}
		<PageState
			variant="error"
			title="โหลดตารางสอบไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={() => loadSchedules(academicTermId)}
		/>
	{:else}
		<StaffExamScheduleDashboard {rounds} {currentStaffId} />
	{/if}
</PageShell>
