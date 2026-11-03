<script lang="ts">
	import { ApiClientError } from '#lib/api/client.js';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import { authStore } from '#lib/stores/auth.js';
	import { untrack } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import {
		getAcademicSetupWorkspace,
		updateAcademicContextSettings,
		type AcademicContextSettings,
		type UpdateAcademicContextSettingsRequest,
		createAcademicTerm,
		createAcademicYear,
		createBellSchedule,
		cloneBellSchedule,
		listBellSchedulePeriods,
		listBellSchedules,
		replaceBellSchedulePeriods,
		type AcademicTerm,
		type AcademicYear,
		type BellSchedule,
		type BellSchedulePeriodsSnapshot,
		updateAcademicTerm,
		updateAcademicYear,
		updateBellSchedule
	} from '#lib/api/academic-core.js';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { getAcademicContextStore } from '#lib/academic-context/store.js';

	import AcademicYearTermEditor from '#lib/components/academic-core/AcademicYearTermEditor.svelte';
	import TimeSetupEditor from '#lib/components/academic-core/TimeSetupEditor.svelte';
	import type { RouteLoadResult } from '#lib/navigation/route-load.js';
	import type { AcademicSetupWorkspace } from '#lib/api/academic-core.js';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';

	let {
		workspaceLoad,
		mode = 'year'
	}: {
		workspaceLoad: Promise<RouteLoadResult<AcademicSetupWorkspace | null>>;
		mode?: 'year' | 'time';
	} = $props();
	let years = $state<AcademicYear[]>([]);
	const termsByYear = new SvelteMap<string, AcademicTerm[]>();
	let bellSchedules = $state<BellSchedule[]>([]);
	let bellScheduleSetupYearIds = $state<string[]>([]);
	let bellScheduleSetupTermIds = $state<string[]>([]);
	let loading = $state(true);
	let workspaceReady = $state(false);
	let busy = $state({ year: false, term: false, schedule: false, periods: false });
	let contextSettings = $state<AcademicContextSettings | null>(null);
	const academicContext = getAcademicContextStore();
	let errorMessage = $state('');
	let mutationRevision = 0;
	let regionIdentity = '';
	const routeLoadOwners = new WeakMap<
		Promise<RouteLoadResult<AcademicSetupWorkspace | null>>,
		string
	>();
	const canRead = $derived(
		$can.hasAny(PERMISSIONS.ACADEMIC_YEAR_READ_SCHOOL, PERMISSIONS.ACADEMIC_YEAR_MANAGE_SCHOOL)
	);
	const canManage = $derived(
		mode === 'time'
			? $can.has(PERMISSIONS.ACADEMIC_TERM_MANAGE_SCHOOL)
			: $can.hasAll(
					PERMISSIONS.ACADEMIC_YEAR_MANAGE_SCHOOL,
					PERMISSIONS.ACADEMIC_TERM_MANAGE_SCHOOL
				)
	);

	const latest = new LatestRequest();
	function applyWorkspace(workspace: AcademicSetupWorkspace) {
		const nextYears = workspace.years.toSorted((a, b) => b.year - a.year);
		const nextTerms = new SvelteMap(nextYears.map((year) => [year.id, [] as AcademicTerm[]]));
		for (const term of workspace.terms) nextTerms.get(term.academicYearId)?.push(term);
		for (const terms of nextTerms.values()) terms.sort((a, b) => a.sequence - b.sequence);
		years = nextYears;
		termsByYear.clear();
		for (const [yearId, terms] of nextTerms) termsByYear.set(yearId, terms);
		bellSchedules = workspace.bellSchedules;
		bellScheduleSetupYearIds = workspace.bellScheduleSetupYearIds;
		bellScheduleSetupTermIds = workspace.bellScheduleSetupTermIds;
		contextSettings = workspace.contextSettings;
		workspaceReady = true;
	}
	async function loadWorkspace() {
		if (!canRead) return;
		const { revision, signal } = latest.begin();
		const identity = appIdentityKey();
		const initialMutationRevision = mutationRevision;
		loading = true;
		errorMessage = '';
		try {
			const result = await getAcademicSetupWorkspace({ signal });
			if (
				latest.isCurrent(revision) &&
				identity === appIdentityKey() &&
				mutationRevision === initialMutationRevision
			)
				applyWorkspace(result);
		} catch (cause) {
			if (latest.isCurrent(revision) && !isAbortError(cause))
				errorMessage = cause instanceof Error ? cause.message : 'โหลดตั้งค่าไม่สำเร็จ';
		} finally {
			if (latest.isCurrent(revision)) loading = false;
		}
	}
	async function reloadContextSettings() {
		await loadWorkspace();
		if (errorMessage) throw new Error(errorMessage);
	}

	export async function refreshAfterLifecycle() {
		await reloadContextSettings();
	}

	$effect.pre(() => {
		const routeResult = workspaceLoad;
		const identityState = $authStore;
		const identity = appIdentityKey();
		if (routeResult && identityState.isAuthenticated && !routeLoadOwners.has(routeResult))
			routeLoadOwners.set(routeResult, identity);
		const ownsRouteResult = !!routeResult && routeLoadOwners.get(routeResult) === identity;
		const { revision } = latest.begin();
		const initialMutationRevision = mutationRevision;
		let current = true;
		untrack(() => {
			loading = Boolean(routeResult);
			if (!identityState.isAuthenticated || identity !== regionIdentity) {
				years = [];
				termsByYear.clear();
				bellSchedules = [];
				bellScheduleSetupYearIds = [];
				bellScheduleSetupTermIds = [];
				contextSettings = null;
				workspaceReady = false;
				busy = { year: false, term: false, schedule: false, periods: false };
			}
			errorMessage = '';
			regionIdentity = identity;
		});
		if (routeResult && ownsRouteResult) {
			void routeResult.then((result) => {
				if (!current || !latest.isCurrent(revision) || identity !== appIdentityKey()) return;
				untrack(() => {
					if (result.ok && result.data && mutationRevision === initialMutationRevision) {
						applyWorkspace(result.data);
					} else if (!result.ok) errorMessage = result.error;
					loading = false;
				});
			});
		} else
			untrack(() => {
				loading = false;
			});
		return () => {
			current = false;
			latest.abort();
		};
	});

	async function saveContextSettings(draft: UpdateAcademicContextSettingsRequest) {
		const identity = appIdentityKey();
		const result = await updateAcademicContextSettings(draft);
		if (identity !== appIdentityKey()) return result;
		mutationRevision += 1;
		if (contextSettings && contextSettings.rowVersion > result.settings.rowVersion) return result;
		contextSettings = result.settings;
		const changedYears = new Map(result.years.map((year) => [year.id, year]));
		years = years.map((year) => {
			const updated = changedYears.get(year.id);
			return updated && updated.rowVersion >= year.rowVersion ? updated : year;
		});
		for (const term of result.terms) {
			termsByYear.set(
				term.academicYearId,
				(termsByYear.get(term.academicYearId) ?? []).map((current) =>
					current.id === term.id && term.rowVersion >= current.rowVersion ? term : current
				)
			);
		}
		academicContext.replaceOptions(result.contextOptions);
		return result;
	}

	async function addYear(draft: Parameters<typeof createAcademicYear>[0]) {
		const identity = appIdentityKey();
		busy.year = true;
		try {
			const created = await createAcademicYear(draft);
			if (identity !== appIdentityKey()) return created;
			mutationRevision += 1;
			years = [created, ...years].sort((a, b) => b.year - a.year);
			termsByYear.set(created.id, []);
			return created;
		} finally {
			if (identity === appIdentityKey()) busy.year = false;
		}
	}

	async function addTerm(draft: Parameters<typeof createAcademicTerm>[0]) {
		const identity = appIdentityKey();
		busy.term = true;
		try {
			const created = await createAcademicTerm(draft);
			if (identity !== appIdentityKey()) return created;
			mutationRevision += 1;
			termsByYear.set(
				created.academicYearId,
				[...(termsByYear.get(created.academicYearId) ?? []), created].sort(
					(a, b) => a.sequence - b.sequence
				)
			);
			return created;
		} finally {
			if (identity === appIdentityKey()) busy.term = false;
		}
	}

	async function editYear(id: string, draft: Parameters<typeof updateAcademicYear>[1]) {
		const identity = appIdentityKey();
		busy.year = true;
		try {
			const updated = await updateAcademicYear(id, draft);
			if (identity !== appIdentityKey()) return updated;
			mutationRevision += 1;
			years = years
				.map((year) =>
					year.id === updated.id && updated.rowVersion >= year.rowVersion ? updated : year
				)
				.sort((a, b) => b.year - a.year);
			return updated;
		} finally {
			if (identity === appIdentityKey()) busy.year = false;
		}
	}

	async function editTerm(id: string, draft: Parameters<typeof updateAcademicTerm>[1]) {
		const identity = appIdentityKey();
		busy.term = true;
		try {
			const updated = await updateAcademicTerm(id, draft);
			if (identity !== appIdentityKey()) return updated;
			mutationRevision += 1;
			termsByYear.set(
				updated.academicYearId,
				(termsByYear.get(updated.academicYearId) ?? [])
					.map((term) =>
						term.id === updated.id && updated.rowVersion >= term.rowVersion ? updated : term
					)
					.sort((a, b) => a.sequence - b.sequence)
			);
			return updated;
		} finally {
			if (identity === appIdentityKey()) busy.term = false;
		}
	}

	async function addBellSchedule(draft: Parameters<typeof createBellSchedule>[0]) {
		const identity = appIdentityKey();
		busy.schedule = true;
		try {
			const created = await createBellSchedule(draft);
			if (identity !== appIdentityKey()) return created;
			mutationRevision += 1;
			bellSchedules = [...bellSchedules, created];
			return created;
		} finally {
			if (identity === appIdentityKey()) busy.schedule = false;
		}
	}
	async function copyBellSchedule(id: string, draft: Parameters<typeof cloneBellSchedule>[1]) {
		const identity = appIdentityKey();
		busy.schedule = true;
		try {
			const created = await cloneBellSchedule(id, draft);
			if (identity !== appIdentityKey()) return created;
			mutationRevision += 1;
			bellSchedules = [...bellSchedules, created];
			return created;
		} finally {
			if (identity === appIdentityKey()) busy.schedule = false;
		}
	}

	async function editBellSchedule(id: string, draft: Parameters<typeof updateBellSchedule>[1]) {
		const identity = appIdentityKey();
		busy.schedule = true;
		try {
			const updated = await updateBellSchedule(id, draft);
			if (identity !== appIdentityKey()) return updated;
			mutationRevision += 1;
			const refreshed = await listBellSchedules(updated.academicYearId);
			if (identity !== appIdentityKey()) return updated;
			bellSchedules = [
				...bellSchedules.filter((item) => item.academicYearId !== updated.academicYearId),
				...refreshed
			];
			return refreshed.find((item) => item.id === id) ?? updated;
		} finally {
			if (identity === appIdentityKey()) busy.schedule = false;
		}
	}

	function applyPeriodSnapshot(snapshot: BellSchedulePeriodsSnapshot, submittedBase: number) {
		bellSchedules = bellSchedules.map((schedule) =>
			schedule.id === snapshot.bellScheduleId &&
			schedule.rowVersion === submittedBase &&
			snapshot.rowVersion > schedule.rowVersion
				? { ...schedule, rowVersion: snapshot.rowVersion }
				: schedule
		);
	}

	async function loadBellSchedulePeriods(
		id: string,
		options: Parameters<typeof listBellSchedulePeriods>[1] = {}
	) {
		const identity = appIdentityKey();
		const snapshot = await listBellSchedulePeriods(id, options);
		if (identity !== appIdentityKey()) return snapshot;
		mutationRevision += 1;
		return snapshot;
	}

	async function saveBellSchedulePeriods(
		id: string,
		draft: Parameters<typeof replaceBellSchedulePeriods>[1]
	) {
		const identity = appIdentityKey();
		busy.periods = true;
		try {
			const snapshot = await replaceBellSchedulePeriods(id, draft);
			if (identity !== appIdentityKey()) return snapshot;
			mutationRevision += 1;
			applyPeriodSnapshot(snapshot, draft.rowVersion);
			return snapshot;
		} catch (cause) {
			if (
				cause instanceof ApiClientError &&
				cause.status === 409 &&
				identity === appIdentityKey()
			) {
				const schedule = bellSchedules.find((item) => item.id === id);
				if (schedule) {
					const refreshed = await listBellSchedules(schedule.academicYearId);
					if (identity === appIdentityKey()) {
						const versions = new Map(bellSchedules.map((item) => [item.id, item]));
						bellSchedules = [
							...bellSchedules.filter((item) => item.academicYearId !== schedule.academicYearId),
							...refreshed.map((item) => {
								const current = versions.get(item.id);
								return current && current.rowVersion > item.rowVersion ? current : item;
							})
						];
					}
				}
			}
			throw cause;
		} finally {
			if (identity === appIdentityKey()) busy.periods = false;
		}
	}
