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
		listChildAcademicContextOptions,
		type AcademicContextOptionsResponse
	} from '#lib/api/academic-context.js';
	import { type TimetableBlock } from '#lib/api/timetable.js';
	import { getChildTimetable, getChildProfile } from '#lib/api/parents.js';
	import type { Student } from '#lib/api/students.js';

	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import PersonalTimetable from '#lib/components/student/PersonalTimetable.svelte';
	import { Badge } from '#lib/components/ui/badge/index.js';

	let { data }: PageProps = $props();
	const studentId = $derived(data.studentId);

	const identityKey = $derived.by(() => {
		void $authStore;
		void $can;
		return appIdentityKey();
	});
	const ownerKey = $derived(`${identityKey}|${data.requestKey}`);
	const allowed = $derived($authStore.user?.user_type === 'parent');
	let child = $state.raw<Student | null>(null);
	let childLoading = $state(true),
		childError = $state('');
	const childRequest = new LatestRequest();
	let consumedChild: typeof data.profile | null = null;
	let contextOptions = $state.raw<AcademicContextOptionsResponse | null>(null);
	let selectedYearId = $state(''),
		selectedTermId = $state('');
	const childDetailHref = $derived(
		`${resolve(`parent/student/${encodeURIComponent(studentId)}`)}?academicYearId=${encodeURIComponent(selectedYearId)}`
	);
	let blocks = $state.raw<TimetableBlock[]>([]);
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
			c = data.profile,
			canRead = allowed;
		untrack(() => {
			if (owner !== key || !canRead) {
				owner = key;

				contextRequest.abort();
				primaryRequest.abort();
				contextOptions = null;
				selectedYearId = '';
				selectedTermId = '';
				blocks = [];
				childRequest.abort();
				child = null;
				childLoading = canRead;
				childError = '';
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
			if (c !== consumedChild) {
				consumedChild = c;
				const t = childRequest.begin();
				childLoading = true;
				void c.then((v) => applyChild(v, t.revision, key));
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
		childRequest.abort();

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

			goto(resolve(`parent/student/${page.params.id}/timetable`) + url.search, {
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
		blocks = v.data.records;
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
			getChildTimetable(studentId, selectedTermId, data.date, { signal: t.signal }).then(
				(records) => ({ ownerKey: key, records })
			),
			'โหลดตารางเรียนไม่สำเร็จ'
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
			listChildAcademicContextOptions(studentId, t.signal).then((options) => {
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

		if (v.ok && selectedYearId)
			await Promise.all([loadChild(), ...(selectedTermId ? [loadPrimary()] : [])]);
	}
	function applyChild(v: Awaited<typeof data.profile>, revision: number, key: string) {
		if (!current(key) || !childRequest.isCurrent(revision)) return;
		childLoading = false;
		if (!v.ok) {
			childError = v.error;
			return;
		}
		if (v.data.ownerKey !== key) return;
		child = v.data.student;
		childError = '';
	}
	async function loadChild() {
		if (!allowed || disposed || !selectedYearId) return;
		const key = ownerKey,
			t = childRequest.begin();
		childLoading = true;
		childError = '';
		const v = await captureRouteLoad(
			getChildProfile(studentId, selectedYearId, { signal: t.signal }).then((student) => ({
				ownerKey: key,
				student
			})),
			'โหลดข้อมูลนักเรียนไม่สำเร็จ'
		);
		applyChild(v, t.revision, key);
	}
	async function updateUrl(yearId: string, termId: string) {
		const url = new URL(data.requestHref);
		url.searchParams.set('academicYearId', yearId);

		if (termId) url.searchParams.set('academicTermId', termId);
		else url.searchParams.delete('academicTermId');

		await goto(resolve(`parent/student/${page.params.id}/timetable`) + url.search, {
			reset: false
		});
	}
	async function changeYear(yearId: string) {
		if (!contextOptions?.years.some((year) => year.id === yearId) || yearId === selectedYearId)
			return;
		const terms = contextOptions.terms.filter((term) => term.academicYearId === yearId);
		const next = terms.find((term) => term.id === contextOptions?.defaultAcademicTermId)?.id ?? '';
		await updateUrl(yearId, next);
	}
	async function changeTerm(value: string) {
		const termId = value;
		if (termId && !termOptions.some((term) => term.id === termId)) return;
		if (termId === selectedTermId) return;
		await updateUrl(selectedYearId, termId);
	}
</script>

<PageShell
	backHref={childDetailHref}
	title={child ? `ตารางเรียนของ ${child.first_name}` : 'ตารางเรียน'}
	description={child
		? `ตารางเรียนของ ${child.first_name} ${child.last_name}`
		: 'รายวิชาและกิจกรรมของนักเรียนในแต่ละภาคเรียน'}
>
	<div data-testid="parent-child-profile-region" aria-busy={childLoading || contextLoading}>
		{#if childError}<PageState
				variant="error"
				title="โหลดข้อมูลนักเรียนไม่สำเร็จ"
				description={childError}
				actionLabel="ลองข้อมูลนักเรียนอีกครั้ง"
				onaction={loadChild}
			/>{/if}

		{#if (childLoading || contextLoading) && !child && !contextError}<div
				role="status"
				aria-label="กำลังโหลดข้อมูลนักเรียน"
			>
				<PageSkeleton variant="detail" />
			</div>
		{:else if child}<div
				class="space-y-3 rounded-xl border border-primary/20 bg-linear-to-br from-primary/10 to-card p-5"
			>
				<p class="break-words font-semibold">
					{child.title ?? ''}{child.first_name}
					{child.last_name}
				</p>
				<div class="flex flex-wrap gap-2">
					<Badge variant="secondary">รหัสนักเรียน {child.student_id || 'ไม่ระบุ'}</Badge><Badge
						variant="outline"
						>{child.grade_level || 'ไม่ระบุชั้น'} · ห้อง {child.homeroom || 'ไม่ระบุ'}</Badge
					>
				</div>
			</div>{/if}
	</div>

	<div class="grid gap-4 rounded-xl border bg-card p-4 sm:grid-cols-2">
		<div class="min-w-0 space-y-2">
			<Label for="student-year">ปีการศึกษา</Label>
			<Select.Root
				type="single"
				value={selectedYearId}
				disabled={contextLoading}
				onValueChange={(value) => void changeYear(value)}
			>
				<Select.Trigger id="student-year" class="w-full">
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
		<div class="min-w-0 space-y-2">
			<Label for="student-term">ภาคเรียน</Label>
			<Select.Root
				type="single"
				value={selectedTermId}
				disabled={contextLoading || termOptions.length === 0}
				onValueChange={(value) => void changeTerm(value)}
			>
				<Select.Trigger id="student-term" class="w-full">
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
			title="โหลดตารางเรียนไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={loadPrimary}
		/>
	{/if}
	<div data-testid="parent-timetable-region" aria-busy={loading || contextLoading}>
		{#if contextLoading || (loading && !loaded)}
			<div role="status" aria-label="กำลังโหลดตารางเรียน">
				<PageSkeleton variant="table" rows={6} columns={4} />
			</div>
		{:else if contextOptions && contextOptions.years.length === 0 && !contextError}
			<PageState
				title="ยังไม่มีประวัติปีการศึกษา"
				description="เมื่อโรงเรียนสร้างข้อมูลนักเรียนประจำปีแล้ว ประวัติจะปรากฏที่นี่"
			/>
		{:else if loaded && blocks.length === 0}
			<PageState
				title="ยังไม่มีตารางเรียน"
				description="โรงเรียนยังไม่ได้จัดตารางเรียนในภาคเรียนที่เลือก"
			/>
		{:else if loaded}
			<PersonalTimetable {blocks} />
		{/if}
	</div>
</PageShell>
