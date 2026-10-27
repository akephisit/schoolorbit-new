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
	import { LoadingButton, PageSkeleton, PageState } from '#lib/components/app-state/index.js';
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
	let accepted = $state.raw<LearningGroupRosterTracking | null>(null);
	let pendingConfig = $state.raw<LearningGroupRosterTracking | null>(null);
	let conflict = $state(false);
	let ownerId = '';
	let requestedKey = '';
	let observedTracking: LearningGroupRosterTracking | null = null;
	const unchanged = $derived(accepted !== null && draftMatches(accepted));
	const editable = $derived(
		canManage &&
			group.status !== 'closed' &&
			group.status !== 'cancelled' &&
			group.rosterStatus !== 'closed'
	);
	function draftMatches(config: LearningGroupRosterTracking): boolean {
		return mode === config.mode && (mode === 'manual' || effectiveFrom === config.effectiveFrom);
	}
	function sameFields(
		left: LearningGroupRosterTracking,
		right: LearningGroupRosterTracking
	): boolean {
		return (
			left.mode === right.mode &&
			(left.mode === 'manual' || left.effectiveFrom === right.effectiveFrom)
		);
	}
	// Every ingress shares the same revision fence and compares drafts with the last
	// accepted baseline, which is independent of the already-updated bindable prop.
	function reconcile(config: LearningGroupRosterTracking | null, preserveDraft: boolean): boolean {
		if (!config) {
			if (observedTracking) return false;
			tracking = null;
			loading = false;
			return true;
		}
		if (config.groupRowVersion < Math.max(group.rowVersion, observedTracking?.groupRowVersion ?? 0))
			return false;
		const dirty = accepted !== null && !draftMatches(accepted);
		const unchangedFields = accepted !== null && sameFields(accepted, config);
		const matchesDraft = draftMatches(config);
		request.abort();
		observedTracking = config;
		tracking = config;
		requestedKey = `${group.id}:${group.rowVersion}`;
		loading = false;
		saved = '';
		if (preserveDraft && dirty && !unchangedFields && !matchesDraft) {
			pendingConfig = config;
			conflict = true;
			error =
				'วิธีจัดรายชื่อเปลี่ยนระหว่างแก้ไข การแก้ไขของคุณยังอยู่ เลือกใช้วิธีล่าสุดก่อนบันทึก';
			return true;
		}
		accepted = { ...config };
		pendingConfig = null;
		conflict = false;
		error = '';
		if (!preserveDraft || !dirty || !unchangedFields || matchesDraft) {
			mode = config.mode;
			effectiveFrom = config.effectiveFrom ?? undefined;
		}
		return true;
	}
	async function retry(
		preserveDraft = false,
		initial: Promise<RouteLoadResult<LearningGroupRosterTracking | null>> | null = null
	) {
		const id = group.id;
		const ticket = request.begin();
		requestedKey = `${id}:${group.rowVersion}`;
		loading = true;
		error = '';
		try {
			let config: LearningGroupRosterTracking | null;
			if (initial) {
				const result = await initial;
				if (!result.ok) throw new Error(result.error);
				config = result.data;
			} else config = await getLearningGroupRosterTracking(id, { signal: ticket.signal });
			if (request.isCurrent(ticket.revision) && group.id === id) reconcile(config, preserveDraft);
		} catch (failure) {
			if (!isAbortError(failure) && request.isCurrent(ticket.revision) && group.id === id)
				error = failure instanceof Error ? failure.message : 'โหลดวิธีจัดรายชื่อไม่สำเร็จ';
		} finally {
			if (request.isCurrent(ticket.revision) && group.id === id) loading = false;
		}
	}
	$effect.pre(() => {
		const id = group.id;
		const rowVersion = group.rowVersion;
		const provided = tracking;
		untrack(() => {
			if (ownerId !== id) {
				request.abort();
				ownerId = id;
				requestedKey = '';
				observedTracking = null;
				accepted = null;
				pendingConfig = null;
				conflict = false;
				mode = 'manual';
				effectiveFrom = undefined;
				error = '';
				saved = '';
			}
			const key = `${id}:${rowVersion}`;
			if (provided && provided !== observedTracking) {
				if (observedTracking && provided.groupRowVersion < observedTracking.groupRowVersion) {
					tracking = observedTracking;
					return;
				}
				if (provided.groupRowVersion >= rowVersion) {
					reconcile(provided, true);
					return;
				}
			}
			if (provided && provided === observedTracking && provided.groupRowVersion >= rowVersion) {
				requestedKey = key;
				return;
			}
			if (key === requestedKey) {
				if (!provided && observedTracking) tracking = observedTracking;
				return;
			}
			const initial = requestedKey ? null : initialTracking;
			void retry(accepted !== null, initial);
		});
	});
	function resolveConflict() {
		if (pendingConfig) reconcile(pendingConfig, false);
	}

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
		if (!editable || !accepted || saving || loading || unchanged || conflict) return;
		if (mode === 'homeroom' && !effectiveFrom) {
			error = 'เลือกวันที่เริ่มติดตาม';
			return;
		}
		const id = group.id;
		const submittedRevision = accepted.groupRowVersion;
		saving = true;
		error = '';
		saved = '';
		try {
			const config = await updateLearningGroupRosterTracking(id, {
				mode,
				effectiveFrom: mode === 'homeroom' ? effectiveFrom : null,
				rowVersion: submittedRevision
			});
			if (disposed || group.id !== id) return;
			if (!reconcile(config, false)) return;
			onUpdated(id, config);
			saved = 'บันทึกวิธีจัดรายชื่อแล้ว';
		} catch (failure) {
			if (
				!disposed &&
				group.id === id &&
				(!observedTracking || observedTracking.groupRowVersion <= submittedRevision)
			)
				error = failure instanceof Error ? failure.message : 'บันทึกวิธีจัดรายชื่อไม่สำเร็จ';
		} finally {
			if (!disposed && group.id === id) saving = false;
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
			onaction={() => retry()}
		/>
	{:else}
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
						conflict ||
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
			{#if conflict}<Button size="sm" variant="outline" class="mt-2" onclick={resolveConflict}
					>ใช้วิธีจัดรายชื่อล่าสุด</Button
				>
			{:else}<Button size="sm" variant="outline" class="mt-2" onclick={() => retry()}
					>โหลดวิธีจัดรายชื่อล่าสุด</Button
				>{/if}
		{/if}
		{#if saved}<p role="status" class="mt-3 text-sm text-muted-foreground">{saved}</p>{/if}
	{/if}
</section>
