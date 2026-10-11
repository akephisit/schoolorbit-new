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
	import {
		listMyAcademicContextOptions,
		type AcademicContextOptionsResponse
	} from '#lib/api/academic-context.js';
	import StudentSummary from '#lib/components/student/StudentSummary.svelte';
	import StudentDestinations from '#lib/components/student/StudentDestinations.svelte';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import ScopedAcademicYearSelect from '#lib/components/academic-context/ScopedAcademicYearSelect.svelte';
	import { resolveScopedAcademicYearUrl } from '#lib/academic-context/scoped-year.js';
	import { User, Calendar, BookOpen, UserCheck } from '@lucide/svelte';
	import { getOwnProfile, type Student } from '#lib/api/students.js';

	let { data }: PageProps = $props();
	const identityKey = $derived.by(() => {
		void $authStore;
		void $can;
		return appIdentityKey();
	});
	const ownerKey = $derived(`${identityKey}|${data.requestKey}`);
	const allowed = $derived($authStore.user?.user_type === 'student');
	let contextOptions = $state.raw<AcademicContextOptionsResponse | null>(null);
	let selectedYearId = $state('');
	let student = $state.raw<Student | null>(null);
	let loading = $state(true),
		loaded = $state(false),
		error = $state('');
	let contextLoading = $state(true),
		contextError = $state('');
	let disposed = false,
		owner = '';
	const contextRequest = new LatestRequest(),
		profileRequest = new LatestRequest();
	let consumedContext: typeof data.context | null = null,
		consumedProfile: typeof data.profile | null = null;

	const academicYearQuery = $derived(
		selectedYearId ? `?academicYearId=${encodeURIComponent(selectedYearId)}` : ''
	);
	const profileHref = $derived(`${resolve('student/profile')}${academicYearQuery}`);
	const termId = $derived(
		contextOptions?.terms.find(
			(term) =>
				term.academicYearId === selectedYearId && term.id === contextOptions.activeAcademicTermId
		)?.id ??
			contextOptions?.terms.find((term) => term.academicYearId === selectedYearId)?.id ??
			''
	);
	const termQuery = $derived(
		`${academicYearQuery}${termId ? '&academicTermId=' + encodeURIComponent(termId) : ''}`
	);
	const destinations = $derived([
		{
			title: 'ตารางเรียน',
			description: 'รายวิชาและกิจกรรมในภาคเรียนที่เลือก',
			href: `${resolve('student/timetable')}${termQuery}`,
			icon: BookOpen
		},
		...($can.has(PERMISSIONS.ATTENDANCE_READ_OWN)
			? [
					{
						title: 'การเช็คชื่อ',
						description: 'ดูสรุปการเข้าเรียนและประวัติรายวันของฉัน',
						href: `${resolve('student/attendance')}${termQuery}`,
						icon: UserCheck
					}
				]
			: []),
		{
			title: 'ปฏิทิน',
			description: 'กิจกรรมและกำหนดการที่เกี่ยวข้องกับฉัน',
			href: `${resolve('student/calendar')}${academicYearQuery}`,
			icon: Calendar
		},
		{
			title: 'ข้อมูลส่วนตัว',
			description: 'ดูข้อมูลส่วนตัวและข้อมูลติดต่อ',
			href: profileHref,
			icon: User
		}
	]);
	$effect.pre(() => {
		const key = ownerKey,
			a = data.context,
			b = data.profile,
			canRead = allowed;
		untrack(() => {
			if (owner !== key || !canRead) {
				owner = key;

				contextRequest.abort();
				profileRequest.abort();
				contextOptions = null;
				selectedYearId = '';
				student = null;
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
			if (b !== consumedProfile) {
				consumedProfile = b;
				const t = profileRequest.begin();
				loading = true;
				void b.then((v) => applyProfile(v, t.revision, key));
			}
		});
	});
	onDestroy(() => {
		disposed = true;

		contextRequest.abort();
		profileRequest.abort();
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
		if (v.data.replaceHref) {
			const url = new URL(v.data.replaceHref);
			goto(resolve('student') + url.search, {
				shallow: true,
				replace: true,
				state: page.state
			});
		}
	}
	function applyProfile(v: Awaited<typeof data.profile>, revision: number, key: string) {
		if (!current(key) || !profileRequest.isCurrent(revision)) return;
		loading = false;
		if (!v.ok) {
			error = v.error;
			return;
		}
		if (v.data.ownerKey !== key) return;
		student = v.data.student;
		loaded = v.data.student !== null;
		error = '';
	}
	async function loadProfile() {
		if (!allowed || disposed || !selectedYearId) return;
		const key = ownerKey,
			yearId = selectedYearId,
			t = profileRequest.begin();
		loading = true;
		error = '';
		const v = await captureRouteLoad(
			getOwnProfile(yearId, { signal: t.signal }).then((student) => ({ ownerKey: key, student })),
			'โหลดข้อมูลนักเรียนไม่สำเร็จ'
		);
		applyProfile(v, t.revision, key);
	}
	async function retryContext() {
		const key = ownerKey,
			t = contextRequest.begin();
		contextLoading = true;
		contextError = '';
		const v = await captureRouteLoad(
			listMyAcademicContextOptions(t.signal).then((options) => {
				const selection = resolveScopedAcademicYearUrl(options, new URL(data.requestHref));
				return {
					ownerKey: key,
					options,
					academicYearId: selection.academicYearId ?? '',
					replaceHref: selection.replaceUrl?.href ?? null
				};
			}),
			'โหลดประวัติปีการศึกษาไม่สำเร็จ'
		);
		if (!current(key) || !contextRequest.isCurrent(t.revision)) return;
		applyContext(v, t.revision, key);
		if (v.ok && selectedYearId) await loadProfile();
	}
	function retry() {
		if (selectedYearId && contextOptions) void loadProfile();
		else void retryContext();
	}
	async function changeAcademicYear(yearId: string) {
		if (!contextOptions?.years.some((y) => y.id === yearId) || yearId === selectedYearId) return;
		await goto(resolve(`student?academicYearId=${encodeURIComponent(yearId)}`), {
			reset: false
		});
	}
</script>

<PageShell
	title="แดชบอร์ด"
	description={student
		? `สวัสดี, ${student.first_name} ${student.last_name}`
		: 'ภาพรวมข้อมูลนักเรียน'}
>
	{#snippet actions()}<Button
			variant="outline"
			disabled={loading || contextLoading || !selectedYearId}
			onclick={loadProfile}>โหลดข้อมูลใหม่</Button
		>{/snippet}
	{#if contextOptions && contextOptions.years.length > 0}
		<div class="flex max-w-sm flex-col gap-2 rounded-xl border bg-card p-4">
			<Label for="student-dashboard-year">ปีการศึกษา</Label>
			<ScopedAcademicYearSelect
				id="student-dashboard-year"
				years={contextOptions.years}
				value={selectedYearId}
				disabled={contextLoading}
				onchange={changeAcademicYear}
			/>
		</div>
	{/if}

	{#if contextError}
		<PageState
			variant="error"
			title="โหลดประวัติปีการศึกษาไม่สำเร็จ"
			description={contextError}
			actionLabel="ลองบริบทอีกครั้ง"
			onaction={retryContext}
		/>
	{:else if error}
		<PageState
			variant="error"
			title="โหลดข้อมูลนักเรียนไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={retry}
		/>
	{/if}
	<div aria-busy={loading || contextLoading} data-testid="student-profile-region">
		{#if contextLoading || (loading && !loaded)}
			<div role="status" aria-label="กำลังโหลดข้อมูลนักเรียน">
				<PageSkeleton variant="cards" rows={3} />
			</div>
		{:else if contextOptions && contextOptions.years.length === 0 && !contextError}
			<PageState
				title="ยังไม่มีประวัติปีการศึกษาสำหรับบัญชีนี้"
				description="กรุณาติดต่อผู้ดูแลระบบเพื่อตรวจสอบการลงทะเบียนนักเรียน"
			/>
		{:else if student}
			<StudentSummary
				{student}
				yearName={contextOptions?.years.find((year) => year.id === selectedYearId)?.name}
			/>
			<div class="mt-6"><StudentDestinations items={destinations} /></div>
		{:else}
			<PageState
				title="ไม่พบข้อมูลนักเรียน"
				description="ไม่พบโปรไฟล์นักเรียนของบัญชีนี้ กรุณาติดต่อผู้ดูแลระบบ"
			/>
		{/if}
	</div>
</PageShell>
