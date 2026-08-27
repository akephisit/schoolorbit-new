<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import type { PageProps } from './$types';
	import { onMount } from 'svelte';
	import { SvelteURLSearchParams } from 'svelte/reactivity';
	import { toast } from 'svelte-sonner';
	import {
		listMyAcademicContextOptions,
		type AcademicContextOptionsResponse
	} from '$lib/api/academic-context';
	import {
		enrollMyActivityRegistration,
		getStudentActivityTypeLabel,
		listMyActivityRegistrations,
		unenrollMyActivityRegistration,
		type StudentActivityOffering,
		type StudentActivityRegistrationResult
	} from '$lib/api/student-activities';
	import { PageShell } from '$lib/components/app-layout';
	import { LoadingButton, PageSkeleton, PageState } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import { CheckCircle2, Clock3, UserRound, UsersRound, X } from 'lucide-svelte';

	let { data }: PageProps = $props();
	let contextOptions = $state<AcademicContextOptionsResponse | null>(null);
	let selectedYearId = $state('');
	let selectedTermId = $state('');
	let offerings = $state.raw<StudentActivityOffering[]>([]);
	let loading = $state(true);
	let errorMessage = $state('');
	let actionGroupId = $state('');
	let requestRevision = 0;

	const termOptions = $derived(
		contextOptions?.terms.filter((term) => term.academicYearId === selectedYearId) ?? []
	);
	const registeredCount = $derived(offerings.filter((offering) => offering.enrolledGroupId).length);

	function authorizedSelection(options: AcademicContextOptionsResponse): {
		yearId: string;
		termId: string;
	} {
		const queryYearId = page.url.searchParams.get('academicYearId');
		const yearId =
			options.years.find((year) => year.id === queryYearId)?.id ??
			options.years.find((year) => year.id === options.activeAcademicYearId)?.id ??
			options.years[0]?.id ??
			'';
		const terms = options.terms.filter((term) => term.academicYearId === yearId);
		const queryTermId = page.url.searchParams.get('academicTermId');
		const termId =
			terms.find((term) => term.id === queryTermId)?.id ??
			terms.find((term) => term.id === options.activeAcademicTermId)?.id ??
			terms[0]?.id ??
			'';
		return { yearId, termId };
	}

	async function loadHistory(): Promise<void> {
		const current = ++requestRevision;
		loading = true;
		errorMessage = '';
		try {
			const options = await listMyAcademicContextOptions();
			if (current !== requestRevision) return;
			contextOptions = options;
			const selection = authorizedSelection(options);
			selectedYearId = selection.yearId;
			selectedTermId = selection.termId;
			if (selectedTermId) {
				const loaded = await listMyActivityRegistrations({ academicTermId: selectedTermId });
				if (current !== requestRevision) return;
				offerings = loaded;
			} else {
				offerings = [];
			}
		} catch (error) {
			if (current !== requestRevision) return;
			errorMessage = error instanceof Error ? error.message : 'โหลดกิจกรรมไม่สำเร็จ';
		} finally {
			if (current === requestRevision) loading = false;
		}
	}

	async function loadActivities(): Promise<void> {
		const current = ++requestRevision;
		loading = true;
		errorMessage = '';
		try {
			const loaded = selectedTermId
				? await listMyActivityRegistrations({ academicTermId: selectedTermId })
				: [];
			if (current !== requestRevision) return;
			offerings = loaded;
		} catch (error) {
			if (current !== requestRevision) return;
			errorMessage = error instanceof Error ? error.message : 'โหลดกิจกรรมไม่สำเร็จ';
		} finally {
			if (current === requestRevision) loading = false;
		}
	}

	async function updateUrl(yearId: string, termId: string): Promise<void> {
		const query = new SvelteURLSearchParams({ academicYearId: yearId });
		if (termId) query.set('academicTermId', termId);
		await goto(resolve(`/student/activities?${query.toString()}`), {
			noScroll: true,
			keepFocus: true
		});
	}

	async function changeYear(yearId: string): Promise<void> {
		const availableTerms =
			contextOptions?.terms.filter((term) => term.academicYearId === yearId) ?? [];
		const nextTerm =
			availableTerms.find((term) => term.id === contextOptions?.activeAcademicTermId) ??
			availableTerms[0];
		selectedYearId = yearId;
		selectedTermId = nextTerm?.id ?? '';
		offerings = [];
		await updateUrl(selectedYearId, selectedTermId);
		await loadActivities();
	}

	async function changeTerm(value: string): Promise<void> {
		selectedTermId = value;
		offerings = [];
		await updateUrl(selectedYearId, selectedTermId);
		await loadActivities();
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
		if (!selectedTermId) return;
		actionGroupId = groupId;
		try {
			const result = await enrollMyActivityRegistration(selectedTermId, groupId);
			applyRegistrationResult(result);
			toast.success('ลงทะเบียนกิจกรรมแล้ว');
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ลงทะเบียนกิจกรรมไม่สำเร็จ');
		} finally {
			actionGroupId = '';
		}
	}

	async function unregister(groupId: string): Promise<void> {
		if (!selectedTermId || !confirm('ยกเลิกการลงทะเบียนกลุ่มกิจกรรมนี้?')) return;
		actionGroupId = groupId;
		try {
			const result = await unenrollMyActivityRegistration(selectedTermId, groupId);
			applyRegistrationResult(result);
			toast.success('ยกเลิกการลงทะเบียนแล้ว');
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ยกเลิกการลงทะเบียนไม่สำเร็จ');
		} finally {
			actionGroupId = '';
		}
	}

	onMount(loadHistory);
</script>

<PageShell title={data.title} description="เลือกกลุ่มกิจกรรมของฉันตามปีการศึกษาและภาคเรียน">
	<div class="flex flex-wrap gap-3 rounded-xl border bg-card p-4">
		<div class="min-w-52 space-y-2">
			<Label for="student-activity-year">ปีการศึกษา</Label>
			<Select.Root
				type="single"
				value={selectedYearId}
				disabled={loading || actionGroupId !== ''}
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
				disabled={loading || actionGroupId !== '' || termOptions.length === 0}
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

	{#if loading}
		<PageSkeleton variant="cards" rows={3} />
	{:else if errorMessage}
		<PageState
			variant="error"
			title="โหลดกิจกรรมไม่สำเร็จ"
			description={errorMessage}
			actionLabel="ลองอีกครั้ง"
			onaction={loadHistory}
		/>
	{:else if !contextOptions || contextOptions.years.length === 0}
		<PageState
			title="ยังไม่มีประวัติปีการศึกษา"
			description="เมื่อโรงเรียนสร้างข้อมูลนักเรียนประจำปีแล้ว ตัวเลือกกิจกรรมจะปรากฏที่นี่"
		/>
	{:else if !selectedTermId}
		<PageState
			title="ปีการศึกษานี้ยังไม่มีภาคเรียน"
			description="โรงเรียนต้องสร้างภาคเรียนก่อนจึงจะเปิดกิจกรรมให้นักเรียนลงทะเบียนได้"
		/>
	{:else if offerings.length === 0}
		<PageState
			title="ยังไม่มีกิจกรรมที่เปิดลงทะเบียน"
			description="ไม่มีกิจกรรมแบบสมัครเองที่ตรงกับระดับชั้น แผนการเรียน และห้องเรียนของฉันในภาคเรียนนี้"
		/>
	{:else}
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
										<p class="text-muted-foreground font-mono text-xs">{group.code}</p>
										<h3 class="mt-1 font-semibold">{group.name}</h3>
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
</PageShell>
