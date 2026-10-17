<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { goto } from '$app/navigation';
	import { SvelteURL } from 'svelte/reactivity';
	import { page } from '$app/state';
	import {
		listCorrespondence,
		type CorrespondenceList,
		type CorrespondenceFilters
	} from '#lib/api/correspondence.js';
	import type { RouteLoadResult } from '#lib/navigation/route-load.js';
	import { CorrespondenceRegion } from '#lib/correspondence/region.svelte.js';
	import { registerLabels, statusLabels, dateLabel } from '#lib/correspondence/presentation.js';
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
</script>

<PageShell title="ทะเบียนหนังสือ" description="รับ ส่ง และติดตามหนังสือกับผู้รับผิดชอบแต่ละขั้นตอน">
	{#snippet actions()}
		{#if $can.has(PERMISSIONS.CORRESPONDENCE_SIGN_OWN)}<Button
				href="/staff/correspondence/signatures"
				variant="outline">ลายมือชื่อของฉัน</Button
			>{/if}
		{#if $can.has(PERMISSIONS.PROCESS_MANAGE_SCHOOL)}<Button
				href="/staff/correspondence/templates"
				variant="outline">ตั้งค่าแม่แบบ</Button
			>{/if}
		{#if $can.has(PERMISSIONS.CORRESPONDENCE_CREATE_OWN)}<Button href="/staff/correspondence/new"
				>สร้างหนังสือ</Button
			>{/if}
	{/snippet}
	<form
		class="grid gap-3 rounded-xl border bg-card p-3 sm:grid-cols-[2fr_1fr_1fr_auto] sm:items-end sm:p-4"
		onsubmit={filter}
	>
		<div class="space-y-2">
			<Label for="correspondence-search">ค้นหาเรื่อง / เลขที่หนังสือ / ผู้ส่ง</Label><Input
				id="correspondence-search"
				bind:value={search}
			/>
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
		<Button type="submit" variant="outline">ค้นหา</Button>
	</form>
	<section aria-busy={region.loading} data-testid="correspondence-register">
		{#if region.error}<PageState
				variant="error"
				title={region.error}
				actionLabel="ลองอีกครั้ง"
				onaction={() => region.retry((signal) => listCorrespondence(filters, { signal }))}
			/>{/if}
		{#if region.loading && !region.value}<PageSkeleton />{:else if region.value?.items.length}
			<div class="overflow-x-auto rounded-xl border bg-card">
				<Table.Root
					><Table.Header
						><Table.Row
							><Table.Head>เรื่อง</Table.Head><Table.Head>ทะเบียน / เลขที่</Table.Head><Table.Head
								>ผู้ส่ง</Table.Head
							><Table.Head>สถานะ</Table.Head><Table.Head>ลงทะเบียน</Table.Head></Table.Row
						></Table.Header
					><Table.Body
						>{#each region.value.items as record (record.id)}<Table.Row
								><Table.Cell class="min-w-48"
									><a
										class="font-medium text-primary underline-offset-4 hover:underline"
										href={`/staff/correspondence/${record.id}`}>{record.title}</a
									>{#if record.urgency !== 'normal'}<p class="text-sm text-warning">
											{record.urgency === 'urgent' ? 'ด่วน' : 'ด่วนมาก'}
										</p>{/if}</Table.Cell
								><Table.Cell
									>{registerLabels[record.register]}
									<p class="text-muted-foreground">{record.documentNumber ?? '—'}</p></Table.Cell
								><Table.Cell class="min-w-36">{record.sender}</Table.Cell><Table.Cell
									><Badge variant="outline">{statusLabels[record.status]}</Badge></Table.Cell
								><Table.Cell class="whitespace-nowrap">{dateLabel(record.createdAt)}</Table.Cell
								></Table.Row
							>{/each}</Table.Body
					></Table.Root
				>
			</div>
			{#if region.value.nextCursor}<div class="mt-4 flex justify-end">
					<Button href={nextHref(region.value.nextCursor)} variant="outline">หน้าถัดไป</Button>
				</div>{/if}
		{:else if !region.error}<PageState
				title={filters.search || filters.status || filters.register
					? 'ไม่พบหนังสือตามเงื่อนไข'
					: 'ยังไม่มีหนังสือ'}
				description="หนังสือที่คุณสร้างหรือได้รับมอบหมายจะแสดงที่นี่"
			/>{/if}
	</section>
</PageShell>
