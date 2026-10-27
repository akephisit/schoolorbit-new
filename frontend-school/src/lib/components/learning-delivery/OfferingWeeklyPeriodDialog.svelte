<script lang="ts">
	import { onDestroy } from 'svelte';
	import {
		getAcademicTermChangeSet,
		getDeliveryVersion,
		upsertAcademicTermChangeItem,
		type AcademicTermChangeSet,
		type DeliveryVersion
	} from '#lib/api/learning-delivery.js';
	import { ApiClientError } from '#lib/api/client.js';
	import { weeklyPeriodAdjustmentRequest } from '#lib/academic/learning-delivery-page.js';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import { LoadingButton, PageSkeleton } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';

	let {
		version,
		changeSetId,
		offeringId,
		canManage,
		onUpdated
	}: {
		version: DeliveryVersion;
		changeSetId: string;
		offeringId: string;
		canManage: boolean;
		onUpdated: (version: DeliveryVersion) => void;
	} = $props();
	let open = $state(false);
	let loading = $state(false);
	let saving = $state(false);
	let errorMessage = $state('');
	let changeSet = $state.raw<AcademicTermChangeSet | null>(null);
	let baseline = $state.raw<DeliveryVersion | null>(null);
	let weeklyPeriodTarget = $state<number>(0);
	let needsReload = $state(false);
	let saved = $state(false);
	let trigger = $state<HTMLButtonElement | null>(null);
	let session = 0;
	let disposed = false;
	let observedContext = '';
	const request = new LatestRequest();
	const context = $derived(`${version.id}:${changeSetId}:${offeringId}`);
	const offering = $derived(baseline?.snapshot.offerings.find((item) => item.id === offeringId));
	const busy = $derived(loading || saving);
	const valid = $derived(
		changeSet && offering && weeklyPeriodAdjustmentRequest(changeSet, offering, weeklyPeriodTarget)
	);
	function current(token: number, owner: string) {
		return (
			!disposed &&
			open &&
			canManage &&
			version.status === 'draft' &&
			session === token &&
			context === owner
		);
	}
	function changeOpen(value: boolean) {
		if (saving) return;
		open = value;
		if (!value) {
			session += 1;
			request.abort();
			loading = false;
		}
	}
	async function show() {
		if (!canManage || version.status !== 'draft' || busy) return;
		session += 1;
		open = true;
		changeSet = null;
		baseline = null;
		needsReload = false;
		saved = false;
		await load(false);
	}
	async function load(preserveDraft: boolean) {
		const token = session,
			owner = context;
		const ticket = request.begin();
		loading = true;
		errorMessage = '';
		try {
			const [loadedChangeSet, loadedVersion] = await Promise.all([
				getAcademicTermChangeSet(changeSetId, { signal: ticket.signal }),
				getDeliveryVersion(version.id, { signal: ticket.signal })
			]);
			if (!current(token, owner) || !request.isCurrent(ticket.revision)) return;
			const target = loadedVersion.snapshot.offerings.find((item) => item.id === offeringId);
			if (
				loadedChangeSet.status !== 'draft' ||
				loadedVersion.status !== 'draft' ||
				loadedChangeSet.targetDeliveryVersionId !== loadedVersion.id ||
				!target
			)
				throw new Error('รายการนี้ไม่อยู่ในรุ่นแบบร่างที่แก้ไขได้แล้ว');
			changeSet = loadedChangeSet;
			baseline = loadedVersion;
			if (!preserveDraft) weeklyPeriodTarget = target.weeklyPeriodTarget;
			needsReload = false;
		} catch (error) {
			if (!isAbortError(error) && current(token, owner)) {
				needsReload = true;
				errorMessage = error instanceof Error ? error.message : 'โหลดจำนวนคาบล่าสุดไม่สำเร็จ';
			}
		} finally {
			if (current(token, owner)) loading = false;
		}
	}
	async function refreshSaved(token = session, owner = context) {
		const ticket = request.begin();
		loading = true;
		errorMessage = '';
		try {
			const updated = await getDeliveryVersion(version.id, { signal: ticket.signal });
			if (!current(token, owner) || !request.isCurrent(ticket.revision)) return;
			onUpdated(updated);
			open = false;
			session += 1;
		} catch (error) {
			if (!isAbortError(error) && current(token, owner))
				errorMessage =
					error instanceof Error ? error.message : 'โหลดจำนวนคาบที่บันทึกแล้วไม่สำเร็จ';
		} finally {
			if (!disposed && context === owner) loading = false;
		}
	}
	async function save(event: SubmitEvent) {
		event.preventDefault();
		if (!canManage || version.status !== 'draft' || busy || !valid || needsReload || saved) return;
		const token = session,
			owner = context;
		saving = true;
		errorMessage = '';
		try {
			await upsertAcademicTermChangeItem(changeSetId, valid);
			if (!current(token, owner)) return;
			saved = true;
			await refreshSaved(token, owner);
		} catch (error) {
			if (!current(token, owner)) return;
			if (error instanceof ApiClientError && error.status === 409) needsReload = true;
			errorMessage = error instanceof Error ? error.message : 'บันทึกจำนวนคาบไม่สำเร็จ';
		} finally {
			if (!disposed && context === owner) saving = false;
		}
	}
	$effect.pre(() => {
		const owner = context;
		if (observedContext !== owner || !canManage || version.status !== 'draft') {
			observedContext = owner;
			open = false;
			session += 1;
			request.abort();
			loading = false;
			saving = false;
		}
	});
	onDestroy(() => {
		disposed = true;
		session += 1;
		request.abort();
	});
