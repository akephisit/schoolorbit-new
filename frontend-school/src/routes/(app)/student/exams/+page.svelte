<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import type { PageProps } from './$types';
	import { onDestroy, untrack } from 'svelte';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { captureRouteLoad } from '#lib/navigation/route-load.js';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { authStore } from '#lib/stores/auth.js';
	import { can } from '#lib/stores/permissions.js';
	import { resolveScopedAcademicContextUrl } from '#lib/academic-context/scoped-year.js';
	import {
		listMyAcademicContextOptions,
		type AcademicContextOptionsResponse
	} from '#lib/api/academic-context.js';
	import { listMyExamSchedules, type PersonalExamScheduleRound } from '#lib/api/examSchedule.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import PersonalExamScheduleView from '#lib/components/academic/exam-schedule/PersonalExamScheduleView.svelte';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Select from '#lib/components/ui/select/index.js';

	let { data }: PageProps = $props();
	const identityKey = $derived.by(() => {
		void $authStore;
		void $can;
		return appIdentityKey();
	});
	const ownerKey = $derived(`${identityKey}|${data.requestKey}`);
	const allowed = $derived($authStore.user?.user_type === 'student');
	let contextOptions = $state.raw<AcademicContextOptionsResponse | null>(null);
	let selectedYearId = $state(''),
		selectedTermId = $state('');
	let rounds = $state.raw<PersonalExamScheduleRound[]>([]);
	let loading = $state(true),
		loaded = $state(false),
		error = $state(''),
		contextLoading = $state(true),
		contextError = $state('');
	let disposed = false,
		owner = '';
	const contextRequest = new LatestRequest(),
		primaryRequest = new LatestRequest();
	let consumedContext: typeof data.context | null = null,
		consumedRecords: typeof data.records | null = null;

	const termOptions = $derived(
		contextOptions?.terms.filter((term) => term.academicYearId === selectedYearId) ?? []
	);

	$effect.pre(() => {
		const key = ownerKey,
			a = data.context,
			b = data.records,
			canRead = allowed;
		untrack(() => {
			if (owner !== key || !canRead) {
				owner = key;

				contextRequest.abort();
				primaryRequest.abort();
				contextOptions = null;
				selectedYearId = '';
				selectedTermId = '';
				rounds = [];
				loaded = false;
				loading = canRead;
				contextLoading = canRead;
				error = '';
				contextError = '';
			}
			if (!canRead) return;

			if (a !== consumedContext) {
				consumedContext = a;
				const t = contextRequest.begin();
				contextLoading = true;
				void a.then((v) => applyContext(v, t.revision, key));
			}
			if (b !== consumedRecords) {
				consumedRecords = b;
				const t = primaryRequest.begin();
				loading = true;
				void b.then((v) => applyRecords(v, t.revision, key));
			}
		});
	});
	onDestroy(() => {
		disposed = true;

		contextRequest.abort();
		primaryRequest.abort();
	});
	function current(key: string) {
		return !disposed && allowed && key === ownerKey;
	}
	function applyContext(v: Awaited<typeof data.context>, revision: number, key: string) {
		if (!current(key) || !contextRequest.isCurrent(revision)) return;
		contextLoading = false;
		if (!v.ok) {
			contextError = v.error;
			return;
		}
		if (v.data.ownerKey !== key) return;
		contextError = '';
		contextOptions = v.data.options;
		selectedYearId = v.data.academicYearId;
		selectedTermId = v.data.academicTermId;
		if (v.data.replaceHref) {
			const url = new URL(v.data.replaceHref);
			goto(resolve('student/exams') + url.search, {
				shallow: true,
				replace: true,
				state: page.state
			});
		}
	}
	function applyRecords(v: Awaited<typeof data.records>, revision: number, key: string) {
		if (!current(key) || !primaryRequest.isCurrent(revision)) return;
		loading = false;
		if (!v.ok) {
			error = v.error;
			return;
		}
		if (v.data.ownerKey !== key) return;
		rounds = v.data.records;
		loaded = contextOptions !== null;
		error = '';
	}
	async function loadPrimary() {
		if (!allowed || disposed || !selectedYearId || !selectedTermId) return;
		const key = ownerKey,
			t = primaryRequest.begin();
		loading = true;
		error = '';
		const v = await captureRouteLoad(
			listMyExamSchedules(selectedTermId, { signal: t.signal }).then((records) => ({
				ownerKey: key,
				records
			})),
			'โหลดตารางสอบไม่สำเร็จ'
		);
		applyRecords(v, t.revision, key);
	}
	async function retryContext() {
		if (!allowed || disposed) return;
		const key = ownerKey,
			t = contextRequest.begin();
		contextLoading = true;
		contextError = '';
		const v = await captureRouteLoad(
			listMyAcademicContextOptions(t.signal).then((options) => {
				const selection = resolveScopedAcademicContextUrl(options, new URL(data.requestHref), true);
				return {
					ownerKey: key,
					options,
					academicYearId: selection.academicYearId,
					academicTermId: selection.academicTermId,
					replaceHref: selection.replaceUrl?.href ?? null
				};
			}),
			'โหลดประวัติปีและภาคเรียนไม่สำเร็จ'
		);
		if (!current(key) || !contextRequest.isCurrent(t.revision)) return;
		applyContext(v, t.revision, key);
		if (v.ok && selectedYearId && selectedTermId) await loadPrimary();
	}
	async function updateUrl(yearId: string, termId: string) {
		const url = new URL(data.requestHref);
		url.searchParams.set('academicYearId', yearId);

		if (termId) url.searchParams.set('academicTermId', termId);
		else url.searchParams.delete('academicTermId');

		await goto(resolve('student/exams') + url.search, {
			reset: false
		});
	}
	async function changeYear(yearId: string) {
		if (!contextOptions?.years.some((year) => year.id === yearId) || yearId === selectedYearId)
			return;
		const terms = contextOptions.terms.filter((term) => term.academicYearId === yearId);
		const next =
			terms.find((term) => term.id === contextOptions?.activeAcademicTermId)?.id ??
			terms[0]?.id ??
			'';
		await updateUrl(yearId, next);
	}
	async function changeTerm(value: string) {
		const termId = value;
		if (termId && !termOptions.some((term) => term.id === termId)) return;
		if (termId === selectedTermId) return;
		await updateUrl(selectedYearId, termId);
	}