</script>

<div class="space-y-4">
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
			aria-label={mode === 'time' ? 'ข้อมูลเวลาเรียน' : 'ข้อมูลตั้งค่ารอบการศึกษา'}
			aria-busy={loading}
			data-testid="academic-setup-ready"
		>
			{#if mode === 'year'}
				<AcademicYearTermEditor
					{years}
					{termsByYear}
					{bellSchedules}
					{canManage}
					{contextSettings}
					onUpdateContextSettings={saveContextSettings}
					onReloadContextSettings={reloadContextSettings}
					{busy}
					onCreateYear={addYear}
					onUpdateYear={editYear}
					onCreateTerm={addTerm}
					onUpdateTerm={editTerm}
				/>
			{:else}
				<TimeSetupEditor
					{years}
					{bellSchedules}
					{termsByYear}
					setupYearIds={bellScheduleSetupYearIds}
					setupTermIds={bellScheduleSetupTermIds}
					{canManage}
					{busy}
					onCreateBellSchedule={addBellSchedule}
					onCloneBellSchedule={copyBellSchedule}
					onUpdateTerm={editTerm}
					onUpdateBellSchedule={editBellSchedule}
					onLoadBellSchedulePeriods={loadBellSchedulePeriods}
					onReplaceBellSchedulePeriods={saveBellSchedulePeriods}
				/>
			{/if}
			{#if errorMessage}<div
					role="alert"
					class="flex flex-wrap items-center gap-2 text-sm text-destructive"
				>
					<p>{errorMessage}</p>
					<Button size="sm" variant="outline" onclick={loadWorkspace}>ลองอีกครั้ง</Button>
				</div>{/if}
		</section>
	{/if}
</div>
