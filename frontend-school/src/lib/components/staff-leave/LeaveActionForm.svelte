<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import {
		actOnLeaveRequest,
		type LeaveRequestDetail,
		type ProcessAction,
		type SignatureInput
	} from '#lib/api/staff-leave.js';
	import { uploadFile } from '#lib/api/files.js';
	import { authStore } from '#lib/stores/auth.js';
	import { LeaveMutationIntent } from '#lib/staff-leave/mutation-intent.js';
	import { leaveActionLabels } from '#lib/staff-leave/presentation.js';
	import { hasInk, type InkPath } from '#lib/correspondence/geometry.js';
	import { LoadingButton, PageState } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import * as ToggleGroup from '#lib/components/ui/toggle-group/index.js';
	import SignaturePad from '#lib/components/correspondence/SignaturePad.svelte';
	let {
		detail,
		action,
		oncomplete,
		oncancel,
		onrefresh
	}: {
		detail: LeaveRequestDetail;
		action: ProcessAction;
		oncomplete: (result: LeaveRequestDetail) => void;
		oncancel: () => void;
		onrefresh: () => void;
	} = $props();
	let expectedRevision = $state(untrack(() => detail.request.revision)),
		expectedProcessRevision = $state(untrack(() => detail.instance?.revision ?? 0));
	let comment = $state(''),
		pending = $state(false),
		error = $state(''),
		confirmed = $state(false),
		paths = $state<InkPath[]>([]);
	let method = $state<'drawn' | 'image'>('drawn'),
		image = $state<File | null>(null),
		imageFileId = $state<string | null>(null);
	const intent = new LeaveMutationIntent();
	let disposed = false;
	const definition = $derived(detail.instance?.steps[detail.instance.currentStep]?.definition);
	const completing = $derived(!['reject', 'return_for_revision', 'read'].includes(action));
	const signing = $derived(completing && !!definition?.requiresSignature);
	const commentRequired = $derived(
		action === 'reject' || action === 'return_for_revision' || !!definition?.requiresComment
	);
	const stale = $derived(
		expectedRevision !== detail.request.revision ||
			expectedProcessRevision !== detail.instance?.revision
	);
	$effect.pre(() => {
		void detail.contentHash;
		void detail.request.revision;
		void method;
		void paths;
		void image;
		untrack(() => {
			confirmed = false;
		});
	});
	onDestroy(() => (disposed = true));
	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (pending) return;
		error = '';
		if (stale) {
			error = 'ตรวจสอบข้อมูลล่าสุดก่อนยืนยันอีกครั้ง';
			return;
		}
		if (commentRequired && !comment.trim()) {
			error = 'กรอกความคิดเห็น / เหตุผล';
			return;
		}
		if (signing && (!confirmed || (method === 'drawn' ? !hasInk(paths) : !image && !imageFileId))) {
			error = 'ระบุลายมือชื่อและยืนยันการใช้กับคำขอฉบับนี้';
			return;
		}
		pending = true;
		try {
			if (signing && method === 'image' && image && !imageFileId) {
				const uploaded = await uploadFile(image, 'staff_leave_signature', $authStore.user?.id);
				if (disposed) return;
				imageFileId = uploaded.id;
			}
			const input: SignatureInput | null = signing
				? {
						method,
						assetId: null,
						strokes:
							method === 'drawn'
								? paths.map((path) => ({ ...path, width: 0.005, color: '#000000' }))
								: [],
						page: 1,
						x: 0,
						y: 0,
						width: 1,
						height: 1,
						documentRevision: expectedRevision,
						contentHash: detail.contentHash
					}
				: null;
			const basis = {
				expectedRevision,
				expectedProcessRevision,
				purpose: detail.processPurpose,
				action,
				comment: comment.trim() || null,
				signature: input
					? { consent: true, input, imageFileId: method === 'image' ? imageFileId : null }
					: null
			};
			const result = await actOnLeaveRequest(detail.request.id, {
				requestId: intent.for(basis),
				...basis
			});
			if (!disposed) oncomplete(result);
		} catch (cause) {
			if (!disposed) error = cause instanceof Error ? cause.message : 'ดำเนินการไม่สำเร็จ';
		} finally {
			if (!disposed) pending = false;
		}
	}
