<script lang="ts">
	import type { PageProps } from './$types';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { onDestroy, untrack } from 'svelte';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { captureRouteLoad } from '#lib/navigation/route-load.js';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { authStore } from '#lib/stores/auth.js';
	import { can } from '#lib/stores/permissions.js';
	import {
		listChildAcademicContextOptions,
		type AcademicContextOptionsResponse
	} from '#lib/api/academic-context.js';
	import { resolveScopedAcademicYearUrl } from '#lib/academic-context/scoped-year.js';
	import { getChildProfile } from '#lib/api/parents.js';
	import type { Student } from '#lib/api/students.js';
	import ScopedAcademicYearSelect from '#lib/components/academic-context/ScopedAcademicYearSelect.svelte';
	import StudentSummary from '#lib/components/student/StudentSummary.svelte';
	import StudentDestinations from '#lib/components/student/StudentDestinations.svelte';
	import { PERMISSIONS } from '#lib/permissions/registry.js';

	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Calendar, BookOpen, UserCheck, ClipboardList } from '@lucide/svelte';

	let { data }: PageProps = $props();
	const studentId = $derived(data.studentId);
	const identityKey = $derived.by(() => {
		void $authStore;
		void $can;
		return appIdentityKey();
	});
	const ownerKey = $derived(`${identityKey}|${data.requestKey}`);
	const allowed = $derived($authStore.user?.user_type === 'parent');
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
			goto(resolve(`parent/student/${page.params.id}`) + url.search, {
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
			getChildProfile(studentId, yearId, { signal: t.signal }).then((student) => ({
				ownerKey: key,
				student
			})),
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
			listChildAcademicContextOptions(studentId, t.signal).then((options) => {
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

	async function changeAcademicYear(yearId: string) {
		if (!contextOptions?.years.some((y) => y.id === yearId) || yearId === selectedYearId) return;
		await goto(
			resolve(
				`parent/student/${encodeURIComponent(studentId)}?academicYearId=${encodeURIComponent(yearId)}`
			),
			{
				reset: false
			}
		);
	}

	const academicYearQuery = $derived(
		selectedYearId ? `?academicYearId=${encodeURIComponent(selectedYearId)}` : ''
	);

	const parentHref = $derived(`${resolve('parent')}${academicYearQuery}`);
	const termId = $derived(
		contextOptions?.terms.find(
			(t) => t.academicYearId === selectedYearId && t.id === contextOptions?.defaultAcademicTermId
		)?.id ??
			contextOptions?.terms.find((t) => t.academicYearId === selectedYearId)?.id ??
			''
	);
	const timetableHref = $derived(
		`${resolve(`parent/student/${studentId}/timetable`)}${academicYearQuery}${termId ? '&academicTermId=' + encodeURIComponent(termId) : ''}`
	);
	const destinations = $derived([
		{
			title: 'ตารางเรียน',
			description: 'รายวิชาและกิจกรรมของนักเรียนในภาคเรียนที่เลือก',
			href: timetableHref,
			icon: BookOpen
		},
		...($can.has(PERMISSIONS.ATTENDANCE_READ_OWN)
			? [
					{
						title: 'การเช็คชื่อ',
						description: 'ดูสรุปการเข้าเรียน ประวัติรายวัน และหลักฐานที่มีสิทธิ์เข้าถึง',
						href: `${resolve(`parent/student/${studentId}/attendance`)}${academicYearQuery}${termId ? '&academicTermId=' + encodeURIComponent(termId) : ''}`,
						icon: UserCheck
					}
				]
			: []),
		{
			title: 'ปฏิทิน',
			description: 'กิจกรรมและกำหนดการที่เกี่ยวข้องกับนักเรียนคนนี้',
			href: `${resolve(`parent/student/${studentId}/calendar`)}${academicYearQuery}`,
			icon: Calendar
		},
		{
			title: 'ตารางสอบ',
			description: 'กำหนดการสอบและสถานที่สอบที่โรงเรียนเผยแพร่',
			href: `${resolve(`parent/student/${studentId}/exams`)}${academicYearQuery}${termId ? '&academicTermId=' + encodeURIComponent(termId) : ''}`,
			icon: ClipboardList
		}
	]);
</script>

<PageShell
	title={student
		? `${student.title || ''}${student.first_name} ${student.last_name}`
		: 'ข้อมูลนักเรียน'}
	description={student
		? `${student.grade_level || 'ไม่ระบุชั้น'} | ห้อง ${student.homeroom || '-'} | รหัสนักเรียน: ${student.student_id || '-'}`
		: 'ข้อมูลนักเรียนที่เชื่อมโยงกับบัญชีผู้ปกครอง'}
	backHref={parentHref}
>
	{#if contextOptions && contextOptions.years.length > 0}
		<div class="flex max-w-sm flex-col gap-2 rounded-xl border bg-card p-4">
			<Label for="parent-child-detail-year">ปีการศึกษา</Label>
			<ScopedAcademicYearSelect
				id="parent-child-detail-year"
				years={contextOptions.years}
				value={selectedYearId}
				disabled={contextLoading}
				onchange={changeAcademicYear}
			/>
		</div>
	{/if}

	{#if contextError}<PageState
			variant="error"
			title="โหลดประวัติปีการศึกษาไม่สำเร็จ"
			description={contextError}
			actionLabel="ลองบริบทอีกครั้ง"
			onaction={retryContext}
		/>{/if}
	{#if error}<PageState
			variant="error"
			title="โหลดข้อมูลไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={loadProfile}
		/>{/if}
	<div data-testid="parent-profile-region" aria-busy={loading || contextLoading}>
		{#if contextLoading || (loading && !loaded)}
			<div role="status" aria-label="กำลังโหลดข้อมูลนักเรียน">
				<PageSkeleton variant="detail" rows={3} />
			</div>
		{:else if contextOptions && !contextError && contextOptions.years.length === 0}
			<PageState
				title="ยังไม่มีประวัติปีการศึกษาสำหรับนักเรียนคนนี้"
				description="กรุณาติดต่อโรงเรียนเพื่อตรวจสอบการลงทะเบียน"
			/>
		{:else if student}
			<StudentSummary
				{student}
				yearName={contextOptions?.years.find((year) => year.id === selectedYearId)?.name}
			/>
			<div class="mt-6"><StudentDestinations items={destinations} /></div>
		{:else if loaded && !contextError && !error}
			<PageState
				title="ไม่พบข้อมูลนักเรียน"
				description="ไม่พบข้อมูลนักเรียนที่เชื่อมโยงกับบัญชีผู้ปกครองนี้"
				actionLabel="กลับหน้าผู้ปกครอง"
				href={parentHref}
			/>
		{/if}
	</div>
</PageShell>
