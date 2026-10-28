<script lang="ts">
	import { onDestroy } from 'svelte';
	import { ApiClientError } from '#lib/api/client.js';
	import {
		getAcademicTermChangeSet,
		getDeliveryVersion,
		getLearningDeliveryManagementOptions,
		removeDeliveryDraftOffering,
		removeDeliveryDraftOfferingHomeroom,
		removeDeliveryDraftGroup,
		type AcademicTermChangeSet,
		type DeliveryVersion,
		type DeliveryManagementOptions
	} from '#lib/api/learning-delivery.js';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import { LoadingButton, PageSkeleton } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as AlertDialog from '#lib/components/ui/alert-dialog/index.js';
	import { Trash2 } from '@lucide/svelte';
	type RemovalScope =
		| { groupId: string; homeroomId?: null; homeroomName?: null }
		| { groupId?: null; homeroomId: string; homeroomName: string }
		| { groupId?: null; homeroomId?: null; homeroomName?: null };
	let {
		changeSetId,
		versionId,
		offeringId,
		label,
		groupId = null,
		homeroomId = null,
		homeroomName = null,
		canManage,
		onRemoved
	}: {
		changeSetId: string;
		versionId: string;
		offeringId: string;
		label: string;
		canManage: boolean;
		onRemoved: (updated: AcademicTermChangeSet) => Promise<void>;
	} & RemovalScope = $props();
	let open = $state(false),
		loading = $state(false),
		saving = $state(false),
		needsReload = $state(false);
	let errorMessage = $state('');
	let changeSet = $state.raw<AcademicTermChangeSet | null>(null);
	let version = $state.raw<DeliveryVersion | null>(null);
	let options = $state.raw<DeliveryManagementOptions | null>(null);
	let saved = $state.raw<AcademicTermChangeSet | null>(null);
	let trigger = $state<HTMLButtonElement | null>(null);
	let disposed = false;
	let session = 0;
	let observedContext = '';
	const request = new LatestRequest();
	const context = $derived(
		`${changeSetId}:${versionId}:${offeringId}:${groupId ?? ''}:${homeroomId ?? ''}`
	);
	const validScope = $derived(!(groupId && homeroomId));
	const actionLabel = $derived(
		homeroomId ? 'นำออกจากห้องนี้' : groupId ? 'ลบกลุ่มจากรุ่นนี้' : 'ลบจากรุ่นนี้'
	);
	const offering = $derived(version?.snapshot.offerings.find((item) => item.id === offeringId));
	const group = $derived(offering?.groups.find((item) => item.id === groupId));
	const homeroomNames = $derived(
		(group?.homeroomIds ?? []).map(
			(id) => options?.homerooms.find((room) => room.id === id)?.name ?? 'ไม่พบชื่อห้องต้นทาง'
		)
	);
	function current(token: number, owner: string) {
		return !disposed && open && canManage && token === session && owner === context;
	}
	function changeOpen(value: boolean) {
		if (saving) return;
		open = value;
		if (!value) {
			session++;
			request.abort();
			loading = false;
		}
	}
	async function show() {
		if (!canManage || saving || !validScope) return;
		open = true;
		session++;
		saved = null;
		changeSet = null;
		version = null;
		options = null;
		await load();
	}
	async function load() {
		const token = session,
			owner = context,
			ticket = request.begin();
		loading = true;
		needsReload = true;
		errorMessage = '';
		try {
			const [loadedChangeSet, loadedVersion] = await Promise.all([
				getAcademicTermChangeSet(changeSetId, { signal: ticket.signal }),
				getDeliveryVersion(versionId, { signal: ticket.signal })
			]);
			if (!current(token, owner) || !request.isCurrent(ticket.revision)) return;
			if (!validScope) throw new Error('เลือกนำออกจากห้องหรือกลุ่มอย่างใดอย่างหนึ่ง');
			const loadedOptions = groupId
				? await getLearningDeliveryManagementOptions(loadedChangeSet.academicTermId, {
						signal: ticket.signal
					})
				: null;
			if (!current(token, owner) || !request.isCurrent(ticket.revision)) return;
			const target = loadedVersion.snapshot.offerings.find((item) => item.id === offeringId);
			if (
				loadedChangeSet.status !== 'draft' ||
				loadedVersion.status !== 'draft' ||
				loadedChangeSet.targetDeliveryVersionId !== loadedVersion.id ||
				!target ||
				(groupId && !target.groups.some((item) => item.id === groupId))
			)
				throw new Error('รายการนี้ไม่อยู่ในแบบร่างที่แก้ไขได้แล้ว');
			if (homeroomId && !target.homeroomIds.includes(homeroomId))
				throw new Error('รายการนี้ไม่อยู่ในห้องที่เลือกแล้ว กรุณาตรวจข้อมูลรายห้องล่าสุด');
			changeSet = loadedChangeSet;
			version = loadedVersion;
			options = loadedOptions;
			needsReload = false;
		} catch (error) {
			if (!isAbortError(error) && current(token, owner))
				errorMessage = error instanceof Error ? error.message : 'โหลดขอบเขตการนำออกไม่สำเร็จ';
		} finally {
			if (current(token, owner)) loading = false;
		}
	}
	async function finish(updated: AcademicTermChangeSet, token: number, owner: string) {
		if (!current(token, owner)) return;
		await onRemoved(updated);
		if (current(token, owner)) {
			saving = false;
			open = false;
			session++;
			request.abort();
		}
	}
	async function remove() {
		if (
			!canManage ||
			!validScope ||
			!changeSet ||
			!version ||
			loading ||
			saving ||
			needsReload ||
			saved
		)
			return;
		const token = session,
			owner = context;
		saving = true;
		errorMessage = '';
		try {
			const body = {
				changeSetRowVersion: changeSet.rowVersion,
				targetDeliveryVersionRowVersion: version.rowVersion
			};
			const updated = groupId
				? await removeDeliveryDraftGroup(changeSetId, groupId, body)
				: homeroomId
					? await removeDeliveryDraftOfferingHomeroom(changeSetId, offeringId, homeroomId, body)
					: await removeDeliveryDraftOffering(changeSetId, offeringId, body);
			if (!current(token, owner)) return;
			saved = updated;
			await finish(updated, token, owner);
		} catch (error) {
			if (current(token, owner)) {
				if (error instanceof ApiClientError && error.status === 409) needsReload = true;
				errorMessage = error instanceof Error ? error.message : 'นำรายการออกไม่สำเร็จ';
			}
		} finally {
			if (!disposed && session === token && context === owner) saving = false;
		}
	}
	async function reconcileSaved() {
		if (!saved || saving) return;
		const token = session,
			owner = context,
			accepted = saved;
		saving = true;
		errorMessage = '';
		try {
			await finish(accepted, token, owner);
		} catch (error) {
			if (current(token, owner))
				errorMessage = error instanceof Error ? error.message : 'แสดงข้อมูลล่าสุดไม่สำเร็จ';
		} finally {
			if (!disposed && session === token && context === owner) saving = false;
		}
	}

	$effect.pre(() => {
		const owner = context;
		if (observedContext !== owner || !canManage) {
			observedContext = owner;
			open = false;
			session++;
			request.abort();
			loading = false;
			saving = false;
		}
	});
	onDestroy(() => {
		disposed = true;
		session++;
		request.abort();
	});
