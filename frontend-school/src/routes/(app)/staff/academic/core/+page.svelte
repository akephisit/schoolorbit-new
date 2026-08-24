<script lang="ts">
	import { onMount } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import {
		createAcademicTerm,
		createAcademicYear,
		createBellSchedule,
		listAcademicTerms,
		listAcademicYears,
		listBellSchedulePeriods,
		listBellSchedules,
		replaceBellSchedulePeriods,
		type AcademicTerm,
		type AcademicYear,
		type BellSchedule,
		updateAcademicTerm,
		updateAcademicYear
	} from '$lib/api/academic-core';
	import AcademicYearTermEditor from '$lib/components/academic-core/AcademicYearTermEditor.svelte';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';

	let years = $state<AcademicYear[]>([]);
	const termsByYear = new SvelteMap<string, AcademicTerm[]>();
	let bellSchedules = $state<BellSchedule[]>([]);
	let loading = $state(true);
	let busy = $state(false);
	let errorMessage = $state('');
	const canRead = $derived(
		$can.hasAny(PERMISSIONS.ACADEMIC_YEAR_READ_SCHOOL, PERMISSIONS.ACADEMIC_YEAR_MANAGE_SCHOOL)
	);
	const canManage = $derived(
		$can.hasAll(PERMISSIONS.ACADEMIC_YEAR_MANAGE_SCHOOL, PERMISSIONS.ACADEMIC_TERM_MANAGE_SCHOOL)
	);

	async function loadWorkspace() {
		loading = true;
		errorMessage = '';
		try {
			years = (await listAcademicYears()).sort((a, b) => b.year - a.year);
			const nextTerms = new SvelteMap<string, AcademicTerm[]>();
			const nextBellSchedules: BellSchedule[] = [];
			for (const year of years) {
				const terms = await listAcademicTerms(year.id);
				nextTerms.set(
					year.id,
					terms.sort((a, b) => a.sequence - b.sequence)
				);
				const schedules = await listBellSchedules(year.id);
				for (const schedule of schedules) nextBellSchedules.push(schedule);
			}
			termsByYear.clear();
			for (const [yearId, terms] of nextTerms) termsByYear.set(yearId, terms);
			bellSchedules = nextBellSchedules;
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'โหลดโครงสร้างปีการศึกษาไม่สำเร็จ';
		} finally {
			loading = false;
		}
	}

	async function addYear(draft: {
		year: number;
		name: string;
		startDate: string;
		endDate: string;
	}) {
		busy = true;
		try {
			const created = await createAcademicYear({
				...draft,
				schoolDays: ['MON', 'TUE', 'WED', 'THU', 'FRI']
			});
			years = [created, ...years].sort((a, b) => b.year - a.year);
			termsByYear.set(created.id, []);
		} finally {
			busy = false;
		}
	}

	async function addTerm(draft: Parameters<typeof createAcademicTerm>[0]) {
		busy = true;
		try {
			const created = await createAcademicTerm(draft);
			termsByYear.set(
				created.academicYearId,
				[...(termsByYear.get(created.academicYearId) ?? []), created].sort(
					(a, b) => a.sequence - b.sequence
				)
			);
		} finally {
			busy = false;
		}
	}

	async function editYear(id: string, draft: Parameters<typeof updateAcademicYear>[1]) {
		busy = true;
		try {
			const updated = await updateAcademicYear(id, draft);
			years = years
				.map((year) => (year.id === updated.id ? updated : year))
				.sort((a, b) => b.year - a.year);
		} finally {
			busy = false;
		}
	}

	async function editTerm(id: string, draft: Parameters<typeof updateAcademicTerm>[1]) {
		busy = true;
		try {
			const updated = await updateAcademicTerm(id, draft);
			termsByYear.set(
				updated.academicYearId,
				(termsByYear.get(updated.academicYearId) ?? [])
					.map((term) => (term.id === updated.id ? updated : term))
					.sort((a, b) => a.sequence - b.sequence)
			);
		} finally {
			busy = false;
		}
	}

	async function addBellSchedule(draft: Parameters<typeof createBellSchedule>[0]) {
		busy = true;
		try {
			const created = await createBellSchedule({ ...draft, owningOrganizationUnitId: null });
			bellSchedules = [...bellSchedules, created];
		} finally {
			busy = false;
		}
	}

	async function saveBellSchedulePeriods(
		id: string,
		draft: Parameters<typeof replaceBellSchedulePeriods>[1]
	) {
		busy = true;
		try {
			const periods = await replaceBellSchedulePeriods(id, draft);
			const schedule = bellSchedules.find((item) => item.id === id);
			if (schedule) {
				const refreshed = await listBellSchedules(schedule.academicYearId);
				bellSchedules = [
					...bellSchedules.filter((item) => item.academicYearId !== schedule.academicYearId),
					...refreshed
				];
			}
			return periods;
		} finally {
			busy = false;
		}
	}

	onMount(loadWorkspace);
</script>

<PageShell
	title="ตั้งค่าปีและภาคเรียน"
	description="กำหนดรอบการศึกษาแบบยืดหยุ่น รองรับภาคปกติ ฤดูร้อน ซ่อมเสริม และรอบกำหนดเอง"
>
	{#if !canRead}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ดูปีการศึกษา"
			description="ต้องมีสิทธิ์อ่านหรือจัดการปีการศึกษาระดับโรงเรียน"
		/>
	{:else if loading}
		<PageSkeleton variant="cards" rows={4} />
	{:else if errorMessage}
		<PageState
			variant="error"
			title="โหลดโครงสร้างไม่สำเร็จ"
			description={errorMessage}
			actionLabel="ลองอีกครั้ง"
			onaction={loadWorkspace}
		/>
	{:else}
		<AcademicYearTermEditor
			{years}
			{termsByYear}
			{bellSchedules}
			{canManage}
			{busy}
			onCreateYear={addYear}
			onUpdateYear={editYear}
			onCreateBellSchedule={addBellSchedule}
			onLoadBellSchedulePeriods={listBellSchedulePeriods}
			onReplaceBellSchedulePeriods={saveBellSchedulePeriods}
			onCreateTerm={addTerm}
			onUpdateTerm={editTerm}
		/>
	{/if}
</PageShell>
