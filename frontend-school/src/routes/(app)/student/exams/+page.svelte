<script lang="ts">
	import type { PageProps } from './$types';
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';
	import {
		listMyExamSchedules,
		type PersonalExamScheduleRound
	} from '$lib/api/examSchedule';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import PersonalExamScheduleView from '$lib/components/academic/exam-schedule/PersonalExamScheduleView.svelte';

	let { data }: PageProps = $props();
	let loading = $state(true);
	let error = $state('');
	let rounds = $state<PersonalExamScheduleRound[]>([]);

	async function loadSchedules() {
		loading = true;
		error = '';
		try {
			rounds = await listMyExamSchedules();
		} catch (loadError: unknown) {
			console.error(loadError);
			error = loadError instanceof Error ? loadError.message : 'โหลดตารางสอบไม่สำเร็จ';
			toast.error(error);
		} finally {
			loading = false;
		}
	}

	onMount(loadSchedules);
</script>

<svelte:head>
	<title>{data.title}</title>
</svelte:head>

<PageShell title={data.title} description="ตารางสอบที่ประกาศแล้วสำหรับฉัน">
	{#if loading}
		<PageSkeleton variant="table" rows={6} columns={7} />
	{:else if error}
		<PageState
			variant="error"
			title="โหลดตารางสอบไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={loadSchedules}
		/>
	{:else}
		<PersonalExamScheduleView {rounds} />
	{/if}
</PageShell>
