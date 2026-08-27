<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import type { PageProps } from './$types';
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';
	import {
		listMyAcademicContextOptions,
		type AcademicContextOptionsResponse
	} from '$lib/api/academic-context';
	import { listMyExamSchedules, type PersonalExamScheduleRound } from '$lib/api/examSchedule';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import PersonalExamScheduleView from '$lib/components/academic/exam-schedule/PersonalExamScheduleView.svelte';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';

	let { data }: PageProps = $props();
	let contextOptions = $state<AcademicContextOptionsResponse | null>(null);
	let selectedYearId = $state('');
	let selectedTermId = $state('');
	let loading = $state(true);
	let error = $state('');
	let rounds = $state<PersonalExamScheduleRound[]>([]);

	const termOptions = $derived(
		contextOptions?.terms.filter((term) => term.academicYearId === selectedYearId) ?? []
	);

	function authorizedSelection(options: AcademicContextOptionsResponse): {
		yearId: string;
		termId: string;
	} {
		const queryYearId = page.url.searchParams.get('academicYearId');
		const yearId =
			options.years.find((year) => year.id === queryYearId)?.id ??
			options.years.find((year) => year.id === options.activeAcademicYearId)?.id ??
			options.years[0]?.id ??
			'';
		const terms = options.terms.filter((term) => term.academicYearId === yearId);
		const queryTermId = page.url.searchParams.get('academicTermId');
		const termId =
			terms.find((term) => term.id === queryTermId)?.id ??
			terms.find((term) => term.id === options.activeAcademicTermId)?.id ??
			terms[0]?.id ??
			'';
		return { yearId, termId };
	}

	async function loadHistory() {
		loading = true;
		error = '';
		try {
			contextOptions = await listMyAcademicContextOptions();
			const selection = authorizedSelection(contextOptions);
			selectedYearId = selection.yearId;
			selectedTermId = selection.termId;
			rounds = selectedTermId ? await listMyExamSchedules(selectedTermId) : [];
		} catch (loadError: unknown) {
			console.error(loadError);
			error = loadError instanceof Error ? loadError.message : 'โหลดตารางสอบไม่สำเร็จ';
			toast.error(error);
		} finally {
			loading = false;
		}
	}

	async function updateUrl(yearId: string, termId: string): Promise<void> {
		await goto(
			resolve(
				`/student/exams?academicYearId=${encodeURIComponent(yearId)}&academicTermId=${encodeURIComponent(termId)}`
			),
			{ noScroll: true, keepFocus: true }
		);
	}

	async function loadSchedules(termId: string): Promise<void> {
		loading = true;
		error = '';
		try {
			rounds = await listMyExamSchedules(termId);
		} catch (loadError) {
			error = loadError instanceof Error ? loadError.message : 'โหลดตารางสอบไม่สำเร็จ';
			toast.error(error);
		} finally {
			loading = false;
		}
	}

	async function changeYear(yearId: string): Promise<void> {
		const availableTerms =
			contextOptions?.terms.filter((term) => term.academicYearId === yearId) ?? [];
		const nextTerm =
			availableTerms.find((term) => term.id === contextOptions?.activeAcademicTermId) ??
			availableTerms[0];
		selectedYearId = yearId;
		selectedTermId = nextTerm?.id ?? '';
		rounds = [];
		if (!selectedTermId) return;
		await updateUrl(selectedYearId, selectedTermId);
		await loadSchedules(selectedTermId);
	}

	async function changeTerm(termId: string): Promise<void> {
		selectedTermId = termId;
		await updateUrl(selectedYearId, selectedTermId);
		await loadSchedules(selectedTermId);
	}

	onMount(loadHistory);
</script>

<PageShell title={data.title} description="ตารางสอบที่ประกาศแล้วสำหรับฉัน">
	<div class="flex flex-wrap gap-3 rounded-xl border bg-card p-4">
		<div class="min-w-52 space-y-2">
			<Label for="student-exam-year">ปีการศึกษา</Label>
			<Select.Root
				type="single"
				value={selectedYearId}
				disabled={loading}
				onValueChange={changeYear}
			>
				<Select.Trigger id="student-exam-year" class="w-full">
					{contextOptions?.years.find((year) => year.id === selectedYearId)?.name ??
						'เลือกปีการศึกษา'}
				</Select.Trigger>
				<Select.Content>
					{#each contextOptions?.years ?? [] as year (year.id)}
						<Select.Item value={year.id}>{year.name}</Select.Item>
					{/each}
				</Select.Content>
			</Select.Root>
		</div>
		<div class="min-w-52 space-y-2">
			<Label for="student-exam-term">ภาคเรียน</Label>
			<Select.Root
				type="single"
				value={selectedTermId}
				disabled={loading || termOptions.length === 0}
				onValueChange={changeTerm}
			>
				<Select.Trigger id="student-exam-term" class="w-full">
					{termOptions.find((term) => term.id === selectedTermId)?.name ?? 'เลือกภาคเรียน'}
				</Select.Trigger>
				<Select.Content>
					{#each termOptions as term (term.id)}
						<Select.Item value={term.id}>{term.name}</Select.Item>
					{/each}
				</Select.Content>
			</Select.Root>
		</div>
	</div>

	{#if loading}
		<PageSkeleton variant="table" rows={6} columns={7} />
	{:else if error}
		<PageState
			variant="error"
			title="โหลดตารางสอบไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={loadHistory}
		/>
	{:else if !contextOptions || contextOptions.years.length === 0}
		<PageState
			title="ยังไม่มีประวัติปีการศึกษา"
			description="เมื่อโรงเรียนสร้างข้อมูลนักเรียนประจำปีแล้ว ประวัติจะปรากฏที่นี่"
		/>
	{:else}
		<PersonalExamScheduleView {rounds} />
	{/if}
</PageShell>
