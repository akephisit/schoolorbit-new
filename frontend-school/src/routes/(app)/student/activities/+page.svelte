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
	import { toast } from 'svelte-sonner';
	import {
		listMyAcademicContextOptions,
		type AcademicContextOptionsResponse
	} from '#lib/api/academic-context.js';
	import {
		enrollMyActivityRegistration,
		getStudentActivityTypeLabel,
		listMyActivityRegistrations,
		unenrollMyActivityRegistration,
		type StudentActivityOffering,
		type StudentActivityRegistrationResult
	} from '#lib/api/student-activities.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { LoadingButton, PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import { CheckCircle2, Clock3, UserRound, UsersRound, X } from '@lucide/svelte';

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
	let offerings = $state.raw<StudentActivityOffering[]>([]);
	let loading = $state(true),
		loaded = $state(false),
		error = $state(''),
		contextLoading = $state(true),
		contextError = $state('');
	let disposed = false,
		owner = '',
		ownerEpoch = 0;
	const contextRequest = new LatestRequest(),
		primaryRequest = new LatestRequest();
	let consumedContext: typeof data.context | null = null,
		consumedRecords: typeof data.records | null = null;
	let actionGroupId = $state('');
	const termOptions = $derived(
		contextOptions?.terms.filter((term) => term.academicYearId === selectedYearId) ?? []
	);
	const registeredCount = $derived(offerings.filter((offering) => offering.enrolledGroupId).length);
	$effect.pre(() => {
		const key = ownerKey,
			a = data.context,
			b = data.records,
			canRead = allowed;
		untrack(() => {
			if (owner !== key || !canRead) {
				owner = key;
				ownerEpoch++;
				contextRequest.abort();
				primaryRequest.abort();
				contextOptions = null;
				selectedYearId = '';
				selectedTermId = '';
				offerings = [];
				loaded = false;
				loading = canRead;
				contextLoading = canRead;
				error = '';
				contextError = '';
				actionGroupId = '';
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
		ownerEpoch++;
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
			goto(resolve('student/activities') + url.search, {
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
		offerings = v.data.records;
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
			listMyActivityRegistrations({ academicTermId: selectedTermId }, t.signal).then((records) => ({
				ownerKey: key,
				records
			})),
			'โหลดกิจกรรมไม่สำเร็จ'
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

		await goto(resolve('student/activities') + url.search, {
			reset: false
		});
	}
	async function changeYear(yearId: string) {
		if (
			!contextOptions?.years.some((year) => year.id === yearId) ||
			yearId === selectedYearId ||
			actionGroupId !== ''
		)
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
		if (termId === selectedTermId || actionGroupId !== '') return;
		await updateUrl(selectedYearId, termId);
	}

	function applyRegistrationResult(result: StudentActivityRegistrationResult): void {
		offerings = offerings.map((offering) => {
			if (offering.id !== result.learningOfferingId) return offering;
			const previousGroupId = offering.enrolledGroupId ?? null;
			return {
				...offering,
				enrolledGroupId: result.enrolled ? result.learningGroupId : null,
				groups: offering.groups.map((group) => {
					const wasEnrolled = group.id === previousGroupId;
					const isEnrolled = result.enrolled && group.id === result.learningGroupId;
					let memberCount = group.memberCount;
					if (!wasEnrolled && isEnrolled) memberCount += 1;
					if (wasEnrolled && !isEnrolled) memberCount = Math.max(0, memberCount - 1);
					return { ...group, enrolled: isEnrolled, memberCount };
				})
			};
		});
	}

	async function register(groupId: string): Promise<void> {
		if (!selectedTermId || actionGroupId || !loaded) return;
		const key = ownerKey,
			epoch = ownerEpoch;
		primaryRequest.abort();
		loading = false;
		error = '';
		actionGroupId = groupId;
		try {
			const result = await enrollMyActivityRegistration(selectedTermId, groupId);
			if (!current(key) || epoch !== ownerEpoch) return;
			applyRegistrationResult(result);
			toast.success('ลงทะเบียนกิจกรรมแล้ว');
		} catch (error) {
			if (!current(key) || epoch !== ownerEpoch) return;
			toast.error(error instanceof Error ? error.message : 'ลงทะเบียนกิจกรรมไม่สำเร็จ');
		} finally {
			if (current(key) && epoch === ownerEpoch) actionGroupId = '';
		}
	}

	async function unregister(groupId: string): Promise<void> {
		if (
			actionGroupId ||
			!loaded ||
			!selectedTermId ||
			!confirm('ยกเลิกการลงทะเบียนกลุ่มกิจกรรมนี้?')
		)
			return;
		const key = ownerKey,
			epoch = ownerEpoch;
		primaryRequest.abort();
		loading = false;
		error = '';
		actionGroupId = groupId;
		try {
			const result = await unenrollMyActivityRegistration(selectedTermId, groupId);
			if (!current(key) || epoch !== ownerEpoch) return;
			applyRegistrationResult(result);
			toast.success('ยกเลิกการลงทะเบียนแล้ว');
		} catch (error) {
			if (!current(key) || epoch !== ownerEpoch) return;
			toast.error(error instanceof Error ? error.message : 'ยกเลิกการลงทะเบียนไม่สำเร็จ');
		} finally {
			if (current(key) && epoch === ownerEpoch) actionGroupId = '';
		}
	}
</script>

<PageShell title={data.title} description="เลือกกลุ่มกิจกรรมของฉันตามปีการศึกษาและภาคเรียน">
	<Button
		variant="outline"
		disabled={loading || contextLoading || actionGroupId !== ''}
		onclick={loadPrimary}>โหลดข้อมูลใหม่</Button
	>

	<div class="flex flex-wrap gap-3 rounded-xl border bg-card p-4">
		<div class="min-w-52 space-y-2">
			<Label for="student-activity-year">ปีการศึกษา</Label>
			<Select.Root
				type="single"
				value={selectedYearId}
				disabled={contextLoading || actionGroupId !== ''}
				onValueChange={(value) => void changeYear(value)}
			>
				<Select.Trigger id="student-activity-year" class="w-full">
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
			<Label for="student-activity-term">ภาคเรียน</Label>
			<Select.Root
				type="single"
				value={selectedTermId}
				disabled={contextLoading || actionGroupId !== '' || termOptions.length === 0}
				onValueChange={(value) => void changeTerm(value)}
			>
				<Select.Trigger id="student-activity-term" class="w-full">
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
			title="โหลดกิจกรรมไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={loadPrimary}
		/>
	{/if}
	<div data-testid="student-activities-region" aria-busy={loading || contextLoading}>
		{#if loading && loaded}
			<p role="status" aria-label="กำลังอัปเดตข้อมูล" class="text-muted-foreground text-sm">
				กำลังอัปเดตข้อมูล…
			</p>
		{/if}
		{#if contextLoading || (loading && !loaded)}
			<div role="status" aria-label="กำลังโหลดลงทะเบียนกิจกรรม">
				<PageSkeleton variant="cards" rows={3} />
			</div>
		{:else if contextOptions && contextOptions.years.length === 0 && !contextError}
			<PageState
				title="ยังไม่มีประวัติปีการศึกษา"
				description="เมื่อโรงเรียนสร้างข้อมูลนักเรียนประจำปีแล้ว ตัวเลือกกิจกรรมจะปรากฏที่นี่"
			/>
		{:else if contextOptions && !contextError && !selectedTermId}
			<PageState
				title="ปีการศึกษานี้ยังไม่มีภาคเรียน"
				description="โรงเรียนต้องสร้างภาคเรียนก่อนจึงจะเปิดกิจกรรมให้นักเรียนลงทะเบียนได้"
			/>
		{:else if loaded && offerings.length === 0}
			<PageState
				title="ยังไม่มีกิจกรรมที่เปิดลงทะเบียน"
				description="ไม่มีกิจกรรมแบบสมัครเองที่ตรงกับระดับชั้น แผนการเรียน และห้องเรียนของฉันในภาคเรียนนี้"
			/>
		{:else if loaded}
			<div
				class="flex flex-col gap-2 rounded-xl border border-emerald-500/25 bg-emerald-500/5 p-4 sm:flex-row sm:items-center sm:justify-between"
				aria-live="polite"
			>
				<div>
					<p class="font-semibold text-emerald-800 dark:text-emerald-200">สถานะการลงทะเบียน</p>
					<p class="text-muted-foreground text-sm">
						เลือกแล้ว {registeredCount} จาก {offerings.length} กิจกรรมที่เปิดให้ฉัน
					</p>
				</div>
				<Badge variant={registeredCount === offerings.length ? 'default' : 'secondary'}>
					{registeredCount === offerings.length ? 'เลือกครบแล้ว' : 'ยังเลือกได้'}
				</Badge>
			</div>

			<div class="space-y-4">
				{#each offerings as offering (offering.id)}
					<section class="overflow-hidden rounded-xl border bg-card">
						<header
							class={[
								'border-b border-l-4 p-4 sm:p-5',
								offering.enrolledGroupId
									? 'border-l-emerald-500 bg-emerald-500/5'
									: 'border-l-sky-500 bg-sky-500/5'
							]}
						>
							<div class="flex flex-wrap items-start justify-between gap-3">
								<div class="min-w-0">
									<div class="flex flex-wrap items-center gap-2">
										<span class="text-muted-foreground font-mono text-xs">{offering.code}</span>
										<Badge variant="outline">
											{getStudentActivityTypeLabel(offering.activityType)}
										</Badge>
									</div>
									<h2 class="mt-2 text-lg font-semibold tracking-tight">{offering.name}</h2>
								</div>
								{#if offering.enrolledGroupId}
									<Badge class="gap-1 bg-emerald-600 text-white hover:bg-emerald-600">
										<CheckCircle2 class="size-3.5" /> ลงทะเบียนแล้ว
									</Badge>
								{:else}
									<Badge variant="secondary">เลือก 1 กลุ่ม</Badge>
								{/if}
							</div>
						</header>

						<div class="grid gap-3 p-4 lg:grid-cols-2">
							{#each offering.groups as group (group.id)}
								{@const isFull = group.capacity != null && group.memberCount >= group.capacity}
								<article
									class={[
										'flex min-h-48 flex-col rounded-lg border p-4 transition-colors',
										group.enrolled
											? 'border-emerald-500/50 bg-emerald-500/5'
											: 'hover:border-sky-500/40'
									]}
								>
									<div class="flex items-start justify-between gap-3">
										<div>
											<h3 class="font-semibold">{group.name}</h3>
										</div>
										{#if group.enrolled}
											<CheckCircle2 class="size-5 shrink-0 text-emerald-600" />
										{/if}
									</div>
									{#if group.description}
										<p class="text-muted-foreground mt-2 text-sm">{group.description}</p>
									{/if}
									<div class="text-muted-foreground mt-4 space-y-2 text-sm">
										<p class="flex items-start gap-2">
											<UserRound class="mt-0.5 size-4 shrink-0" />
											<span>{group.teacherNames.join(' · ') || 'ยังไม่ระบุครูผู้ดูแล'}</span>
										</p>
										<p class="flex items-center gap-2">
											<UsersRound class="size-4 shrink-0" />
											<span>
												{group.memberCount}{group.capacity !== null ? ` / ${group.capacity}` : ''} คน
											</span>
											{#if isFull && !group.enrolled}
												<Badge variant="destructive">เต็ม</Badge>
											{/if}
										</p>
									</div>

									<div class="mt-auto pt-4">
										{#if group.enrolled && group.registrationOpen}
											<LoadingButton
												variant="outline"
												class="w-full gap-2"
												loading={actionGroupId === group.id}
												loadingLabel="กำลังยกเลิก..."
												disabled={actionGroupId !== '' && actionGroupId !== group.id}
												onclick={() => unregister(group.id)}
											>
												<X class="size-4" /> ยกเลิกการลงทะเบียน
											</LoadingButton>
										{:else if group.enrolled}
											<div class="text-muted-foreground flex items-center gap-2 text-sm">
												<Clock3 class="size-4" /> ปิดแก้ไขรายชื่อแล้ว
											</div>
										{:else if offering.enrolledGroupId}
											<p class="text-muted-foreground text-sm">เลือกกลุ่มอื่นในกิจกรรมนี้แล้ว</p>
										{:else if !group.registrationOpen}
											<p class="text-muted-foreground flex items-center gap-2 text-sm">
												<Clock3 class="size-4" /> ปิดรับลงทะเบียนแล้ว
											</p>
										{:else if isFull}
											<p class="text-destructive text-sm">กลุ่มนี้เต็มแล้ว</p>
										{:else}
											<LoadingButton
												class="w-full"
												loading={actionGroupId === group.id}
												loadingLabel="กำลังลงทะเบียน..."
												disabled={actionGroupId !== '' && actionGroupId !== group.id}
												onclick={() => register(group.id)}
											>
												ลงทะเบียนกลุ่มนี้
											</LoadingButton>
										{/if}
									</div>
								</article>
							{/each}
						</div>
					</section>
				{/each}
			</div>
		{/if}
	</div>
</PageShell>
