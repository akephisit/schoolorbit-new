<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { goto } from '$app/navigation';
	import { SvelteURL } from 'svelte/reactivity';
	import { page } from '$app/state';
	import {
		listCorrespondence,
		type CorrespondenceList,
		type CorrespondenceSummary,
		type CorrespondenceView,
		type CorrespondenceFilters
	} from '#lib/api/correspondence.js';
	import type { RouteLoadResult } from '#lib/navigation/route-load.js';
	import { CorrespondenceRegion } from '#lib/correspondence/region.svelte.js';
	import {
		actionLabels,
		registerLabels,
		statusLabels,
		dateLabel
	} from '#lib/correspondence/presentation.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	import Choice from './Choice.svelte';
	let {
		source,
		filters
	}: { source: Promise<RouteLoadResult<CorrespondenceList>>; filters: CorrespondenceFilters } =
		$props();
	const region = new CorrespondenceRegion<CorrespondenceList>();
	let search = $state(untrack(() => filters.search ?? ''));
	let register = $state(untrack(() => filters.register ?? 'all'));
	let status = $state(untrack(() => filters.status ?? 'all'));
	$effect.pre(() => {
		const operation = source;
		untrack(() => region.consume(operation));
	});
	onDestroy(() => region.dispose());
	function filter(event: SubmitEvent) {
		event.preventDefault();
		const url = new SvelteURL(page.url.href);
		url.search = '';
		url.searchParams.set('view', filters.view ?? 'action_required');
		if (search.trim()) url.searchParams.set('search', search.trim());
		if (register !== 'all') url.searchParams.set('register', register);
		if (status !== 'all') url.searchParams.set('status', status);
		void goto(url);
	}
	function nextHref(cursor: string): string {
		const url = new SvelteURL(page.url.href);
		url.searchParams.set('cursor', cursor);
		return `${url.pathname}${url.search}`;
	}
	const views = [
		{ value: 'action_required', label: 'รอฉันดำเนินการ', count: 'actionRequired' },
		{ value: 'sent', label: 'ฉันส่งติดตาม', count: 'sent' },
		{ value: 'drafts', label: 'ฉบับร่าง', count: 'drafts' },
		{ value: 'all', label: 'ทั้งหมด', count: 'all' }
	] as const;
	function viewHref(view: CorrespondenceView): string {
		const url = new SvelteURL(page.url.href);
		url.searchParams.set('view', view);
		url.searchParams.delete('cursor');
		return `${url.pathname}${url.search}`;
	}
	function detailHref(record: CorrespondenceSummary): string {
		return `/staff/correspondence/${record.id}?${new URLSearchParams({ returnTo: `${page.url.pathname}${page.url.search}` })}`;
	}
	function progressFor(record: CorrespondenceSummary) {
		return region.value?.progress.find((item) => item.correspondenceId === record.id);
	}
</script>

