<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import {
		sendCorrespondence,
		sendDirectedCorrespondence,
		type DirectedSendCorrespondenceRequest,
		type CorrespondenceDetail,
		type CorrespondenceOptions,
		type SendCorrespondenceRequest,
		type ProcessStepRecipients
	} from '#lib/api/correspondence.js';
	import { LoadingButton, PageState } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { MutationIntent } from '#lib/correspondence/mutation-intent.js';
	import RoutePicker from './RoutePicker.svelte';
	import {
		focusCorrespondenceField,
		type CorrespondenceFieldErrors
	} from '#lib/correspondence/validation.js';
	let fieldErrors = $state<CorrespondenceFieldErrors>({});
	let {
		detail,
		oncomplete,
		oncancel,
		ondirtychange,
		onrefresh,
		onpendingchange
	}: {
		detail: CorrespondenceDetail;
		oncomplete: (detail: CorrespondenceDetail) => void;
		oncancel: () => void;
		ondirtychange?: (dirty: boolean) => void;
		onrefresh: () => void;
		onpendingchange?: (pending: boolean) => void;
	} = $props();
	let mode = $state(
			untrack(() => (detail.instance && !detail.instance.directed ? 'template' : 'directed'))
		),
		firstUserId = $state('');
	let templateId = $state(''),
		steps = $state<ProcessStepRecipients[]>([]),
		dueDate = $state<string | undefined>();
	let options = $state<CorrespondenceOptions | null>(null),
		error = $state(''),
		pending = $state(false);
	let expectedRevision = $state(untrack(() => detail.record.revision));
	let disposed = false;
	const intent = new MutationIntent<SendCorrespondenceRequest>();
	const directedIntent = new MutationIntent<DirectedSendCorrespondenceRequest>();
	let uncertain = $state(false);
	$effect(() => {
		ondirtychange?.(!!templateId || !!firstUserId);
	});
	onDestroy(() => {
		disposed = true;
	});
	const template = $derived(options?.templates.find((item) => item.id === templateId));
	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (pending || uncertain) return;
		error = '';
		fieldErrors = {};
		const missingStep = steps.findIndex((step) => !step.recipients.length);
		const invalidId =
			mode === 'directed'
				? !firstUserId
					? 'send-first-decisionmaker'
					: ''
				: !template
					? 'send-template'
					: missingStep >= 0
						? `send-step-${missingStep}-recipients`
						: '';
		if (invalidId) {
			error =
				mode === 'directed'
					? 'เลือกผู้พิจารณาสั่งการคนแรก'
					: !template
						? 'เลือกแม่แบบลำดับงาน'
						: 'เลือกผู้รับให้ครบทุกขั้นตอน';
			fieldErrors = { [invalidId]: error };
			void focusCorrespondenceField(invalidId);
			return;
		}
		pending = true;
		onpendingchange?.(true);
		try {
			const result =
				mode === 'directed'
					? await sendDirectedCorrespondence(
							detail.record.id,
							directedIntent.begin({
								expectedRevision,
								firstRecipient: { kind: 'user', userId: firstUserId },
								dueAt: dueDate ? `${dueDate}T23:59:59+07:00` : undefined
							})
						)
					: await sendCorrespondence(
							detail.record.id,
							intent.begin({
								expectedRevision,
								templateId,
								stepRecipients: steps,
								dueAt: dueDate ? `${dueDate}T23:59:59+07:00` : undefined
							})
						);
			intent.accepted();
			directedIntent.accepted();
			if (!disposed) oncomplete(result);
		} catch (cause) {
			intent.rejected(cause);
			directedIntent.rejected(cause);
			uncertain = intent.unresolved || directedIntent.unresolved;
			if (!disposed) error = cause instanceof Error ? cause.message : 'ส่งหนังสือไม่สำเร็จ';
		} finally {
			if (!disposed) {
				pending = false;
				onpendingchange?.(false);
			}
		}
	}
</script>

<form class="space-y-4" onsubmit={submit} data-testid="correspondence-send">
	<h3 class="font-semibold">ส่งหนังสือดำเนินการ</h3>
	{#if expectedRevision !== detail.record.revision}<p class="text-sm text-warning">
			เอกสารเปลี่ยนแปลงแล้ว กรุณาตรวจสอบฉบับปัจจุบัน
		</p>
		<Button
			type="button"
			variant="outline"
			onclick={() => {
				expectedRevision = detail.record.revision;
			}}>ใช้ข้อมูลล่าสุดและเก็บผู้รับที่เลือก</Button
		>{/if}
	{#if error}<PageState
			variant="error"
			title={error}
			description="ผู้รับที่เลือกยังอยู่ ตรวจสอบฉบับล่าสุดก่อนลองส่งอีกครั้ง"
			actionLabel="ตรวจสอบข้อมูลล่าสุด"
			onaction={onrefresh}
		/>{/if}
	{#if uncertain}<p class="text-sm text-warning">
			ยังยืนยันผลการส่งไม่ได้ กดส่งอีกครั้งเพื่อตรวจผลด้วยข้อมูลผู้รับเดิม
		</p>{/if}
	<RoutePicker
		{fieldErrors}
		bind:mode
		bind:firstUserId
		register={detail.record.register}
		bind:templateId
		bind:steps
		bind:dueDate
		disabled={pending || uncertain}
		onready={(value) => {
			options = value;
		}}
	/>

	<div class="flex justify-end gap-2">
		<Button type="button" variant="outline" onclick={oncancel} disabled={pending}>ยกเลิก</Button
		><LoadingButton type="submit" loading={pending} disabled={uncertain || !options}
			>ส่งหนังสือ</LoadingButton
		>
	</div>
</form>
