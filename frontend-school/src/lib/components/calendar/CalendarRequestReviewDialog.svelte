<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { get } from 'svelte/store';
	import { can } from '#lib/stores/permissions.js';
	import { authStore } from '#lib/stores/auth.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { LoadingButton, PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import CalendarEventDialog from './CalendarEventDialog.svelte';
	import {
		getCalendarRequestForReview,
		listCalendarCategories,
		listCalendarTags,
		listCalendarTargetOptions,
		approveCalendarRequest,
		rejectCalendarRequest,
		type CalendarEventRequest,
		type CalendarEvent,
		type CalendarTargetOptions,
		type CalendarCategory,
		type CalendarTag,
		type CreateCalendarEventRequest
	} from '#lib/api/calendar.js';
	import { toast } from 'svelte-sonner';

	let {
		open = $bindable(true),
		target,
		mode,
		initialRequest,
		initialCatalogs,
		ondecided
	}: {
		open?: boolean;
		target: Pick<CalendarEventRequest, 'id' | 'title' | 'startDate'>;
		mode: 'approve' | 'reject';
		initialRequest?: CalendarEventRequest;
		initialCatalogs?: { categories: CalendarCategory[]; tags: CalendarTag[] };
		ondecided: (request: CalendarEventRequest, event?: CalendarEvent) => void;
	} = $props();
	const authorized = $derived(
		$authStore.user?.user_type === 'staff' &&
			$can.hasAll(PERMISSIONS.CALENDAR_READ_SCHOOL, PERMISSIONS.CALENDAR_MANAGE_SCHOOL)
	);
	const identity = $derived.by(() => {
		void $authStore;
		void $can;
		return appIdentityKey();
	});
	let request = $state.raw<CalendarEventRequest | null>(null);
	let catalogs = $state.raw<{ categories: CalendarCategory[]; tags: CalendarTag[] }>({
		categories: [],
		tags: []
	});
	let targetOptions = $state.raw<CalendarTargetOptions>({ gradeLevels: [], homerooms: [] });
	let optionsDate = $state(''),
		optionsLoading = $state(false),
		optionsLoaded = $state(false),
		optionsError = $state('');
	let detailLoading = $state(false),
		detailError = $state(''),
		saving = $state(false),
		decisionError = $state(''),
		reason = $state('');
	let catalogsLoaded = false;
	let owner = '',
		disposed = false;
	const detailRead = new LatestRequest(),
		optionsRead = new LatestRequest();

	function current(key: string) {
		return !disposed && open && authorized && key === identity;
	}
	async function loadDetail() {
		if (!open || !authorized) return;
		const key = identity,
			ticket = detailRead.begin();
		detailLoading = true;
		detailError = '';
		try {
			const result =
				initialRequest ?? (await getCalendarRequestForReview(target.id, { signal: ticket.signal }));
			if (!current(key) || !detailRead.isCurrent(ticket.revision)) return;
			request = result;
			if (optionsDate !== result.startDate) changeDate(result.startDate);
		} catch (error: unknown) {
			if (current(key) && detailRead.isCurrent(ticket.revision))
				detailError = error instanceof Error ? error.message : 'โหลดรายละเอียดคำร้องไม่สำเร็จ';
		} finally {
			if (current(key) && detailRead.isCurrent(ticket.revision)) detailLoading = false;
		}
	}
	async function loadOptions() {
		if (!open || !authorized || mode !== 'approve' || !optionsDate) return;
		const key = identity,
			ticket = optionsRead.begin();
		optionsLoading = true;
		optionsError = '';
		try {
			const [categories, tags, targets] = await Promise.all([
				initialCatalogs || catalogsLoaded
					? Promise.resolve(initialCatalogs?.categories ?? catalogs.categories)
					: listCalendarCategories({ signal: ticket.signal }),
				initialCatalogs || catalogsLoaded
					? Promise.resolve(initialCatalogs?.tags ?? catalogs.tags)
					: listCalendarTags({ signal: ticket.signal }),
				listCalendarTargetOptions(optionsDate, { signal: ticket.signal })
			]);
			if (!current(key) || !optionsRead.isCurrent(ticket.revision)) return;
			catalogs = { categories, tags };
			catalogsLoaded = true;
			targetOptions = targets;
			optionsLoaded = true;
		} catch (error: unknown) {
			if (current(key) && optionsRead.isCurrent(ticket.revision))
				optionsError = error instanceof Error ? error.message : 'โหลดตัวเลือกไม่สำเร็จ';
		} finally {
			if (current(key) && optionsRead.isCurrent(ticket.revision)) optionsLoading = false;
		}
	}
	function changeDate(date: string) {
		if (date === optionsDate) return;
		optionsDate = date;
		optionsLoaded = false;
		void loadOptions();
	}
	$effect.pre(() => {
		const key = open && authorized ? `${identity}|${target.id}|${mode}` : '';
		untrack(() => {
			if (owner === key) return;
			owner = key;
			detailRead.abort();
			optionsRead.abort();
			if (!key) return;
			request = null;
			catalogsLoaded = false;
			detailError = '';
			decisionError = '';
			reason = '';
			optionsDate = target.startDate;
			optionsLoaded = false;
			optionsError = '';
			if (mode === 'approve') {
				void loadDetail();
				void loadOptions();
			}
		});
	});
	onDestroy(() => {
		disposed = true;
		detailRead.abort();
		optionsRead.abort();
	});

	async function decide(payload: CreateCalendarEventRequest | null) {
		if (
			!authorized ||
			!open ||
			saving ||
			(payload &&
				(!request ||
					request.status !== 'pending' ||
					!optionsLoaded ||
					optionsLoading ||
					optionsError))
		)
			return;
		if (!payload && !reason.trim()) {
			decisionError = 'กรุณาระบุเหตุผลที่ไม่อนุมัติ';
			return;
		}
		const key = identity;
		saving = true;
		decisionError = '';
		try {
			const result = payload
				? await approveCalendarRequest(target.id, payload)
				: { request: await rejectCalendarRequest(target.id, reason.trim()), event: undefined };
			if (
				key !== appIdentityKey() ||
				!get(can).hasAll(PERMISSIONS.CALENDAR_READ_SCHOOL, PERMISSIONS.CALENDAR_MANAGE_SCHOOL)
			)
				return;
			ondecided(result.request, result.event);
			if (!current(key)) return;
			open = false;
			toast.success(payload ? 'อนุมัติแล้ว กิจกรรมขึ้นปฏิทินแล้ว' : 'บันทึกผลไม่อนุมัติแล้ว');
		} catch (error: unknown) {
			if (current(key))
				decisionError = error instanceof Error ? error.message : 'บันทึกผลไม่สำเร็จ';
		} finally {
			if (!disposed && key === identity) saving = false;
		}
	}