</script>

<PageShell title={data.title} description="ตารางสอบที่ประกาศแล้วสำหรับฉัน">
	<Button variant="outline" disabled={loading || contextLoading} onclick={loadPrimary}
		>โหลดข้อมูลใหม่</Button
	>

	<div class="flex flex-wrap gap-3 rounded-xl border bg-card p-4">
		<div class="min-w-52 space-y-2">
			<Label for="student-exam-year">ปีการศึกษา</Label>
			<Select.Root
				type="single"
				value={selectedYearId}
				disabled={contextLoading}
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
				disabled={contextLoading || termOptions.length === 0}
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

	{#if contextError}<PageState
			variant="error"
			title="โหลดประวัติปีและภาคเรียนไม่สำเร็จ"
			description={contextError}
			actionLabel="ลองบริบทอีกครั้ง"
			onaction={retryContext}
		/>{/if}
	{#if error}
		<PageState
			variant="error"
			title="โหลดตารางสอบไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={loadPrimary}
		/>
	{/if}
	<div data-testid="student-exams-region" aria-busy={loading || contextLoading}>
		{#if loading && loaded}
			<p role="status" aria-label="กำลังอัปเดตข้อมูล" class="text-muted-foreground text-sm">
				กำลังอัปเดตข้อมูล…
			</p>
		{/if}
		{#if contextLoading || (loading && !loaded)}
			<div role="status" aria-label="กำลังโหลดตารางสอบ">
				<PageSkeleton variant="table" rows={6} columns={7} />
			</div>
		{:else if contextOptions && contextOptions.years.length === 0 && !contextError}
			<PageState
				title="ยังไม่มีประวัติปีการศึกษา"
				description="เมื่อโรงเรียนสร้างข้อมูลนักเรียนประจำปีแล้ว ประวัติจะปรากฏที่นี่"
			/>
		{:else if loaded && !contextError}
			<PersonalExamScheduleView {rounds} />
		{/if}
	</div>
</PageShell>
