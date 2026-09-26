<script lang="ts">
	import { invalidate } from '$app/navigation';
	import { untrack } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import {
		createAcademicTerm,
		createAcademicYear,
		createBellSchedule,
		listBellSchedulePeriods,
		listBellSchedules,
		replaceBellSchedulePeriods,
		type AcademicTerm,
		type AcademicYear,
		type BellSchedule,
		updateAcademicTerm,
		updateAcademicYear,
		updateBellSchedule
	} from '$lib/api/academic-core';
	import { ACADEMIC_SETUP_WORKSPACE_DEPENDENCY } from '$lib/academic-core/foundation-route';
	import AcademicYearTermEditor from '$lib/components/academic-core/AcademicYearTermEditor.svelte';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState, RegionUpdatingState } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	let years = $state<AcademicYear[]>([]);
	const termsByYear = new SvelteMap<string, AcademicTerm[]>();
	let bellSchedules = $state<BellSchedule[]>([]);
	let loading = $state(true);
	let workspaceReady = $state(false);
	let busy = $state(false);
	let errorMessage = $state('');
	let mutationRevision = 0;
	const canRead = $derived(
		$can.hasAny(PERMISSIONS.ACADEMIC_YEAR_READ_SCHOOL, PERMISSIONS.ACADEMIC_YEAR_MANAGE_SCHOOL)
	);
	const canManage = $derived(
		$can.hasAll(PERMISSIONS.ACADEMIC_YEAR_MANAGE_SCHOOL, PERMISSIONS.ACADEMIC_TERM_MANAGE_SCHOOL)
	);

	function loadWorkspace() {
		return invalidate(ACADEMIC_SETUP_WORKSPACE_DEPENDENCY);
	}

	$effect.pre(() => {
		const routeResult = data.workspace;
		const initialMutationRevision = mutationRevision;
		let current = true;
		untrack(() => {
			loading = Boolean(routeResult);
			errorMessage = '';
		});
		if (routeResult) {
			void routeResult.then((result) => {
				if (!current) return;
				untrack(() => {
					if (result.ok && mutationRevision === initialMutationRevision) {
						const workspace = result.data;
						const nextYears = workspace.years.toSorted((a, b) => b.year - a.year);
						const nextTerms = new SvelteMap(
							nextYears.map((year) => [year.id, [] as AcademicTerm[]])
						);
						for (const term of workspace.terms) nextTerms.get(term.academicYearId)?.push(term);
						for (const terms of nextTerms.values()) terms.sort((a, b) => a.sequence - b.sequence);
						years = nextYears;
						termsByYear.clear();
						for (const [yearId, terms] of nextTerms) termsByYear.set(yearId, terms);
						bellSchedules = workspace.bellSchedules;
						workspaceReady = true;
					} else if (!result.ok) errorMessage = result.error;
					loading = false;
				});
			});
		}
		return () => {
			current = false;
		};
	});

	async function addYear(draft: Parameters<typeof createAcademicYear>[0]) {
		busy = true;
		try {
			const created = await createAcademicYear(draft);
			mutationRevision += 1;
			years = [created, ...years].sort((a, b) => b.year - a.year);
			termsByYear.set(created.id, []);
			return created;
		} finally {
			busy = false;
		}
	}

	async function addTerm(draft: Parameters<typeof createAcademicTerm>[0]) {
		busy = true;
		try {
			const created = await createAcademicTerm(draft);
			mutationRevision += 1;
			termsByYear.set(
				created.academicYearId,
				[...(termsByYear.get(created.academicYearId) ?? []), created].sort(
					(a, b) => a.sequence - b.sequence
				)
			);
			return created;
		} finally {
			busy = false;
		}
	}

	async function editYear(id: string, draft: Parameters<typeof updateAcademicYear>[1]) {
		busy = true;
		try {
			const updated = await updateAcademicYear(id, draft);
			mutationRevision += 1;
			years = years
				.map((year) => (year.id === updated.id ? updated : year))
				.sort((a, b) => b.year - a.year);
			return updated;
		} finally {
			busy = false;
		}
	}

	async function editTerm(id: string, draft: Parameters<typeof updateAcademicTerm>[1]) {
		busy = true;
		try {
			const updated = await updateAcademicTerm(id, draft);
			mutationRevision += 1;
			termsByYear.set(
				updated.academicYearId,
				(termsByYear.get(updated.academicYearId) ?? [])
					.map((term) => (term.id === updated.id ? updated : term))
					.sort((a, b) => a.sequence - b.sequence)
			);
			return updated;
		} finally {
			busy = false;
		}
	}

	async function addBellSchedule(draft: Parameters<typeof createBellSchedule>[0]) {
		busy = true;
		try {
			const created = await createBellSchedule(draft);
			mutationRevision += 1;
			bellSchedules = [...bellSchedules, created];
			return created;
		} finally {
			busy = false;
		}
	}

	async function editBellSchedule(id: string, draft: Parameters<typeof updateBellSchedule>[1]) {
		busy = true;
		try {
			const updated = await updateBellSchedule(id, draft);
			mutationRevision += 1;
			const refreshed = await listBellSchedules(updated.academicYearId);
			bellSchedules = [
				...bellSchedules.filter((item) => item.academicYearId !== updated.academicYearId),
				...refreshed
			];
			return refreshed.find((item) => item.id === id) ?? updated;
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
			mutationRevision += 1;
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
	{:else if loading && !workspaceReady}
		<PageSkeleton variant="cards" rows={4} />
	{:else if errorMessage && !workspaceReady}
		<PageState
			variant="error"
			title="โหลดโครงสร้างไม่สำเร็จ"
			description={errorMessage}
			actionLabel="ลองอีกครั้ง"
			onaction={loadWorkspace}
		/>
	{:else}
		<section
			class="relative"
			aria-label="ตั้งค่าปีและภาคเรียน"
			aria-busy={loading}
			data-testid="academic-setup-ready"
		>
			{#if loading}<RegionUpdatingState label="กำลังอัปเดตโครงสร้างปีการศึกษา" />{/if}
			<AcademicYearTermEditor
				{years}
				{termsByYear}
				{bellSchedules}
				{canManage}
				{busy}
				onCreateYear={addYear}
				onUpdateYear={editYear}
				onCreateBellSchedule={addBellSchedule}
				onUpdateBellSchedule={editBellSchedule}
				onLoadBellSchedulePeriods={listBellSchedulePeriods}
				onReplaceBellSchedulePeriods={saveBellSchedulePeriods}
				onCreateTerm={addTerm}
				onUpdateTerm={editTerm}
			/>
			{#if errorMessage}<div
					role="alert"
					class="flex flex-wrap items-center gap-2 text-sm text-destructive"
				>
					<p>{errorMessage}</p>
					<Button size="sm" variant="outline" onclick={loadWorkspace}>ลองอีกครั้ง</Button>
				</div>{/if}
		</section>
	{/if}
</PageShell>
