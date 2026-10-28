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
	type FormSession = {
		token: number;
		owner: string;
		changeSetId: string;
		offeringId: string;
		groupId: string;
		onSaved: (updated: AcademicTermChangeSet) => Promise<void>;
		onConflict: (message: string) => Promise<void>;
		onSavingChange: (value: boolean) => void;
		onCancel: () => void;
	};
	let session = 0,
		disposed = false,
		observedContext = '';
	let activeForm = $state.raw<FormSession | null>(null);
	const request = new LatestRequest();
	const context = $derived(`${changeSetId}:${offeringId}:${groupId}`);
	function current(owned: FormSession): boolean {
		return (
			!disposed &&
			open &&
			canManage &&
			activeForm === owned &&
			owned.token === session &&
			owned.owner === context
		);
	}
	function changeOpen(value: boolean) {
		if (saving) return;
		open = value;
		if (!value) {
			session++;
			activeForm = null;
			request.abort();
			loading = false;
		}
	}
	async function load(owned = activeForm) {
		if (!owned || !current(owned)) return;
		const ticket = request.begin();
		loading = true;
		errorMessage = '';
		try {
			const loaded = await getAcademicTermChangeSet(owned.changeSetId, { signal: ticket.signal });
			if (!current(owned) || !request.isCurrent(ticket.revision)) return;
			const loadedOptions = await getLearningDeliveryManagementOptions(loaded.academicTermId, {
				signal: ticket.signal
			});
			if (!current(owned) || !request.isCurrent(ticket.revision)) return;
			if (
				loaded.status !== 'draft' ||
				!loadedOptions.learningGroups.some(
					(group) => group.id === owned.groupId && group.learningOfferingId === owned.offeringId
				)
			)
				throw new Error('กลุ่มนี้ไม่อยู่ในขอบเขตการแก้ไขแล้ว');
			changeSet = loaded;
			options = loadedOptions;
		} catch (error) {
			if (!isAbortError(error) && current(owned) && request.isCurrent(ticket.revision))
				errorMessage = error instanceof Error ? error.message : 'โหลดช่วงความรับผิดชอบไม่สำเร็จ';
		} finally {
			if (current(owned) && request.isCurrent(ticket.revision)) loading = false;
		}
	}
	async function show() {
		if (!canManage || saving) return;
		session++;
		open = true;
		changeSet = null;
		options = null;
		const update = onUpdated;
		const owned: FormSession = {
			token: session,
			owner: context,
			changeSetId,
			offeringId,
			groupId,
			async onSaved(updated) {
				if (!current(owned)) return;
				await update(updated);
				if (!current(owned)) return;
				open = false;
				saving = false;
				session++;
				activeForm = null;
				request.abort();
			},
			async onConflict(message) {
				if (!current(owned)) return;
				errorMessage = message;
				const ticket = request.begin();
				try {
					const loaded = await getAcademicTermChangeSet(owned.changeSetId, {
						signal: ticket.signal
					});
					if (!current(owned) || !request.isCurrent(ticket.revision)) return;
					changeSet = loaded;
				} catch (error) {
					if (!isAbortError(error) && current(owned) && request.isCurrent(ticket.revision))
						errorMessage = error instanceof Error ? `${message} (${error.message})` : message;
				}
			},
			onSavingChange(value) {
				if (current(owned)) {
					saving = value;
					if (value) errorMessage = '';
				}
			},
			onCancel() {
				if (current(owned)) changeOpen(false);
			}
		};
		activeForm = owned;
		await load(owned);
	}
	$effect.pre(() => {
		const owner = context;
		if (observedContext !== owner || !canManage) {
			observedContext = owner;
			open = false;
			session++;
			activeForm = null;
			request.abort();
			loading = false;
			saving = false;
		}
	});
	onDestroy(() => {
		disposed = true;
		session++;
		activeForm = null;
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
			{#if loading}<PageSkeleton variant="form" />
			{:else if changeSet && options && activeForm}{#key activeForm}<AcademicTeacherChangeForm
						{changeSet}
						managementOptions={options}
						offeringId={activeForm.offeringId}
						initialGroupId={activeForm.groupId}
						onSavingChange={activeForm.onSavingChange}
						onSaved={activeForm.onSaved}
						onConflict={activeForm.onConflict}
						onCancel={activeForm.onCancel}
					/>{/key}{:else if errorMessage}<LoadingButton
					variant="outline"
					{loading}
					onclick={() => load()}>ลองอีกครั้ง</LoadingButton
				>{/if}
			{#if errorMessage}<p role="alert" class="mt-3 text-sm text-destructive">
					{errorMessage}
				</p>{/if}
		</div>
	</Dialog.Content>
</Dialog.Root>
