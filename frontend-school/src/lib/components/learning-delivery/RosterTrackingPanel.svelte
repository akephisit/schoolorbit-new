<script lang="ts">
	import { untrack, onDestroy } from 'svelte';
	import {
		getLearningGroupRosterTracking,
		updateLearningGroupRosterTracking,
		type LearningGroup,
		type LearningGroupRosterTracking
	} from '#lib/api/learning-delivery.js';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import type { RouteLoadResult } from '#lib/navigation/route-load.js';
	import {
		LoadingButton,
		PageSkeleton,
		PageState,
		RegionUpdatingState
	} from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { DatePicker } from '#lib/components/ui/date-picker/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import { formatCalendarDate } from '#lib/utils/calendar.js';

	let {
		group,
		canManage,
		tracking = $bindable(null),
		initialTracking = null,
		onUpdated
	}: {
		group: LearningGroup;
		canManage: boolean;
		tracking: LearningGroupRosterTracking | null;
		initialTracking?: Promise<RouteLoadResult<LearningGroupRosterTracking | null>> | null;
		onUpdated: (groupId: string, config: LearningGroupRosterTracking) => void;
	} = $props();
	let loading = $state(true),
		saving = $state(false),
		error = $state(''),
		saved = $state('');
	let mode = $state<'manual' | 'homeroom'>('manual');
	let effectiveFrom = $state<string | undefined>();
	const request = new LatestRequest();
	let disposed = false;
	onDestroy(() => {
		disposed = true;
		request.abort();
	});
	let lastKey = '';
	const unchanged = $derived(
		mode === tracking?.mode && (mode === 'manual' || effectiveFrom === tracking?.effectiveFrom)
	);
	const editable = $derived(
		canManage &&
			group.status !== 'closed' &&
			group.status !== 'cancelled' &&
			group.rosterStatus !== 'closed'
	);
	function apply(config: LearningGroupRosterTracking | null, preserveDraft = false) {
		if (preserveDraft && tracking && !unchanged) {
			if (config?.mode === tracking.mode && config.effectiveFrom === tracking.effectiveFrom)
				tracking = config;
			else error = 'วิธีจัดรายชื่อเปลี่ยนระหว่างแก้ไข กรุณาโหลดวิธีล่าสุดก่อนบันทึก';
			return;
		}
		tracking = config;
		mode = config?.mode ?? 'manual';
		effectiveFrom = config?.effectiveFrom ?? undefined;
	}
	async function retry(preserveDraft = false) {
		const ticket = request.begin();
		loading = true;
		error = '';
		try {
			const config = await getLearningGroupRosterTracking(group.id, { signal: ticket.signal });
			if (request.isCurrent(ticket.revision)) apply(config, preserveDraft);
		} catch (failure) {
			if (!isAbortError(failure) && request.isCurrent(ticket.revision))
				error = failure instanceof Error ? failure.message : 'โหลดวิธีจัดรายชื่อไม่สำเร็จ';
		} finally {
			if (request.isCurrent(ticket.revision)) loading = false;
		}
	}
	$effect.pre(() => {
		const key = `${group.id}:${group.rowVersion}`;
		if (key === lastKey) return;
		const previousKey = lastKey;
		const alreadyPatched = untrack(
			() => previousKey.startsWith(`${group.id}:`) && tracking?.groupRowVersion === group.rowVersion
		);
		lastKey = key;
		if (alreadyPatched) return;
		const initial = untrack(() => (previousKey ? null : initialTracking));
		const ticket = request.begin();
		untrack(() => {
			if (!previousKey) tracking = null;
			loading = true;
			error = '';
		});
		if (initial)
			void initial.then((result) => {
				if (!request.isCurrent(ticket.revision)) return;
				untrack(() => {
					if (result.ok) apply(result.data);
					else error = result.error;
					loading = false;
				});
			});
		else untrack(() => void retry(Boolean(previousKey)));
		return () => request.abort();
	});
	function changeMode(value: string) {
		mode = value === 'homeroom' ? 'homeroom' : 'manual';
		if (mode === 'homeroom' && !effectiveFrom && tracking) {
			const today = new Intl.DateTimeFormat('en-CA', {
				timeZone: 'Asia/Bangkok',
				year: 'numeric',
				month: '2-digit',
				day: '2-digit'
			}).format(new Date());
			effectiveFrom = today > tracking.startsOn ? today : tracking.startsOn;
		}
		saved = '';
	}
	async function save(event: SubmitEvent) {
		event.preventDefault();
		if (!editable || !tracking || saving) return;
		if (mode === 'homeroom' && !effectiveFrom) {
			error = 'เลือกวันที่เริ่มติดตาม';
			return;
		}
		const id = group.id;
		saving = true;
		error = '';
		saved = '';
		try {
			const config = await updateLearningGroupRosterTracking(id, {
				mode,
				effectiveFrom: mode === 'homeroom' ? effectiveFrom : null,
				rowVersion: tracking.groupRowVersion
			});
			if (disposed || group.id !== id) return;
			apply(config);
			onUpdated(id, config);
			saved = 'บันทึกวิธีจัดรายชื่อแล้ว';
		} catch (failure) {
			error = failure instanceof Error ? failure.message : 'บันทึกวิธีจัดรายชื่อไม่สำเร็จ';
		} finally {
			saving = false;
		}
	}
