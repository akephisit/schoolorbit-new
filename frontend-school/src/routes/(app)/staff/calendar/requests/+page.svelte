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
	import { LoadingButton, PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import CalendarEventDialog from '#lib/components/calendar/CalendarEventDialog.svelte';
	import {
		listCalendarRequests,
		listCalendarCategories,
		listCalendarTags,
		listCalendarTargetOptions,
		approveCalendarRequest,
		rejectCalendarRequest,
		type CalendarEventRequest,
		type CalendarTargetOptions,
		type CalendarCategory,
		type CalendarTag,
		type CreateCalendarEventRequest
	} from '#lib/api/calendar.js';
	import { formatCalendarDate } from '#lib/utils/calendar.js';
	import { toast } from 'svelte-sonner';
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
	const listRequest = new LatestRequest(),
		optionRequest = new LatestRequest();
	let reviewing = $state<CalendarEventRequest | null>(null),
		rejecting = $state<CalendarEventRequest | null>(null);
	let reviewOpen = $state(false),
		rejectOpen = $state(false),
		session = $state(0),
		saving = $state(false),
		decisionError = $state(''),
		reason = $state('');
	let catalogs = $state.raw<{ categories: CalendarCategory[]; tags: CalendarTag[] }>({
		categories: [],
		tags: []
	});
	let targetOptions = $state.raw<CalendarTargetOptions>({ gradeLevels: [], homerooms: [] });
	let optionsDate = $state(''),
		optionsLoading = $state(false),
		optionsLoaded = $state(false),
		optionsError = $state('');
	const statusOptions = [
		{ value: 'all', label: 'ทุกสถานะ' },
		{ value: 'pending', label: 'รออนุมัติ' },
		{ value: 'approved', label: 'อนุมัติแล้ว' },
		{ value: 'rejected', label: 'ไม่อนุมัติ' }
	];
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
		records = result.data.page.records;
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
				optionRequest.abort();
				records = [];
				loaded = false;
				loading = canRead;
				error = '';
				reviewOpen = false;
				rejectOpen = false;
				reviewing = null;
				rejecting = null;
				saving = false;
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
		optionRequest.abort();
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
	async function loadOptions() {
		if (!reviewOpen || !manager || !optionsDate) return;
		const key = ownerKey,
			activeSession = session,
			ticket = optionRequest.begin();
		optionsLoading = true;
		optionsError = '';
		try {
			const [categories, tags, targets] = await Promise.all([
				listCalendarCategories({ signal: ticket.signal }),
				listCalendarTags({ signal: ticket.signal }),
				listCalendarTargetOptions(optionsDate, { signal: ticket.signal })
			]);
			if (
				!current(key) ||
				activeSession !== session ||
				!reviewOpen ||
				!optionRequest.isCurrent(ticket.revision)
			)
				return;
			catalogs = { categories, tags };
			targetOptions = targets;
			optionsLoaded = true;
		} catch (error: unknown) {
			if (
				current(key) &&
				activeSession === session &&
				reviewOpen &&
				optionRequest.isCurrent(ticket.revision)
			)
				optionsError = error instanceof Error ? error.message : 'โหลดตัวเลือกไม่สำเร็จ';
		} finally {
			if (current(key) && activeSession === session && optionRequest.isCurrent(ticket.revision))
				optionsLoading = false;
		}
	}
	function review(request: CalendarEventRequest) {
		if (!manager || request.status !== 'pending') return;
		reviewing = request;
		optionsDate = request.startDate;
		optionsLoaded = false;
		optionsError = '';
		decisionError = '';
		session++;
		reviewOpen = true;
		void loadOptions();
	}
	function changeDate(date: string) {
		if (date === optionsDate) return;
		optionsDate = date;
		optionsLoaded = false;
		void loadOptions();
	}
	function patch(request: CalendarEventRequest) {
		records = records
			.map((item) => (item.id === request.id ? request : item))
			.filter((item) => !data.query.status || item.status === data.query.status);
	}
	async function decide(payload: CreateCalendarEventRequest | null) {
		const target = payload ? reviewing : rejecting;
		if (
			!target ||
			!manager ||
			saving ||
			(payload && (!optionsLoaded || optionsLoading || optionsError))
		)
			return;
		if (!payload && !reason.trim()) {
			decisionError = 'กรุณาระบุเหตุผลที่ไม่อนุมัติ';
			return;
		}
		const key = ownerKey,
			activeSession = session;
		const owns = () =>
			current(key) && manager && activeSession === session && (payload ? reviewOpen : rejectOpen);
		saving = true;
		decisionError = '';
		listRequest.abort();
		loading = false;
		try {
			const result = payload
				? (await approveCalendarRequest(target.id, payload)).request
				: await rejectCalendarRequest(target.id, reason.trim());
			if (!current(key) || !manager) return;
			listRequest.abort();
			loading = false;
			patch(result);
			if (!owns()) return;
			reviewOpen = false;
			rejectOpen = false;
			toast.success(payload ? 'อนุมัติแล้ว กิจกรรมขึ้นปฏิทินแล้ว' : 'บันทึกผลไม่อนุมัติแล้ว');
		} catch (error: unknown) {
			if (owns()) {
				decisionError = error instanceof Error ? error.message : 'บันทึกผลไม่สำเร็จ';
				toast.error(decisionError);
			}
		} finally {
			if (current(key) && activeSession === session) saving = false;
		}
	}
</script>

<PageShell title="คำร้องเพิ่มกิจกรรม" description="ติดตามคำร้องและผลการพิจารณา">
	{#snippet actions()}<Button variant="outline" href={resolve('staff/calendar')}>กลับปฏิทิน</Button
		>{/snippet}
	<div class="flex flex-wrap items-center gap-3 rounded-xl border bg-card p-3 sm:p-4">
		{#if manager}<div class="flex flex-wrap gap-2">
				<Button
					variant={!data.query.review ? 'default' : 'outline'}
					onclick={() => navigate({ review: '', offset: '' })}>คำร้องของฉัน</Button
				><Button
					variant={data.query.review ? 'default' : 'outline'}
					onclick={() => navigate({ review: 'true', offset: '' })}>คิวอนุมัติทั้งหมด</Button
				>
			</div>{/if}
		<Select.Root
			type="single"
			value={data.query.status ?? 'all'}
			onValueChange={(value) => navigate({ status: value === 'all' ? '' : value, offset: '' })}
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
									>{statusOptions.find((option) => option.value === request.status)?.label}</Badge
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
									<Button size="sm" onclick={() => review(request)}>ตรวจและอนุมัติ</Button><Button
										variant="outline"
										size="sm"
										onclick={() => {
											session++;
											reason = '';
											decisionError = '';
											rejecting = request;
											rejectOpen = true;
										}}>ไม่อนุมัติ</Button
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
						onclick={() => navigate({ offset: String((data.query.offset ?? 0) + 25) })}
						>ถัดไป</Button
					>
				</div>
			{/if}
		</section>
	{/if}
	{#if reviewOpen && reviewing && manager}{#key session}<CalendarEventDialog
				bind:open={reviewOpen}
				request={reviewing}
				categories={catalogs.categories}
				tags={catalogs.tags}
				gradeLevels={targetOptions.gradeLevels}
				homerooms={targetOptions.homerooms}
				{saving}
				{optionsLoading}
				{optionsLoaded}
				{optionsError}
				error={decisionError}
				onretryoptions={loadOptions}
				ondatechange={changeDate}
				onsave={(payload) => void decide(payload)}
				submitLabel="อนุมัติและเพิ่มลงปฏิทิน"
			/>{/key}{/if}
	<Dialog.Root bind:open={rejectOpen}
		><Dialog.Content
			><Dialog.Header
				><Dialog.Title>ไม่อนุมัติคำร้อง</Dialog.Title><Dialog.Description
					>{rejecting?.title}</Dialog.Description
				></Dialog.Header
			>
			<form
				class="space-y-4"
				onsubmit={(event) => {
					event.preventDefault();
					void decide(null);
				}}
			>
				<div class="space-y-2">
					<Label for="rejection-reason">เหตุผลที่ไม่อนุมัติ *</Label><Textarea
						id="rejection-reason"
						bind:value={reason}
						required
						maxlength={2000}
						disabled={saving}
					/>
				</div>
				{#if decisionError}<p role="alert" class="text-sm text-destructive">
						{decisionError}
					</p>{/if}<Dialog.Footer
					><Button
						variant="outline"
						type="button"
						disabled={saving}
						onclick={() => (rejectOpen = false)}>ยกเลิก</Button
					><LoadingButton type="submit" loading={saving}>บันทึกผลไม่อนุมัติ</LoadingButton
					></Dialog.Footer
				>
			</form></Dialog.Content
		></Dialog.Root
	>
</PageShell>
