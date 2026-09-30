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
	import { resolveScopedAcademicYearUrl } from '$lib/academic-context/scoped-year';
	import ScopedAcademicYearSelect from '$lib/components/academic-context/ScopedAcademicYearSelect.svelte';
	import { Card } from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Textarea } from '$lib/components/ui/textarea';
	import { toast } from 'svelte-sonner';
	import { User, Edit, Save, X } from '@lucide/svelte';
	import { getOwnProfile, updateOwnProfile, type Student } from '$lib/api/students';

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
		owner = '',
		ownerEpoch = 0;
	const contextRequest = new LatestRequest(),
		profileRequest = new LatestRequest();
	let consumedContext: typeof data.context | null = null,
		consumedProfile: typeof data.profile | null = null;
	let editing = $state(false),
		saving = $state(false),
		draftEpoch = 0;
	let phone = $state(''),
		address = $state(''),
		nickname = $state('');
	const academicYearQuery = $derived(
		selectedYearId ? `?academicYearId=${encodeURIComponent(selectedYearId)}` : ''
	);
	const dashboardHref = $derived(`${resolve('/student')}${academicYearQuery}`);
	$effect.pre(() => {
		const key = ownerKey,
			a = data.context,
			b = data.profile,
			canRead = allowed;
		untrack(() => {
			if (owner !== key || !canRead) {
				owner = key;
				ownerEpoch++;
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
				editing = false;
				saving = false;
				draftEpoch++;
				phone = '';
				address = '';
				nickname = '';
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
		ownerEpoch++;
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
			replaceState(resolve(`${url.pathname}${url.search}` as '/student/profile'), page.state);
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
		await goto(
			resolve(
				`/student/profile?academicYearId=${encodeURIComponent(yearId)}` as '/student/profile'
			),
			{ noScroll: true, keepFocus: true }
		);
	}
	function startEdit() {
		draftEpoch++;
		phone = student?.phone ?? '';
		address = student?.address ?? '';
		nickname = student?.nickname ?? '';
		editing = true;
	}
	function handleCancel() {
		draftEpoch++;
		editing = false;
	}
	async function handleSave() {
		if (saving || !editing || !student) return;
		const key = ownerKey,
			epoch = ownerEpoch,
			draft = draftEpoch;
		const payload = { phone, address, nickname };
		profileRequest.abort();
		loading = false;
		saving = true;
		try {
			await updateOwnProfile(payload);
			if (!current(key) || epoch !== ownerEpoch) return;
			toast.success('บันทึกข้อมูลสำเร็จ');
			if (draft === draftEpoch) editing = false;
			await loadProfile();
		} catch (e) {
			if (current(key) && epoch === ownerEpoch)
				toast.error(e instanceof Error ? e.message : 'บันทึกข้อมูลไม่สำเร็จ');
		} finally {
			if (current(key) && epoch === ownerEpoch) saving = false;
		}
	}
</script>

<PageShell
	title="ข้อมูลส่วนตัว"
	description="ดูและแก้ไขข้อมูลส่วนตัวของคุณ"
	backHref={dashboardHref}
	backPreload="tap"
>
	{#snippet actions()}
		{#if student && !editing && !loading}
			<Button onclick={startEdit}>
				<Edit class="w-4 h-4 mr-2" />
				แก้ไขข้อมูล
			</Button>
		{/if}
	{/snippet}

	{#if student}<Button
			variant="outline"
			disabled={loading || contextLoading || saving}
			onclick={loadProfile}>โหลดข้อมูลใหม่</Button
		>{/if}
	{#if contextOptions && contextOptions.years.length > 0}
		<div class="flex max-w-sm flex-col gap-2 rounded-xl border bg-card p-4">
			<Label for="student-profile-year">ปีการศึกษา</Label>
			<ScopedAcademicYearSelect
				id="student-profile-year"
				years={contextOptions.years}
				value={selectedYearId}
				disabled={contextLoading || saving || editing}
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
				<PageSkeleton variant="form" rows={6} />
			</div>
		{:else if contextOptions && contextOptions.years.length === 0 && !contextError}
			<PageState
				title="ยังไม่มีประวัติปีการศึกษาสำหรับบัญชีนี้"
				description="กรุณาติดต่อผู้ดูแลระบบเพื่อตรวจสอบการลงทะเบียนนักเรียน"
			/>
		{:else if student}
			<!-- Basic Information (Read-only) -->
			<Card class="p-6">
				<div class="flex items-center gap-3 mb-6">
					<div class="w-10 h-10 bg-primary/10 rounded-lg flex items-center justify-center">
						<User class="w-5 h-5 text-primary" />
					</div>
					<h2 class="text-xl font-semibold">ข้อมูลพื้นฐาน</h2>
				</div>

				<div class="grid grid-cols-1 md:grid-cols-2 gap-6">
					<div class="space-y-2">
						<Label>ชื่อ-นามสกุล</Label>
						<div class="px-3 py-2 bg-muted/50 rounded-md text-foreground">
							{student.title || ''}
							{student.first_name}
							{student.last_name}
						</div>
					</div>

					<div class="space-y-2">
						<Label>รหัสนักเรียน</Label>
						<div class="px-3 py-2 bg-muted/50 rounded-md text-foreground">
							{student.student_id || '-'}
						</div>
					</div>

					<div class="space-y-2">
						<Label>ระดับชั้น</Label>
						<div class="px-3 py-2 bg-muted/50 rounded-md text-foreground">
							{#if student.grade_level && student.homeroom}
								{student.grade_level}/{student.homeroom}
							{:else}
								-
							{/if}
						</div>
					</div>

					<div class="space-y-2">
						<Label>เพศ</Label>
						<div class="px-3 py-2 bg-muted/50 rounded-md text-foreground">
							{#if student.gender === 'male'}
								ชาย
							{:else if student.gender === 'female'}
								หญิง
							{:else}
								-
							{/if}
						</div>
					</div>

					<div class="space-y-2">
						<Label>วันเกิด</Label>
						<div class="px-3 py-2 bg-muted/50 rounded-md text-foreground">
							{student.date_of_birth || '-'}
						</div>
					</div>

					<div class="space-y-2">
						<Label>อีเมล</Label>
						<div class="px-3 py-2 bg-muted/50 rounded-md text-foreground">
							{student.email || '-'}
						</div>
					</div>
				</div>

				<p class="text-sm text-muted-foreground mt-4">
					ข้อมูลเหล่านี้ไม่สามารถแก้ไขได้ หากพบข้อผิดพลาดกรุณาติดต่อผู้ดูแลระบบ
				</p>
			</Card>

			<!-- Editable Information -->
			<Card class="p-6">
				<h2 class="text-xl font-semibold mb-6">ข้อมูลติดต่อ</h2>

				{#if editing}
					<div class="space-y-6">
						<div class="space-y-2">
							<Label for="nickname">ชื่อเล่น</Label>
							<Input
								id="nickname"
								type="text"
								bind:value={nickname}
								placeholder="ชื่อเล่น"
								disabled={saving}
							/>
						</div>

						<div class="space-y-2">
							<Label for="phone">เบอร์โทรศัพท์</Label>
							<Input
								id="phone"
								type="tel"
								bind:value={phone}
								placeholder="0812345678"
								disabled={saving}
							/>
						</div>

						<div class="space-y-2">
							<Label for="address">ที่อยู่</Label>
							<Textarea
								id="address"
								bind:value={address}
								placeholder="ที่อยู่ปัจจุบัน"
								rows={4}
								disabled={saving}
							/>
						</div>

						<div class="flex gap-3">
							<Button onclick={handleSave} disabled={saving} class="flex-1">
								{#if saving}
									กำลังบันทึก...
								{:else}
									<Save class="w-4 h-4 mr-2" />
									บันทึก
								{/if}
							</Button>
							<Button variant="outline" onclick={handleCancel} disabled={saving}>
								<X class="w-4 h-4 mr-2" />
								ยกเลิก
							</Button>
						</div>
					</div>
				{:else}
					<div class="grid grid-cols-1 md:grid-cols-2 gap-6">
						<div class="space-y-2">
							<Label>ชื่อเล่น</Label>
							<div class="px-3 py-2 bg-muted/50 rounded-md text-foreground">
								{student.nickname || '-'}
							</div>
						</div>

						<div class="space-y-2">
							<Label>เบอร์โทรศัพท์</Label>
							<div class="px-3 py-2 bg-muted/50 rounded-md text-foreground">
								{student.phone || '-'}
							</div>
						</div>

						<div class="space-y-2 md:col-span-2">
							<Label>ที่อยู่</Label>
							<div class="px-3 py-2 bg-muted/50 rounded-md text-foreground min-h-[80px]">
								{student.address || '-'}
							</div>
						</div>
					</div>
				{/if}
			</Card>

			<!-- Medical Information (Read-only) -->
			{#if student.blood_type || student.allergies || student.medical_conditions}
				<Card class="p-6">
					<h2 class="text-xl font-semibold mb-6">ข้อมูลสุขภาพ</h2>

					<div class="grid grid-cols-1 md:grid-cols-2 gap-6">
						{#if student.blood_type}
							<div class="space-y-2">
								<Label>หมู่เลือด</Label>
								<div class="px-3 py-2 bg-muted/50 rounded-md text-foreground">
									{student.blood_type}
								</div>
							</div>
						{/if}

						{#if student.allergies}
							<div class="space-y-2 md:col-span-2">
								<Label>อาการแพ้</Label>
								<div class="px-3 py-2 bg-muted/50 rounded-md text-foreground">
									{student.allergies}
								</div>
							</div>
						{/if}

						{#if student.medical_conditions}
							<div class="space-y-2 md:col-span-2">
								<Label>โรคประจำตัว</Label>
								<div class="px-3 py-2 bg-muted/50 rounded-md text-foreground">
									{student.medical_conditions}
								</div>
							</div>
						{/if}
					</div>

					<p class="text-sm text-muted-foreground mt-4">
						ข้อมูลสุขภาพไม่สามารถแก้ไขได้ ติดต่อผู้ดูแลระบบหากต้องการเปลี่ยนแปลง
					</p>
				</Card>
			{/if}
		{:else}
			<PageState
				title="ไม่พบข้อมูลนักเรียน"
				description="ไม่พบโปรไฟล์นักเรียนของบัญชีนี้ กรุณาติดต่อผู้ดูแลระบบ"
			/>
		{/if}
	</div>
</PageShell>
