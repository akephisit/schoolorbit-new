<script lang="ts">
	import { onDestroy } from 'svelte';
	import {
		getAcademicTermChangeSet,
		getLearningDeliveryManagementOptions,
		type AcademicTermChangeSet,
		type DeliveryManagementOptions
	} from '#lib/api/learning-delivery.js';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { LoadingButton, PageSkeleton } from '#lib/components/app-state/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import AcademicTeacherChangeForm from './AcademicTeacherChangeForm.svelte';
	let {
		changeSetId,
		offeringId,
		groupId,
		canManage,
		onUpdated
	}: {
		changeSetId: string;
		offeringId: string;
		groupId: string;
		canManage: boolean;
		onUpdated: (updated: AcademicTermChangeSet) => Promise<void>;
	} = $props();
	let open = $state(false),
		loading = $state(false),
		saving = $state(false);
	let changeSet = $state.raw<AcademicTermChangeSet | null>(null),
		options = $state.raw<DeliveryManagementOptions | null>(null);
	let errorMessage = $state('');
	let trigger = $state<HTMLButtonElement | null>(null);
	let session = 0,
		disposed = false,
		observedContext = '';
	const request = new LatestRequest();
	const context = $derived(`${changeSetId}:${offeringId}:${groupId}`);
	function changeOpen(value: boolean) {
		if (saving) return;
		open = value;
		if (!value) {
			session++;
			request.abort();
			loading = false;
		}
	}
	async function load() {
		const token = session,
			owner = context,
			ticket = request.begin();
		loading = true;
		errorMessage = '';
		try {
			const loaded = await getAcademicTermChangeSet(changeSetId, { signal: ticket.signal });
			const loadedOptions = await getLearningDeliveryManagementOptions(loaded.academicTermId, {
				signal: ticket.signal
			});
			if (
				disposed ||
				!open ||
				token !== session ||
				owner !== context ||
				!canManage ||
				!request.isCurrent(ticket.revision)
			)
				return;
			if (
				loaded.status !== 'draft' ||
				!loadedOptions.learningGroups.some(
					(group) => group.id === groupId && group.learningOfferingId === offeringId
				)
			)
				throw new Error('กลุ่มนี้ไม่อยู่ในขอบเขตการแก้ไขแล้ว');
			changeSet = loaded;
			options = loadedOptions;
		} catch (error) {
			if (!isAbortError(error) && !disposed && token === session && owner === context)
				errorMessage = error instanceof Error ? error.message : 'โหลดช่วงความรับผิดชอบไม่สำเร็จ';
		} finally {
			if (!disposed && token === session && owner === context) loading = false;
		}
	}
	async function show() {
		if (!canManage || saving) return;
		session++;
		open = true;
		changeSet = null;
		options = null;
		await load();
	}
	async function changed(updated: AcademicTermChangeSet) {
		await onUpdated(updated);
		open = false;
		session++;
	}
	async function conflict(message: string) {
		errorMessage = message;
		const loaded = await getAcademicTermChangeSet(changeSetId);
		if (!disposed && open) changeSet = loaded;
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

<Button bind:ref={trigger} variant="outline" size="sm" disabled={!canManage} onclick={show}
	>เปลี่ยนครูผู้สอน</Button
>
<Dialog.Root {open} onOpenChange={changeOpen}>
	<Dialog.Content
		class="flex max-h-[calc(100dvh-2rem)] flex-col overflow-hidden sm:max-w-2xl"
		showCloseButton={!saving}
		escapeKeydownBehavior={saving ? 'ignore' : 'close'}
		interactOutsideBehavior={saving ? 'ignore' : 'close'}
		onCloseAutoFocus={(event) => {
			event.preventDefault();
			if (!disposed && trigger?.isConnected) trigger.focus();
		}}
	>
		<Dialog.Header class="shrink-0"
			><Dialog.Title>เปลี่ยนครูผู้สอนในกลุ่มนี้</Dialog.Title><Dialog.Description
				>บันทึกช่วงความรับผิดชอบในแบบร่าง แล้วเลือกวันเริ่มใช้ตอนเผยแพร่</Dialog.Description
			></Dialog.Header
		>
		<div class="min-h-0 overflow-y-auto">
			{#if loading}<PageSkeleton
					variant="form"
				/>{:else if changeSet && options}<AcademicTeacherChangeForm
					{changeSet}
					managementOptions={options}
					{offeringId}
					initialGroupId={groupId}
					onSavingChange={(value) => (saving = value)}
					onSaved={changed}
					onConflict={conflict}
					onCancel={() => changeOpen(false)}
				/>{:else if errorMessage}<LoadingButton variant="outline" {loading} onclick={load}
					>ลองอีกครั้ง</LoadingButton
				>{/if}
			{#if errorMessage}<p role="alert" class="mt-3 text-sm text-destructive">
					{errorMessage}
				</p>{/if}
		</div>
	</Dialog.Content>
</Dialog.Root>
