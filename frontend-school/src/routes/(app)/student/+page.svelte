<script lang="ts">
	import { goto, replaceState } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import type { PageProps } from './$types';
	import { onDestroy, untrack } from 'svelte';
	import { LatestRequest } from '$lib/async/latest-request';
	import { captureRouteLoad } from '$lib/navigation/route-load';
	import { appIdentityKey } from '$lib/auth/settled-user';
	import { authStore } from '$lib/stores/auth';
	import { can } from '$lib/stores/permissions';
	import {
		listMyAcademicContextOptions,
		type AcademicContextOptionsResponse
	} from '$lib/api/academic-context';
	import { Card } from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { Label } from '$lib/components/ui/label';
	import ScopedAcademicYearSelect from '$lib/components/academic-context/ScopedAcademicYearSelect.svelte';
	import { resolveScopedAcademicYearUrl } from '$lib/academic-context/scoped-year';
	import { User, Calendar, BookOpen, Award } from '@lucide/svelte';
	import { getOwnProfile, type Student } from '$lib/api/students';

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
	const profileHref = $derived(`${resolve('/student/profile')}${academicYearQuery}`);
	const timetableHref = $derived(`${resolve('/student/timetable')}${academicYearQuery}`);
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
			replaceState(resolve(`${url.pathname}${url.search}` as '/student'), page.state);
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
		await goto(resolve(`/student?academicYearId=${encodeURIComponent(yearId)}` as '/student'), {
			noScroll: true,
			keepFocus: true
		});
	}
</script>

<PageShell
	title="แดชบอร์ด"
	description={student
		? `สวัสดี, ${student.first_name} ${student.last_name}`
		: 'ภาพรวมข้อมูลนักเรียน'}
>
	{#if student}<Button variant="outline" disabled={loading || contextLoading} onclick={loadProfile}
			>โหลดข้อมูลใหม่</Button
		>{/if}
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
			<!-- Student Info Cards -->
			<div class="grid grid-cols-1 md:grid-cols-3 gap-6">
				<!-- Student ID Card -->
				<Card class="p-6 hover:shadow-md transition-shadow">
					<div class="flex items-start justify-between">
						<div class="space-y-2">
							<p class="text-sm text-muted-foreground font-medium">รหัสนักเรียน</p>
							<p class="text-2xl font-bold text-foreground">
								{student.student_id || '-'}
							</p>
						</div>
						<div class="w-12 h-12 bg-primary/10 rounded-lg flex items-center justify-center">
							<User class="w-6 h-6 text-primary" />
						</div>
					</div>
				</Card>

				<!-- Class Card -->
				<Card class="p-6 hover:shadow-md transition-shadow">
					<div class="flex items-start justify-between">
						<div class="space-y-2">
							<p class="text-sm text-muted-foreground font-medium">ชั้นเรียน</p>
							<p class="text-2xl font-bold text-foreground">
								{#if student.grade_level && student.homeroom}
									{student.grade_level}/{student.homeroom}
								{:else}
									-
								{/if}
							</p>
						</div>
						<div class="w-12 h-12 bg-blue-500/10 rounded-lg flex items-center justify-center">
							<BookOpen class="w-6 h-6 text-blue-500" />
						</div>
					</div>
				</Card>

				<!-- Attendance Card -->
				<Card class="p-6 hover:shadow-md transition-shadow">
					<div class="flex items-start justify-between">
						<div class="space-y-2">
							<p class="text-sm text-muted-foreground font-medium">การเข้าเรียน</p>
							<p class="text-2xl font-bold text-muted-foreground">—</p>
							<p class="text-xs text-muted-foreground">ยังไม่เปิดใช้งาน</p>
						</div>
						<div class="w-12 h-12 bg-muted rounded-lg flex items-center justify-center">
							<Calendar class="w-6 h-6 text-muted-foreground" />
						</div>
					</div>
				</Card>
			</div>

			<!-- Quick Actions -->
			<Card class="p-6">
				<h2 class="text-xl font-semibold mb-4">เมนูด่วน</h2>
				<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
					<Button
						variant="outline"
						class="h-auto py-4 flex-col gap-2"
						href={profileHref}
						data-sveltekit-preload-data="tap"
					>
						<User class="w-6 h-6" />
						<span>ข้อมูลส่วนตัว</span>
					</Button>

					<Button
						variant="outline"
						class="h-auto py-4 flex-col gap-2"
						href={timetableHref}
						data-sveltekit-preload-data="tap"
					>
						<BookOpen class="w-6 h-6" />
						<span>ตารางเรียน</span>
					</Button>

					<Button variant="outline" class="h-auto py-4 flex-col gap-2" disabled>
						<Award class="w-6 h-6" />
						<span>คะแนน</span>
					</Button>

					<Button variant="outline" class="h-auto py-4 flex-col gap-2" disabled>
						<Calendar class="w-6 h-6" />
						<span>การเข้าเรียน</span>
					</Button>
				</div>
				<p class="text-sm text-muted-foreground mt-4 text-center">
					เมนูที่เป็นสีเทาจะเปิดใช้งานในอนาคต
				</p>
			</Card>

			<!-- Announcements (placeholder) -->
			<Card class="p-6">
				<h2 class="text-xl font-semibold mb-4">ประกาศ</h2>
				<div class="text-center py-8 text-muted-foreground">
					<p>ไม่มีประกาศในขณะนี้</p>
				</div>
			</Card>
		{:else}
			<PageState
				title="ไม่พบข้อมูลนักเรียน"
				description="ไม่พบโปรไฟล์นักเรียนของบัญชีนี้ กรุณาติดต่อผู้ดูแลระบบ"
			/>
		{/if}
	</div>
</PageShell>