</script>

<section
	class="relative rounded-xl border bg-card p-4"
	data-testid="roster-tracking"
	aria-busy={loading || saving}
>
	<h3 class="font-semibold">วิธีจัดรายชื่อกลุ่มวิชา</h3>
	{#if loading && !tracking}<PageSkeleton variant="form" />
	{:else if !tracking}<PageState
			variant="error"
			title="โหลดวิธีจัดรายชื่อไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={retry}
		/>
	{:else}
		{#if loading}<RegionUpdatingState label="กำลังอัปเดตวิธีจัดรายชื่อ" />{/if}
		<p class="mt-2 text-sm text-muted-foreground">
			{tracking.mode === 'homeroom'
				? `ติดตามนักเรียนจากห้องต้นทาง ${group.homeroomIds.length} ห้อง เริ่ม ${formatCalendarDate(tracking.effectiveFrom ?? tracking.startsOn)} การเพิ่ม ย้าย และนำออกจากห้องจะปรับรายชื่อกลุ่มนี้พร้อมกัน`
				: 'จัดรายชื่อเอง: เพิ่มและกำหนดวันสิ้นสุดเป็นรายคน การเปลี่ยนห้องจะไม่เปลี่ยนรายชื่อกลุ่มนี้'}
		</p>
		{#if editable}
			<form
				class="mt-4 grid gap-3 sm:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_auto] sm:items-end"
				onsubmit={save}
			>
				<div class="space-y-2">
					<Label for={`tracking-mode-${group.id}`}>วิธีจัดรายชื่อ</Label><Select.Root
						type="single"
						value={mode}
						onValueChange={changeMode}
						disabled={saving}
						><Select.Trigger id={`tracking-mode-${group.id}`} class="w-full"
							>{mode === 'homeroom' ? 'ติดตามห้องอัตโนมัติ' : 'จัดรายชื่อเอง'}</Select.Trigger
						><Select.Content
							><Select.Item value="manual">จัดรายชื่อเอง</Select.Item><Select.Item
								value="homeroom"
								disabled={group.homeroomIds.length === 0}>ติดตามห้องอัตโนมัติ</Select.Item
							></Select.Content
						></Select.Root
					>
				</div>
				{#if mode === 'homeroom'}<div class="space-y-2">
						<Label for={`tracking-date-${group.id}`}>วันที่เริ่มติดตาม</Label><DatePicker
							id={`tracking-date-${group.id}`}
							bind:value={effectiveFrom}
							disabled={saving}
							required
							ariaLabel="วันที่เริ่มติดตาม"
						/>
					</div>{/if}
				<LoadingButton
					type="submit"
					loading={saving}
					loadingLabel="กำลังบันทึก"
					disabled={loading ||
						unchanged ||
						(mode === 'homeroom' && (!effectiveFrom || !group.homeroomIds.length))}
					>บันทึกวิธีจัดรายชื่อ</LoadingButton
				>
			</form>
			<p class="mt-3 text-xs text-muted-foreground">
				การเปิดติดตามจะเพิ่มนักเรียนที่ขาดจากห้องต้นทาง โดยเก็บประวัติและคะแนนเดิมไว้
				เริ่มติดตามได้ตั้งแต่วันนี้ ภายใน {formatCalendarDate(tracking.startsOn)} – {formatCalendarDate(
					tracking.endsOn
				)}{group.rosterStatus !== 'published'
					? ' และเริ่มปรับรายชื่ออัตโนมัติหลังเผยแพร่รายชื่อกลุ่ม'
					: ''}
			</p>
			{#if !group.homeroomIds.length}<p class="mt-2 text-sm text-muted-foreground">
					กำหนดห้องต้นทางในกลุ่มเรียนก่อนเปิดติดตามอัตโนมัติ
				</p>{/if}
		{/if}
		{#if error}<p role="alert" class="mt-3 text-sm text-destructive">{error}</p>
			<Button size="sm" variant="outline" class="mt-2" onclick={() => retry()}
				>โหลดวิธีจัดรายชื่อล่าสุด</Button
			>{/if}
		{#if saved}<p role="status" class="mt-3 text-sm text-muted-foreground">{saved}</p>{/if}
	{/if}
</section>
