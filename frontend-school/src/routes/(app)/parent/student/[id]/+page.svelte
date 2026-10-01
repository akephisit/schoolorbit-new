<script lang="ts">
	import type { PageProps } from './$types';
	import { goto, replaceState } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { onDestroy, untrack } from 'svelte';
	import { LatestRequest } from '$lib/async/latest-request';
	import { captureRouteLoad } from '$lib/navigation/route-load';
	import { appIdentityKey } from '$lib/auth/settled-user';
	import { authStore } from '$lib/stores/auth';
	import { can } from '$lib/stores/permissions';
	import {
		listChildAcademicContextOptions,
		type AcademicContextOptionsResponse
	} from '$lib/api/academic-context';
	import { resolveScopedAcademicYearUrl } from '$lib/academic-context/scoped-year';
	import { getChildProfile } from '$lib/api/parents';
	import type { Student } from '$lib/api/students';
	import ScopedAcademicYearSelect from '$lib/components/academic-context/ScopedAcademicYearSelect.svelte';
	import { Card } from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import { Label } from '$lib/components/ui/label';
	import { User, Calendar, BookOpen, Clock } from '@lucide/svelte';
	import { formatDate } from '$lib/utils/date';
	import PrivateFileImage from '$lib/components/files/PrivateFileImage.svelte';

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
			replaceState(resolve(`${url.pathname}${url.search}` as '/parent/student/[id]'), page.state);
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
				`/parent/student/${encodeURIComponent(studentId)}?academicYearId=${encodeURIComponent(yearId)}` as '/parent/student/[id]'
			),
			{
				noScroll: true,
				keepFocus: true
			}
		);
	}

	const academicYearQuery = $derived(
		selectedYearId ? `?academicYearId=${encodeURIComponent(selectedYearId)}` : ''
	);
	const parentHref = $derived(`${resolve('/parent')}${academicYearQuery}`);
	const termId = $derived(
		contextOptions?.terms.find(
			(t) => t.academicYearId === selectedYearId && t.id === contextOptions?.activeAcademicTermId
		)?.id ??
			contextOptions?.terms.find((t) => t.academicYearId === selectedYearId)?.id ??
			''
	);
	const timetableHref = $derived(
		`${resolve(`/parent/student/${studentId}/timetable`)}${academicYearQuery}${termId ? '&academicTermId=' + encodeURIComponent(termId) : ''}`
	);
</script>

<PageShell
	title={student
		? `${student.title || ''}${student.first_name} ${student.last_name}`
		: 'ข้อมูลนักเรียน'}
	description={student
		? `${student.grade_level || 'ไม่ระบุชั้น'} | ห้อง ${student.homeroom || '-'} | รหัสนักเรียน: ${student.student_number || '-'}`
		: 'ข้อมูลนักเรียนที่เชื่อมโยงกับบัญชีผู้ปกครอง'}
	backHref={parentHref}
>
	<Button
		variant="outline"
		disabled={loading || contextLoading || !selectedYearId}
		onclick={loadProfile}>โหลดข้อมูลใหม่</Button
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
		{#if loading && loaded}<p
				role="status"
				aria-label="กำลังอัปเดตข้อมูล"
				class="text-muted-foreground text-sm"
			>
				กำลังอัปเดตข้อมูล…
			</p>{/if}
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
			<!-- Header -->
			<div class="flex flex-col md:flex-row gap-6 items-start">
				<div
					class="w-32 h-32 rounded-full bg-muted flex items-center justify-center overflow-hidden border-4 border-background shadow-lg"
				>
					{#if student.profile_image_file_id}
						<PrivateFileImage
							fileId={student.profile_image_file_id}
							resourceId={student.id}
							alt={student.first_name}
							class="w-full h-full object-cover"
						/>
					{:else}
						<User class="w-12 h-12 text-muted-foreground/50" />
					{/if}
				</div>

				<div class="flex-1">
					<div class="flex flex-wrap gap-2 mb-4">
						<Badge variant="secondary" class="text-sm px-3 py-1">
							{student.grade_level || 'ไม่ระบุชั้น'}
						</Badge>
						<Badge variant="outline" class="text-sm px-3 py-1 text-muted-foreground">
							ห้อง {student.homeroom || '-'}
						</Badge>
						<Badge variant="outline" class="text-sm px-3 py-1 text-muted-foreground">
							รหัสนักเรียน: {student.student_number || '-'}
						</Badge>
					</div>

					<div class="grid grid-cols-1 md:grid-cols-2 gap-4 text-sm text-muted-foreground">
						<div class="flex items-center gap-2">
							<Calendar class="w-4 h-4" />
							วันเกิด: {student.date_of_birth ? formatDate(student.date_of_birth) : '-'}
						</div>
					</div>
				</div>
			</div>

			<!-- Content Tabs -->
			<!-- Placeholder for future features like Grades, Timetable, Attendance -->
			<div class="grid grid-cols-1 md:grid-cols-3 gap-6 mt-8">
				<Card class="p-6">
					<div class="flex items-center gap-3 mb-4">
						<div
							class="p-2 rounded-lg bg-blue-100 text-blue-600 dark:bg-blue-900/30 dark:text-blue-400"
						>
							<Clock class="w-5 h-5" />
						</div>
						<h3 class="font-semibold">การเข้าเรียน</h3>
					</div>
					<p class="text-muted-foreground text-sm">ยังไม่มีข้อมูลการเข้าเรียน</p>
					<Button variant="link" class="px-0 mt-2 text-blue-600">ดูทั้งหมด</Button>
				</Card>

				<Card class="p-6">
					<div class="flex items-center gap-3 mb-4">
						<div
							class="p-2 rounded-lg bg-green-100 text-green-600 dark:bg-green-900/30 dark:text-green-400"
						>
							<BookOpen class="w-5 h-5" />
						</div>
						<h3 class="font-semibold">ผลการเรียน</h3>
					</div>
					<p class="text-muted-foreground text-sm">ยังไม่มีข้อมูลผลการเรียน</p>
					<Button variant="link" class="px-0 mt-2 text-green-600">ดูทั้งหมด</Button>
				</Card>

				<Card class="p-6">
					<div class="flex items-center gap-3 mb-4">
						<div
							class="p-2 rounded-lg bg-purple-100 text-purple-600 dark:bg-purple-900/30 dark:text-purple-400"
						>
							<Calendar class="w-5 h-5" />
						</div>
						<h3 class="font-semibold">ตารางเรียน</h3>
					</div>
					<p class="text-muted-foreground text-sm">ดูตารางเรียนของบุตรในแต่ละภาคเรียน</p>
					<Button
						variant="link"
						class="px-0 mt-2 text-purple-600"
						href={timetableHref}
						data-sveltekit-preload-data="tap"
					>
						ดูทั้งหมด
					</Button>
				</Card>
			</div>
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
