<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { onDestroy, untrack } from 'svelte';
	import type { PageProps } from './$types';
	import { authStore } from '#lib/stores/auth.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { captureRouteLoad } from '#lib/navigation/route-load.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import CalendarRequestReviewDialog from '#lib/components/calendar/CalendarRequestReviewDialog.svelte';
	import { listCalendarRequests, type CalendarEventRequest } from '#lib/api/calendar.js';
	import { formatCalendarDate } from '#lib/utils/calendar.js';
	let { data }: PageProps = $props();
	const identity = $derived.by(() => {
		void $authStore;
		void $can;
		return appIdentityKey();
	});
	const ownerKey = $derived(`${identity}|${data.requestKey}`);
	const manager = $derived($can.has(PERMISSIONS.CALENDAR_MANAGE_SCHOOL));
	const allowed = $derived(
		$authStore.user?.user_type === 'staff' &&
			$can.has(PERMISSIONS.CALENDAR_READ_SCHOOL) &&
			(data.query.review ? manager : $can.has(PERMISSIONS.CALENDAR_REQUEST_OWN) || manager)
	);
	let records = $state.raw<CalendarEventRequest[]>([]),
		hasMore = $state(false),
		loading = $state(true),
		loaded = $state(false),
		error = $state('');
	let owner = '',
		disposed = false,
		consumed: typeof data.requests | null = null;
	const listRequest = new LatestRequest();
	let reviewTarget = $state<CalendarEventRequest | null>(null),
		reviewOpen = $state(false),
		reviewMode = $state<'approve' | 'reject'>('approve'),
		session = $state(0);
	const allStatusOptions = [
		{ value: 'all', label: 'ทุกสถานะ' },
		{ value: 'pending', label: 'รออนุมัติ' },
		{ value: 'approved', label: 'อนุมัติแล้ว' },
		{ value: 'rejected', label: 'ไม่อนุมัติ' }
	];
	const statusOptions = $derived(
		data.query.review
			? [
					{ value: 'all', label: 'ยังไม่อนุมัติทั้งหมด' },
					...allStatusOptions.filter((item) => item.value !== 'all' && item.value !== 'approved')
				]
			: allStatusOptions
	);
	const statusLabel = $derived(
		statusOptions.find((item) => item.value === (data.query.status ?? 'all'))?.label ?? 'ทุกสถานะ'
	);
	function current(key: string) {
		return !disposed && allowed && key === ownerKey;
	}
	function apply(result: Awaited<typeof data.requests>, revision: number, key: string) {
		if (!current(key) || !listRequest.isCurrent(revision)) return;
		loading = false;
		if (!result.ok) {
			error = result.error;
			return;
		}
		if (result.data.ownerKey !== key) return;
		if (!result.data.page) {
			error = 'ไม่มีสิทธิ์ดูคำร้อง';
			return;
		}
		records = result.data.page.records.filter(matchesQueue);
		hasMore = result.data.page.hasMore;
		loaded = true;
		error = '';
	}
	$effect.pre(() => {
		const key = ownerKey,
			source = data.requests,
			canRead = allowed;
		untrack(() => {
			if (owner !== key || !canRead) {
				owner = key;
				listRequest.abort();
				records = [];
				loaded = false;
				loading = canRead;
				error = '';
				reviewOpen = false;
				reviewTarget = null;
				session++;
			}
			if (!canRead) return;
			if (source !== consumed) {
				consumed = source;
				const ticket = listRequest.begin();
				loading = true;
				void source.then((result) => apply(result, ticket.revision, key));
			}
		});
	});
	onDestroy(() => {
		disposed = true;
		listRequest.abort();
	});
	async function reload() {
		if (!allowed) return;
		const key = ownerKey,
			ticket = listRequest.begin();
		loading = true;
		error = '';
		apply(
			await captureRouteLoad(
				listCalendarRequests(data.query, { signal: ticket.signal }).then((page) => ({
					ownerKey: key,
					page
				})),
				'โหลดคำร้องไม่สำเร็จ'
			),
			ticket.revision,
			key
		);
	}
	function navigate(values: Record<string, string>) {
		const url = new URL(page.url.href);
		for (const [key, value] of Object.entries(values)) {
			if (value) url.searchParams.set(key, value);
			else url.searchParams.delete(key);
		}
		void goto(resolve('staff/calendar/requests') + url.search, { reset: false });
	}
	function matchesQueue(request: CalendarEventRequest) {
		return (
			(!data.query.review || request.status !== 'approved') &&
			(!data.query.status || request.status === data.query.status)
		);
	}
	function openDecision(request: CalendarEventRequest, mode: 'approve' | 'reject') {
		if (!allowed || !manager || request.status !== 'pending') return;
		reviewTarget = request;
		reviewMode = mode;
		session++;
		reviewOpen = true;
	}
	function patch(request: CalendarEventRequest, key: string) {
		if (!current(key) || !manager) return;
		listRequest.abort();
		loading = false;
		records = records.map((item) => (item.id === request.id ? request : item)).filter(matchesQueue);
	}
</script>