</script>

<Button
	bind:ref={trigger}
	size="sm"
	variant="ghost"
	disabled={!canManage || !validScope}
	onclick={show}
	aria-label={`${actionLabel} · ${homeroomName ? `${homeroomName} · ` : ''}${label}`}
	><Trash2 class="size-4" /><span>{actionLabel}</span></Button
>
<AlertDialog.Root {open} onOpenChange={changeOpen}>
	<AlertDialog.Content
		class="flex max-h-[calc(100dvh-2rem)] flex-col overflow-hidden sm:max-w-xl"
		escapeKeydownBehavior={saving ? 'ignore' : 'close'}
		onCloseAutoFocus={(event) => {
			event.preventDefault();
			if (!disposed && trigger?.isConnected) trigger.focus();
		}}
	>
		<AlertDialog.Header class="shrink-0"
			><AlertDialog.Title>{actionLabel}</AlertDialog.Title><AlertDialog.Description
				>“{label}” · {changeSet?.reason ?? 'กำลังตรวจรุ่นแบบร่างที่เลือก'}</AlertDialog.Description
			></AlertDialog.Header
		>
		<div class="min-h-0 space-y-3 overflow-y-auto">
			{#if loading}<PageSkeleton variant="detail" />{:else if version && offering}
				{#if groupId}<p class="text-sm">
						นำกลุ่ม “{group?.name ?? label}” ออกจากรายการ “{offering.name}” ในแบบร่างนี้
					</p>
					{#if offering.catalog.kind === 'activity' && offering.catalog.schedulingMode === 'synchronized'}<p
							class="text-sm font-medium"
						>
							กิจกรรมคาบพร้อมกัน: นำเฉพาะกลุ่มนี้ออก โดยห้องที่เข้าร่วมกิจกรรมในรุ่นนี้ยังคงเดิม
						</p>{:else}<p class="text-sm text-muted-foreground">
							ห้องที่ไม่มีกลุ่มอื่นรองรับจะออกจากขอบเขตของรายการในรุ่นนี้ หากไม่เหลือห้อง
							รายการเปิดสอนนี้จะออกจากรุ่นด้วย รวมถึงกลุ่มที่ไม่ได้ผูกห้อง
						</p>{/if}
					{#if homeroomNames.length > 0}<div class="text-sm">
							<p class="font-medium">ห้องของกลุ่มนี้ในรุ่นที่เลือก ({homeroomNames.length} ห้อง)</p>
							<ul class="mt-1 list-inside list-disc">
								{#each homeroomNames as name, index (`${index}:${name}`)}<li>{name}</li>{/each}
							</ul>
						</div>{/if}
				{:else if homeroomId}<p class="text-sm">
						นำรายการ “{offering.name}” ออกจากห้อง “{homeroomName}” เฉพาะรุ่นแบบร่างนี้
					</p>
					<p class="text-sm text-muted-foreground">
						ห้องอื่นยังอยู่ กลุ่มที่ยังครอบคลุมห้องอื่นจะคงเดิม รวมถึงครู รายชื่อ และคะแนนที่มีอยู่
					</p>
				{:else}<p class="text-sm">
						นำรายการ “{offering.name}” พร้อมทุกกลุ่มเรียน {offering.groups.length} กลุ่มออกจากแบบร่างนี้
					</p>{/if}
				<p class="text-sm text-muted-foreground">
					รุ่นที่เผยแพร่แล้ว ตารางสอน รายชื่อ คะแนน และประวัติเดิมยังคงอยู่
					การนำออกมีผลเฉพาะแบบร่างที่เลือก
				</p>
			{/if}
			{#if saved}<p role="status" class="text-sm">นำออกจากแบบร่างแล้ว กำลังแสดงข้อมูลล่าสุด</p>{/if}
			{#if errorMessage}<p role="alert" class="text-sm text-destructive">{errorMessage}</p>{/if}
		</div>
		<AlertDialog.Footer class="shrink-0"
			><Button variant="outline" disabled={saving} onclick={() => changeOpen(false)}>ยกเลิก</Button>
			{#if saved}<LoadingButton loading={saving} onclick={reconcileSaved}
					>แสดงข้อมูลล่าสุด</LoadingButton
				>
			{:else if needsReload && !loading}<LoadingButton {loading} variant="outline" onclick={load}
					>ตรวจขอบเขตล่าสุดอีกครั้ง</LoadingButton
				>
			{:else}<LoadingButton
					variant="destructive"
					loading={saving}
					loadingLabel="กำลังนำออก"
					disabled={loading || needsReload || !changeSet || !version}
					onclick={remove}>ยืนยัน{actionLabel}</LoadingButton
				>{/if}
		</AlertDialog.Footer>
	</AlertDialog.Content>
</AlertDialog.Root>
