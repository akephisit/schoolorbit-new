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
	import { periodsFromTimetableBlocks, type TimetableBlock } from '#lib/api/timetable.js';
	import { getChildTimetable, getChildProfile } from '#lib/api/parents.js';
	import type { Student } from '#lib/api/students.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import { MapPin, School } from '@lucide/svelte';

	const dayOptions = [
		{ value: 'MON', label: 'จันทร์' },
		{ value: 'TUE', label: 'อังคาร' },
		{ value: 'WED', label: 'พุธ' },
		{ value: 'THU', label: 'พฤหัสบดี' },
		{ value: 'FRI', label: 'ศุกร์' },
		{ value: 'SAT', label: 'เสาร์' },
		{ value: 'SUN', label: 'อาทิตย์' }
	];

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
		`/parent/student/${encodeURIComponent(studentId)}?academicYearId=${encodeURIComponent(selectedYearId)}`
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
	const periods = $derived(periodsFromTimetableBlocks(blocks));
	const schoolDays = $derived.by(() => {
		const configured = new Set(blocks.map((block) => block.dayOfWeek));
		return configured.size > 0
			? dayOptions.filter((day) => configured.has(day.value))
			: dayOptions.slice(0, 5);
	});
	const tableMinWidth = $derived(96 + periods.length * 132);

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

	function blocksForCell(day: string, periodId: string): TimetableBlock[] {
		return blocks.filter(
			(block) => block.dayOfWeek === day && block.bellSchedulePeriodId === periodId
		);
	}

	function blockTitle(block: TimetableBlock): string {
		return block.offeringCode ?? block.title ?? 'กิจกรรม';
	}

	function blockColor(blockKind: TimetableBlock['blockKind']): string {
		if (blockKind === 'course')
			return 'border-blue-200 bg-blue-50 text-blue-950 dark:border-blue-800 dark:bg-blue-950/40 dark:text-blue-100';
		if (blockKind === 'activity')
			return 'border-emerald-200 bg-emerald-50 text-emerald-950 dark:border-emerald-800 dark:bg-emerald-950/40 dark:text-emerald-100';
		return 'border-amber-200 bg-amber-50 text-amber-950 dark:border-amber-800 dark:bg-amber-950/40 dark:text-amber-100';
	}

	function groupLabel(block: TimetableBlock): string {
		return [
			...block.groups.map((group) => group.name),
			...block.homerooms.map((room) => room.name)
		].join(', ');
	}

	function roomLabel(block: TimetableBlock): string {
		return [
			...block.groups.map((group) => group.roomCode),
			...block.homerooms.map((room) => room.roomCode)
		]
			.filter(Boolean)
			.join(', ');
	}
</script>

<PageShell
	backHref={childDetailHref}
	title="ตารางเรียน"
	description={child
		? `ตารางเรียนของ ${child.first_name} ${child.last_name}`
		: 'ดูตารางเรียนย้อนหลังของนักเรียน'}
>
	<Button variant="outline" disabled={loading || contextLoading} onclick={loadPrimary}
		>โหลดข้อมูลใหม่</Button
	>

	<div data-testid="parent-child-profile-region" aria-busy={childLoading || contextLoading}>
		{#if childError}<PageState
				variant="error"
				title="โหลดข้อมูลนักเรียนไม่สำเร็จ"
				description={childError}
				actionLabel="ลองข้อมูลนักเรียนอีกครั้ง"
				onaction={loadChild}
			/>{/if}
		{#if childLoading && child}<p
				role="status"
				aria-label="กำลังอัปเดตข้อมูลนักเรียน"
				class="text-muted-foreground text-sm"
			>
				กำลังอัปเดตข้อมูลนักเรียน…
			</p>{/if}
		{#if (childLoading || contextLoading) && !child && !contextError}<div
				role="status"
				aria-label="กำลังโหลดข้อมูลนักเรียน"
			>
				<PageSkeleton variant="detail" />
			</div>
		{:else if child}<p>
				{child.first_name}
				{child.last_name} · {child.grade_level} · ห้อง {child.homeroom}
			</p>{/if}
	</div>

	<div class="flex flex-wrap gap-3 rounded-xl border bg-card p-4">
		<div class="min-w-52 space-y-2">
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
		<div class="min-w-52 space-y-2">
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
		{#if loading && loaded}
			<p role="status" aria-label="กำลังอัปเดตข้อมูล" class="text-muted-foreground text-sm">
				กำลังอัปเดตข้อมูล…
			</p>
		{/if}
		{#if contextLoading || (loading && !loaded)}
			<div role="status" aria-label="กำลังโหลดตารางเรียน">
				<PageSkeleton variant="table" rows={6} columns={Math.max(periods.length + 1, 4)} />
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
			<div class="overflow-x-auto rounded-lg border">
				<table class="w-full table-fixed border-collapse" style={`min-width: ${tableMinWidth}px`}>
					<thead
						><tr
							><th class="bg-muted/70 w-24 border p-2 text-xs">วัน / คาบ</th
							>{#each periods as period, index (period.id)}<th
									class="bg-muted/70 border p-2 text-center text-xs"
									><p class="font-semibold">{period.name ?? `คาบ ${index + 1}`}</p>
									<p class="text-muted-foreground font-normal">
										{period.startTime.slice(0, 5)}–{period.endTime.slice(0, 5)}
									</p></th
								>{/each}</tr
						></thead
					>
					<tbody
						>{#each schoolDays as day (day.value)}<tr
								><th class="bg-muted/30 border p-2 text-xs">{day.label}</th
								>{#each periods as period (period.id)}{@const cellBlocks = blocksForCell(
										day.value,
										period.id
									)}<td class="h-24 border p-1 align-top"
										>{#each cellBlocks as block (block.id)}<div
												class={`mb-1 flex min-h-20 flex-col rounded-md border p-2 text-xs ${blockColor(block.blockKind)}`}
											>
												<p class="truncate font-semibold">{blockTitle(block)}</p>
												{#if block.offeringName}<p class="mt-1 line-clamp-2 opacity-80">
														{block.offeringName}
													</p>{/if}{#if groupLabel(block)}<p
														class="mt-auto flex items-center gap-1 truncate opacity-70"
													>
														<School class="size-3" />
														{groupLabel(block)}
													</p>{/if}{#if roomLabel(block)}<p
														class="flex items-center gap-1 truncate opacity-70"
													>
														<MapPin class="size-3" />
														{roomLabel(block)}
													</p>{/if}
											</div>{/each}</td
									>{/each}</tr
							>{/each}</tbody
					>
				</table>
			</div>
		{/if}
	</div>
</PageShell>