<PageShell title="คำร้องเพิ่มกิจกรรม" description="ติดตามคำร้องและผลการพิจารณา">
	{#snippet actions()}<Button variant="outline" href={resolve('staff/calendar')}>กลับปฏิทิน</Button
		>{/snippet}
	<div class="flex flex-wrap items-center gap-3 rounded-xl border bg-card p-3 sm:p-4">
		{#if manager}<div class="flex flex-wrap gap-2">
				<Button
					variant={!data.query.review ? 'default' : 'outline'}
					onclick={() => navigate({ review: '', status: '', offset: '' })}>คำร้องของฉัน</Button
				><Button
					variant={data.query.review ? 'default' : 'outline'}
					onclick={() => navigate({ review: 'true', status: 'pending', offset: '' })}
					>คิวอนุมัติทั้งหมด</Button
				>
			</div>{/if}
		<Select.Root
			type="single"
			value={data.query.status ?? 'all'}
			onValueChange={(value) =>
				navigate({ status: value === 'all' && !data.query.review ? '' : value, offset: '' })}
		>
			<Select.Trigger class="w-40" aria-label="สถานะคำร้อง">{statusLabel}</Select.Trigger
			><Select.Content
				>{#each statusOptions as option (option.value)}<Select.Item value={option.value}
						>{option.label}</Select.Item
					>{/each}</Select.Content
			>
		</Select.Root>
		<Button variant="ghost" onclick={reload} disabled={loading}>รีเฟรช</Button>
	</div>
	{#if data.query.review && manager}<p class="text-sm text-muted-foreground">
			เรียงตามเวลาส่งคำร้อง: ขอก่อนอยู่บน ขอทีหลังอยู่ล่าง
		</p>{/if}
	{#if !allowed}<PageState variant="permission" title="ไม่มีสิทธิ์ดูคำร้องนี้" />
	{:else}
		<section aria-busy={loading} data-testid="calendar-requests">
			{#if error}<PageState
					variant="error"
					title="โหลดคำร้องไม่สำเร็จ"
					description={error}
					actionLabel="ลองอีกครั้ง"
					onaction={reload}
				/>{/if}
			{#if loading && !loaded}<PageSkeleton variant="cards" />
			{:else if loaded}
				{#if loading}<p role="status" class="text-sm text-muted-foreground">
						กำลังอัปเดตคำร้อง...
					</p>{/if}
				{#if records.length === 0}<PageState
						title="ยังไม่มีคำร้องในรายการนี้"
						description="คำร้องที่ส่งจะปรากฏที่นี่ พร้อมสถานะการพิจารณา"
					/>{/if}
				<div class="space-y-3">
					{#each records as request (request.id)}
						<article class="space-y-3 rounded-xl border bg-card p-4">
							<div class="flex flex-wrap items-start justify-between gap-3">
								<div class="min-w-0">
									<h2 class="break-words font-semibold">{request.title}</h2>
									<p class="text-sm text-muted-foreground">
										{request.requesterName} · {formatCalendarDate(
											request.startDate
										)}{request.endDate !== request.startDate
											? ` – ${formatCalendarDate(request.endDate)}`
											: ''}
									</p>
								</div>
								<Badge variant={request.status === 'pending' ? 'secondary' : 'outline'}
									>{allStatusOptions.find((option) => option.value === request.status)
										?.label}</Badge
								>
							</div>
							<p class="whitespace-pre-wrap break-words text-sm">{request.description}</p>
							<p class="text-sm text-muted-foreground">
								{request.allDay
									? 'ทั้งวัน'
									: `${request.startTime?.slice(0, 5)} – ${request.endTime?.slice(0, 5)}`}{request.location
									? ` · ${request.location}`
									: ''}
							</p>
							{#if request.rejectionReason}<p class="whitespace-pre-wrap break-words text-sm">
									<span class="font-medium">เหตุผลที่ไม่อนุมัติ:</span>
									{request.rejectionReason}
								</p>{/if}
							{#if request.eventId}<Button
									variant="outline"
									size="sm"
									href={resolve('staff/calendar')}>ดูปฏิทิน</Button
								>{/if}
							{#if data.query.review && manager && request.status === 'pending'}<div
									class="flex flex-wrap gap-2"
								>
									<Button size="sm" onclick={() => openDecision(request, 'approve')}
										>ตรวจและอนุมัติ</Button
									><Button
										variant="outline"
										size="sm"
										onclick={() => openDecision(request, 'reject')}>ไม่อนุมัติ</Button
									>
								</div>{/if}
						</article>
					{/each}
				</div>
				<div class="mt-4 flex justify-end gap-2">
					<Button
						variant="outline"
						disabled={!data.query.offset}
						onclick={() => navigate({ offset: String(Math.max(0, (data.query.offset ?? 0) - 25)) })}
						>ก่อนหน้า</Button
					><Button
						variant="outline"
						disabled={!hasMore}
						onclick={() =>
							records.length === 0
								? reload()
								: navigate({ offset: String((data.query.offset ?? 0) + records.length) })}
						>ถัดไป</Button
					>
				</div>
			{/if}
		</section>
	{/if}
	{#if reviewTarget && manager && allowed}
		{#key session}
			{@const decisionOwner = ownerKey}
			<CalendarRequestReviewDialog
				bind:open={reviewOpen}
				target={reviewTarget}
				mode={reviewMode}
				initialRequest={reviewTarget}
				ondecided={(request) => patch(request, decisionOwner)}
			/>
		{/key}
	{/if}
</PageShell>