</script>

<Button bind:ref={trigger} size="sm" variant="outline" onclick={show} disabled={!canManage}
	>แก้ไขจำนวนคาบ</Button
>
<Dialog.Root {open} onOpenChange={changeOpen}>
	<Dialog.Content
		class="sm:max-w-lg"
		showCloseButton={!saving}
		escapeKeydownBehavior={saving ? 'ignore' : 'close'}
		interactOutsideBehavior={saving ? 'ignore' : 'close'}
		onCloseAutoFocus={(event) => {
			event.preventDefault();
			if (!disposed && trigger?.isConnected) trigger.focus();
		}}
	>
		<Dialog.Header>
			<Dialog.Title>แก้ไขจำนวนคาบต่อสัปดาห์</Dialog.Title>
			<Dialog.Description
				>ปรับจำนวนคาบของรายการนี้ในรุ่นแบบร่าง ใช้กับทุกกลุ่มเรียนของรายการนี้
				เมื่อพร้อมจึงเผยแพร่จากหน้าภาพรวม</Dialog.Description
			>
		</Dialog.Header>
		{#if loading && !baseline}<PageSkeleton variant="form" />{/if}
		<form class="space-y-4" onsubmit={save}>
			{#if offering}
				<p class="font-medium">{offering.name}</p>
				{#if offering.catalog.kind === 'course'}<p class="text-sm text-muted-foreground">
						ตามหลักสูตร {offering.catalog.standardPeriodsPerWeek} คาบ/สัปดาห์
					</p>{/if}
				<div class="space-y-2">
					<Label for="offering-weekly-period-target">คาบที่จัดจริงต่อสัปดาห์</Label>
					<Input
						id="offering-weekly-period-target"
						type="number"
						min={offering.kind === 'course' ? 0 : 1}
						step="1"
						required
						bind:value={weeklyPeriodTarget}
						disabled={busy || saved}
					/>
					{#if offering.kind === 'course'}<p class="text-xs text-muted-foreground">
							0 คาบ = เปิดรายวิชาโดยไม่จัดคาบแยกในตารางสอน โดยหน่วยกิตและคาบตามหลักสูตรยังคงเดิม
						</p>{/if}
				</div>
			{/if}
			{#if saved}<p role="status" class="text-sm text-muted-foreground">
					บันทึกจำนวนคาบแล้ว กำลังแสดงข้อมูลล่าสุด
				</p>{/if}
			{#if errorMessage}<p role="alert" class="text-sm text-destructive">{errorMessage}</p>{/if}
			{#if saved && errorMessage}<LoadingButton
					type="button"
					variant="outline"
					{loading}
					onclick={() => refreshSaved()}>โหลดจำนวนคาบที่บันทึกแล้ว</LoadingButton
				>
			{:else if needsReload}<LoadingButton
					type="button"
					variant="outline"
					{loading}
					onclick={() => load(baseline !== null)}>โหลดข้อมูลล่าสุดก่อนบันทึก</LoadingButton
				>{/if}
			<Dialog.Footer>
				<Button type="button" variant="outline" disabled={saving} onclick={() => changeOpen(false)}
					>{saved ? 'ปิด' : 'ยกเลิก'}</Button
				>
				<LoadingButton
					type="submit"
					loading={saving}
					loadingLabel="กำลังบันทึก"
					disabled={loading ||
						!valid ||
						needsReload ||
						saved ||
						weeklyPeriodTarget === offering?.weeklyPeriodTarget}>บันทึกจำนวนคาบ</LoadingButton
				>
			</Dialog.Footer>
		</form>
	</Dialog.Content>
</Dialog.Root>
