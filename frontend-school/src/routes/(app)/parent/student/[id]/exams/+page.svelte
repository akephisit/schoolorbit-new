<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import type { PageProps } from './$types';
	import { toast } from 'svelte-sonner';
	import {
		listChildAcademicContextOptions,
		type AcademicContextOptionsResponse
	} from '$lib/api/academic-context';
	import { listChildExamSchedules, type PersonalExamScheduleRound } from '$lib/api/examSchedule';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import PersonalExamScheduleView from '$lib/components/academic/exam-schedule/PersonalExamScheduleView.svelte';
	import { Label } from '$lib/components/ui/label';

	let { data }: PageProps = $props();
	let studentId = $derived(data.studentId);
	let contextOptions = $state<AcademicContextOptionsResponse | null>(null);
	let selectedYearId = $state('');
	let selectedTermId = $state('');
	let loading = $state(true);
	let error = $state('');
	let rounds = $state<PersonalExamScheduleRound[]>([]);
	let scheduleRequestToken = 0;
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

	async function loadHistory(requestedStudentId: string) {
		const requestToken = ++scheduleRequestToken;
		loading = true;
		error = '';
		rounds = [];
		try {
			const options = await listChildAcademicContextOptions(requestedStudentId);
			if (requestToken !== scheduleRequestToken) return;
			contextOptions = options;
			const selection = authorizedSelection(options);
			selectedYearId = selection.yearId;
			selectedTermId = selection.termId;
			if (selectedTermId) {
				const nextRounds = await listChildExamSchedules(requestedStudentId, selectedTermId);
				if (requestToken !== scheduleRequestToken) return;
				rounds = nextRounds;
			}
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

	async function loadSchedules(termId: string): Promise<void> {
		const requestToken = ++scheduleRequestToken;
		loading = true;
		error = '';
		try {
			const nextRounds = await listChildExamSchedules(studentId, termId);
			if (requestToken === scheduleRequestToken) rounds = nextRounds;
		} catch (loadError) {
			if (requestToken === scheduleRequestToken) {
				error = loadError instanceof Error ? loadError.message : 'โหลดตารางสอบไม่สำเร็จ';
				toast.error(error);
			}
		} finally {
			if (requestToken === scheduleRequestToken) loading = false;
		}
	}

	async function updateUrl(yearId: string, termId: string): Promise<void> {
		await goto(
			resolve(
				`/parent/student/${studentId}/exams?academicYearId=${encodeURIComponent(yearId)}&academicTermId=${encodeURIComponent(termId)}`
			),
			{ noScroll: true, keepFocus: true }
		);
	}

	async function changeYear(event: Event): Promise<void> {
		const yearId = (event.currentTarget as HTMLSelectElement).value;
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

	async function changeTerm(event: Event): Promise<void> {
		selectedTermId = (event.currentTarget as HTMLSelectElement).value;
		await updateUrl(selectedYearId, selectedTermId);
		await loadSchedules(selectedTermId);
	}

	$effect(() => {
		void loadHistory(studentId);
	});
</script>

<PageShell
	title={data.title}
	description="ตารางสอบที่ประกาศแล้วสำหรับนักเรียน"
	backHref={`/parent/student/${studentId}`}
>
	<div class="flex flex-wrap gap-3 rounded-xl border bg-card p-4">
		<div class="min-w-52 space-y-2">
			<Label for="parent-exam-year">ปีการศึกษา</Label>
			<select
				id="parent-exam-year"
				class="border-input bg-background h-9 w-full rounded-md border px-3 text-sm"
				value={selectedYearId}
				disabled={loading}
				onchange={changeYear}
				>{#each contextOptions?.years ?? [] as year (year.id)}<option value={year.id}
						>{year.name}</option
					>{/each}</select
			>
		</div>
		<div class="min-w-52 space-y-2">
			<Label for="parent-exam-term">ภาคเรียน</Label>
			<select
				id="parent-exam-term"
				class="border-input bg-background h-9 w-full rounded-md border px-3 text-sm"
				value={selectedTermId}
				disabled={loading || termOptions.length === 0}
				onchange={changeTerm}
				>{#each termOptions as term (term.id)}<option value={term.id}>{term.name}</option
					>{/each}</select
			>
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
			onaction={() => loadHistory(studentId)}
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