<PageShell title="ทะเบียนหนังสือ" description="รับ ส่ง และติดตามหนังสือกับผู้รับผิดชอบแต่ละขั้นตอน">
	{#snippet actions()}
		{#if $can.has(PERMISSIONS.CORRESPONDENCE_SIGN_OWN)}<Button
				href="/staff/correspondence/signatures"
				variant="outline">ลายมือชื่อของฉัน</Button
			>{/if}
		{#if $can.has(PERMISSIONS.PROCESS_MANAGE_SCHOOL)}<Button
				href="/staff/correspondence/settings"
				variant="outline">ตั้งค่าการสั่งการ</Button
			>
			<Button href="/staff/correspondence/templates" variant="outline">ตั้งค่าแม่แบบ</Button>{/if}
		{#if $can.has(PERMISSIONS.CORRESPONDENCE_CREATE_OWN)}<Button href="/staff/correspondence/new"
				>สร้างหนังสือ</Button
			>{/if}
	{/snippet}
	<nav class="mb-4 grid grid-cols-2 gap-2 sm:grid-cols-4" aria-label="มุมมองทะเบียนหนังสือ">
		{#each views as view (view.value)}<Button
				href={viewHref(view.value)}
				variant={(filters.view ?? 'action_required') === view.value ? 'secondary' : 'outline'}
				aria-current={(filters.view ?? 'action_required') === view.value ? 'page' : undefined}
				class="justify-between gap-2"
			>
				{view.label}<span class="tabular-nums">{region.value?.counts[view.count] ?? '—'}</span>
			</Button>{/each}
	</nav>
	{#if filters.search || filters.status || filters.register}<p
			class="mb-3 text-xs text-muted-foreground"
		>
			จำนวนหนังสือตามตัวกรองที่เลือก
		</p>{/if}
	<form
		class="grid gap-3 rounded-xl border bg-card p-3 sm:grid-cols-[2fr_1fr_1fr_auto] sm:items-end sm:p-4"
		onsubmit={filter}
	>
		<div class="space-y-2">
			<Label for="correspondence-search">ค้นหาเรื่อง / เลขทะเบียน / เลขที่ต้นทาง / ผู้ส่ง</Label
			><Input id="correspondence-search" bind:value={search} />
		</div>
		<Choice
			id="register-filter"
			label="ทะเบียน"
			bind:value={register}
			options={[
				{ value: 'all', label: 'ทุกทะเบียน' },
				...Object.entries(registerLabels).map(([value, label]) => ({ value, label }))
			]}
		/>
		<Choice
			id="status-filter"
			label="สถานะ"
			bind:value={status}
			options={[
				{ value: 'all', label: 'ทุกสถานะ' },
				...Object.entries(statusLabels).map(([value, label]) => ({ value, label }))
			]}
		/>
		<div class="flex flex-wrap gap-2">
			<Button type="submit" variant="outline">ค้นหา</Button
			>{#if filters.search || filters.status || filters.register}<Button
					href={`/staff/correspondence?${new URLSearchParams({ view: filters.view ?? 'action_required' })}`}
					variant="ghost">ล้างตัวกรอง</Button
				>{/if}
		</div>
	</form>
	<section aria-busy={region.loading} data-testid="correspondence-register">
		{#if region.error}<PageState
				variant="error"
				title={region.error}
				actionLabel="ลองอีกครั้ง"
				onaction={() => region.retry((signal) => listCorrespondence(filters, { signal }))}
			/>{/if}
		{#if region.loading && !region.value}<PageSkeleton />{:else if region.value?.items.length}
			<div class="space-y-3 md:hidden">
				{#each region.value.items as record (record.id)}{@const progress = progressFor(record)}
					<article class="space-y-3 rounded-xl border bg-card p-4">
						<div class="flex items-start justify-between gap-2">
							<a class="min-w-0 break-words font-medium text-primary" href={detailHref(record)}
								>{record.title}</a
							><Badge variant="outline">{statusLabels[record.status]}</Badge>
						</div>
						<p class="text-sm">
							{registerLabels[record.register]} · เลขทะเบียน {record.registerNumber}/{record.registerYear}
						</p>
						<p class="text-sm text-muted-foreground">
							เลขที่ต้นทาง {record.documentNumber ?? '—'} · {record.sender}
						</p>
						{#if record.urgency !== 'normal'}<p class="text-sm text-warning">
								{record.urgency === 'urgent' ? 'ด่วน' : 'ด่วนมาก'}
							</p>{/if}
						{@render progressSummary(record)}
						<Button
							href={detailHref(record)}
							variant={progress?.myAction || progress?.canRevise ? 'default' : 'outline'}
							class="w-full">เปิดอ่าน</Button
						>
					</article>{/each}
			</div>
			<div class="hidden overflow-x-auto rounded-xl border bg-card md:block">
				<Table.Root
					><Table.Header
						><Table.Row
							><Table.Head>เรื่อง / ผู้ส่ง</Table.Head><Table.Head
								>เลขทะเบียน / เลขที่ต้นทาง</Table.Head
							><Table.Head>สถานะ / ขั้นตอนปัจจุบัน</Table.Head><Table.Head
								>ผู้ที่รอ / กำหนดเสร็จ</Table.Head
							><Table.Head>ดำเนินการ</Table.Head></Table.Row
						></Table.Header
					><Table.Body>
						{#each region.value.items as record (record.id)}{@const progress =
								progressFor(record)}<Table.Row>
								<Table.Cell class="min-w-48"
									><a class="font-medium text-primary hover:underline" href={detailHref(record)}
										>{record.title}</a
									>
									<p class="mt-1 text-sm text-muted-foreground">{record.sender}</p>
									{#if record.urgency !== 'normal'}<p class="text-sm text-warning">
											{record.urgency === 'urgent' ? 'ด่วน' : 'ด่วนมาก'}
										</p>{/if}</Table.Cell
								>
								<Table.Cell class="min-w-40"
									><p>
										{registerLabels[record.register]}
										{record.registerNumber}/{record.registerYear}
									</p>
									<p class="mt-1 text-sm text-muted-foreground">
										ต้นทาง {record.documentNumber ?? '—'}
									</p></Table.Cell
								>
								<Table.Cell class="min-w-40"
									><Badge variant="outline">{statusLabels[record.status]}</Badge
									>{#if progress?.currentStepTitle}<p class="mt-2 text-sm">
											{#if progress.currentStepIndex != null}ขั้น {progress.currentStepIndex +
													1}/{progress.stepCount} ·
											{/if}{progress.currentStepTitle}
										</p>{/if}</Table.Cell
								>
								<Table.Cell class="min-w-44">{@render progressSummary(record)}</Table.Cell>
								<Table.Cell
									><Button
										href={detailHref(record)}
										size="sm"
										variant={progress?.myAction || progress?.canRevise ? 'default' : 'outline'}
										>เปิดอ่าน</Button
									></Table.Cell
								>
							</Table.Row>{/each}
					</Table.Body></Table.Root
				>
			</div>

			{#if region.value.nextCursor}<div class="mt-4 flex justify-end">
					<Button href={nextHref(region.value.nextCursor)} variant="outline">หน้าถัดไป</Button>
				</div>{/if}
		{:else if !region.error}<PageState
				title={filters.search || filters.status || filters.register
					? 'ไม่พบหนังสือตามเงื่อนไข'
					: filters.view === 'action_required'
						? 'ไม่มีหนังสือที่รอคุณดำเนินการ'
						: 'ยังไม่มีหนังสือในมุมมองนี้'}
				description="หนังสือที่คุณสร้างหรือได้รับมอบหมายจะแสดงที่นี่"
				actionLabel="ดูหนังสือทั้งหมด"
				onaction={() => {
					void goto(viewHref('all'));
				}}
			/>{/if}
	</section>
</PageShell>

{#snippet progressSummary(record: CorrespondenceSummary)}{@const progress = progressFor(record)}
	<div class="space-y-1 text-sm">
		{#if progress?.myAction}<p class="font-medium text-primary">
				รอคุณ: {actionLabels[progress.myAction]}
			</p>{/if}
		{#if progress?.canRevise}<p class="font-medium text-primary">
				{record.status === 'revision_requested' ? 'แก้ไขและส่งใหม่' : 'เขียนต่อและส่ง'}
			</p>{/if}
		{#if progress?.currentStepTitle}<p class="md:hidden">
				{#if progress.currentStepIndex != null}ขั้น {progress.currentStepIndex +
						1}/{progress.stepCount} ·
				{/if}{progress.currentStepTitle}
			</p>{/if}
		<p class="break-words">
			{progress?.pendingRecipients.length
				? `รอ ${progress.pendingRecipients.map((item) => item.displayName).join(', ')}`
				: record.status === 'draft'
					? 'ยังไม่ได้ส่ง'
					: record.status === 'revision_requested'
						? 'รอเจ้าของเรื่องแก้ไข'
						: 'ไม่มีผู้รับที่รอดำเนินการ'}
		</p>
		{#if progress?.dueAt}<p class="text-muted-foreground">กำหนด {dateLabel(progress.dueAt)}</p>{/if}
	</div>
{/snippet}