</script>

{#if mode === 'approve' && request?.status === 'pending' && authorized}
	<CalendarEventDialog
		bind:open
		{request}
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
	/>
{:else}
	<Dialog.Root bind:open
		><Dialog.Content>
			<Dialog.Header
				><Dialog.Title
					>{mode === 'reject' ? 'ไม่อนุมัติคำร้อง' : 'ตรวจและอนุมัติคำร้อง'}</Dialog.Title
				><Dialog.Description>{target.title}</Dialog.Description></Dialog.Header
			>
			{#if !authorized}<PageState variant="permission" title="ไม่มีสิทธิ์ตรวจคำร้อง" />
			{:else if mode === 'approve'}
				{#if detailLoading}<div role="status" aria-label="กำลังโหลดรายละเอียดคำร้อง">
						<PageSkeleton variant="cards" />
					</div>
				{:else if detailError}<PageState
						variant="error"
						title="โหลดรายละเอียดคำร้องไม่สำเร็จ"
						description={detailError}
						actionLabel="ลองอีกครั้ง"
						onaction={loadDetail}
					/>
				{:else if request}<PageState
						title="คำร้องนี้ได้รับการพิจารณาแล้ว"
						description="กรุณารีเฟรชปฏิทินหรือคิวคำร้องเพื่อดูสถานะล่าสุด"
					/>{/if}
			{:else}
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
						</p>{/if}
					<Dialog.Footer
						><Button
							variant="outline"
							type="button"
							disabled={saving}
							onclick={() => (open = false)}>ยกเลิก</Button
						><LoadingButton type="submit" loading={saving}>บันทึกผลไม่อนุมัติ</LoadingButton
						></Dialog.Footer
					>
				</form>
			{/if}
		</Dialog.Content></Dialog.Root
	>
{/if}
