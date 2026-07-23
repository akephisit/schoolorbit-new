<script lang="ts">
	import type { PageProps } from './$types';
	import { toast } from 'svelte-sonner';
	import {
		listChildExamSchedules,
		type PersonalExamScheduleRound
	} from '$lib/api/examSchedule';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import PersonalExamScheduleView from '$lib/components/academic/exam-schedule/PersonalExamScheduleView.svelte';

	let { data }: PageProps = $props();
	let studentId = $derived(data.studentId);
	let loading = $state(true);
	let error = $state('');
	let rounds = $state<PersonalExamScheduleRound[]>([]);
	let scheduleRequestToken = 0;

	async function loadSchedules(requestedStudentId: string) {
		const requestToken = ++scheduleRequestToken;
		loading = true;
		error = '';
		rounds = [];
		try {
			const nextRounds = await listChildExamSchedules(requestedStudentId);
			if (requestToken !== scheduleRequestToken) return;
			rounds = nextRounds;
		} catch (loadError: unknown) {
			if (requestToken !== scheduleRequestToken) return;
			console.error(loadError);
			error = loadError instanceof Error ? loadError.message : 'โหลดตารางสอบของนักเรียนไม่สำเร็จ';
			toast.error(error);
		} finally {
			if (requestToken === scheduleRequestToken) {
				loading = false;
			}
		}
	}

	$effect(() => {
		void loadSchedules(studentId);
	});
</script>

<svelte:head>
	<title>{data.title}</title>
</svelte:head>

<PageShell
	title={data.title}
	description="ตารางสอบที่ประกาศแล้วสำหรับนักเรียน"
	backHref={`/parent/student/${studentId}`}
>
	{#if loading}
		<PageSkeleton variant="table" rows={6} columns={7} />
	{:else if error}
		<PageState
			variant="error"
			title="โหลดตารางสอบไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={() => loadSchedules(studentId)}
		/>
	{:else}
		<PersonalExamScheduleView {rounds} />
	{/if}
</PageShell>