</script>

<form onsubmit={submit} class="space-y-4" data-testid="leave-action-form">
	<h3 class="font-semibold">
		{leaveActionLabels[action]}{detail.processPurpose === 'cancellation' ? 'การยกเลิกคำขอ' : 'คำขอ'}
	</h3>
	{#if error}<PageState
			variant="error"
			title="ดำเนินการไม่สำเร็จ"
			description={error}
			actionLabel="ตรวจสอบข้อมูลล่าสุด"
			onaction={onrefresh}
		/>{/if}
	{#if stale}<div class="space-y-2 rounded-md border p-3 text-sm">
			<p>คำขอมีการเปลี่ยนแปลง ตรวจสอบรายละเอียดและขั้นตอนล่าสุดก่อนยืนยัน</p>
			<Button
				type="button"
				variant="outline"
				onclick={() => {
					expectedRevision = detail.request.revision;
					expectedProcessRevision = detail.instance?.revision ?? 0;
					confirmed = false;
				}}>ใช้ข้อมูลล่าสุดและเก็บฉบับร่าง</Button
			>
		</div>{/if}
	<div class="space-y-2">
		<Label for="leave-action-comment">ความคิดเห็น / เหตุผล{commentRequired ? ' *' : ''}</Label
		><Textarea id="leave-action-comment" bind:value={comment} rows={3} disabled={pending} />
	</div>
	{#if signing}
		<div class="space-y-4 rounded-xl border bg-muted/20 p-3">
			<p class="text-sm">ขั้นตอนนี้กำหนดให้ลงลายมือชื่อกับคำขอฉบับ {expectedRevision}</p>
			<ToggleGroup.Root
				type="single"
				value={method}
				onValueChange={(value) => {
					if (value === 'drawn' || value === 'image') method = value;
				}}
				disabled={pending}
				><ToggleGroup.Item value="drawn">วาดลายมือชื่อ</ToggleGroup.Item><ToggleGroup.Item
					value="image">อัปโหลดภาพลายมือชื่อ</ToggleGroup.Item
				></ToggleGroup.Root
			>
			{#if method === 'drawn'}<SignaturePad bind:paths disabled={pending} />{:else}<div
					class="space-y-2"
				>
					<Label for="leave-signature-image">ภาพลายมือชื่อสำหรับครั้งนี้ *</Label><Input
						id="leave-signature-image"
						type="file"
						accept="image/png,image/jpeg,image/webp"
						disabled={pending}
						onchange={(event) => {
							image = event.currentTarget.files?.[0] ?? null;
							imageFileId = null;
						}}
					/>{#if image}<p class="text-sm text-muted-foreground">{image.name}</p>{/if}
				</div>{/if}
			<div class="flex items-start gap-2">
				<Checkbox id="leave-signature-consent" bind:checked={confirmed} disabled={pending} /><Label
					for="leave-signature-consent"
					class="leading-relaxed"
					>ยืนยันใช้ลายมือชื่อนี้กับคำขอฉบับ {expectedRevision} และการ{leaveActionLabels[
						action
					]}ครั้งนี้</Label
				>
			</div>
			<p class="text-xs text-muted-foreground">
				ระบบบันทึกผู้ลงนาม เวลา และเนื้อหาฉบับที่ยืนยันเป็นหลักฐาน
			</p>
		</div>
	{/if}
	<div class="flex flex-wrap justify-end gap-2">
		<Button type="button" variant="outline" disabled={pending} onclick={oncancel}>ยกเลิก</Button
		><LoadingButton
			type="submit"
			loading={pending}
			disabled={stale}
			variant={action === 'reject' ? 'destructive' : 'default'}
			>ยืนยัน{leaveActionLabels[action]}</LoadingButton
		>
	</div>
</form>
