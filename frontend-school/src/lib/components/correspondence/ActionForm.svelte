<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { page } from '$app/state';
	import {
		actOnCorrespondence,
		listSignatureAssets,
		createSignatureAsset,
		getCorrespondenceOptions,
		type CorrespondenceDetail,
		type ProcessAction,
		type SignatureAsset,
		type SignatureInput,
		type DocumentAnnotation,
		type CorrespondenceOptions
	} from '#lib/api/correspondence.js';
	import { uploadFile } from '#lib/api/files.js';
	import { authStore } from '#lib/stores/auth.js';
	import type { InkPath } from '#lib/correspondence/geometry.js';
	import { actionRequirements, validateSigningDraft } from '#lib/correspondence/drafts.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { actionLabels } from '#lib/correspondence/presentation.js';
	import { LoadingButton, PageState } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import PrivateFileImage from '#lib/components/files/PrivateFileImage.svelte';
	import SignaturePad from './SignaturePad.svelte';
	import Choice from './Choice.svelte';
	let {
		detail,
		action,
		annotations,
		signature = $bindable(),
		oncomplete,
		oncancel,
		onrefresh,
		onpendingchange,
		ondirtychange,
		onpreviewchange
	}: {
		detail: CorrespondenceDetail;
		action: ProcessAction;
		annotations: DocumentAnnotation[];
		signature?: SignatureInput;
		oncomplete: (result: CorrespondenceDetail) => void;
		oncancel: () => void;
		onrefresh: () => void;
		onpendingchange?: (pending: boolean) => void;
		ondirtychange?: (dirty: boolean) => void;
		onpreviewchange?: (paths: InkPath[], fileId?: string) => void;
	} = $props();
	const initialRevision = untrack(() => detail.record.revision);
	let expectedRevision = $state(initialRevision),
		comment = $state(''),
		error = $state(''),
		pending = $state(false),
		confirmed = $state(false),
		paths = $state<InkPath[]>([]),
		method = $state('drawn'),
		assetId = $state(''),
		assets = $state<SignatureAsset[]>([]),
		assetsError = $state(''),
		assetsLoading = $state(false),
		imagePending = $state(false),
		evidence = $state<File[]>([]),
		reassignUserId = $state(''),
		reassignFromUserId = $state(''),
		options = $state<CorrespondenceOptions | null>(null);
	const definition = $derived(detail.instance?.steps[detail.instance.currentStep]?.definition);
	const requirements = $derived(actionRequirements(action, definition));
	const pendingRecipients = $derived(
		detail.instance?.steps[detail.instance.currentStep]?.recipients.filter(
			(recipient) => !recipient.completedAt
		) ?? []
	);
	const recovery = $derived(
		action === 'reassign' &&
			!pendingRecipients.some((recipient) => recipient.userId === $authStore.user?.id)
	);
	const commentRequired = $derived(requirements.comment || recovery);
	const needsSignature = $derived(requirements.signature);
	let includeSignature = $state(false);
	const signing = $derived(needsSignature || includeSignature);
	let confirmationBasis = '';
	$effect.pre(() => {
		const basis = JSON.stringify([
			signature,
			method,
			assetId,
			paths,
			detail.document.revision,
			detail.document.contentHash
		]);
		untrack(() => {
			if (basis !== confirmationBasis) {
				confirmationBasis = basis;
				confirmed = false;
			}
		});
	});
	$effect(() => {
		ondirtychange?.(
			!!comment || paths.length > 0 || !!assetId || evidence.length > 0 || !!reassignUserId
		);
	});
	$effect(() => {
		onpreviewchange?.(
			method === 'drawn' ? paths : [],
			method === 'image' ? assets.find((asset) => asset.id === assetId)?.fileId : undefined
		);
	});
	let disposed = false;
	const controller = new AbortController();
	onDestroy(() => {
		disposed = true;
		controller.abort();
		signature = undefined;
	});
	$effect.pre(() => {
		const enabled = signing;
		untrack(() => {
			if (enabled && !signature)
				signature = {
					method: 'drawn',
					strokes: [],
					page: 1,
					x: 0.6,
					y: 0.75,
					width: 0.25,
					height: 0.1,
					documentRevision: detail.document.revision,
					contentHash: detail.document.contentHash
				};
			else if (!enabled) signature = undefined;
		});
	});
	async function loadAssets() {
		assetsLoading = true;
		assetsError = '';
		try {
			const result = await listSignatureAssets({ signal: controller.signal });
			if (!disposed) assets = result;
		} catch (cause) {
			if (!disposed)
				assetsError = cause instanceof Error ? cause.message : 'โหลดลายมือชื่อไม่สำเร็จ';
		} finally {
			if (!disposed) assetsLoading = false;
		}
	}
	async function selectMethod(value: string) {
		method = value;
		confirmed = false;
		if (value === 'image' && !assets.length) await loadAssets();
	}
	async function uploadImage(file: File) {
		if (!$authStore.user || imagePending) return;
		imagePending = true;
		assetsError = '';
		try {
			const uploaded = await uploadFile(file, 'correspondence_signature', $authStore.user.id);
			if (disposed) return;
			const asset = await createSignatureAsset(file.name, uploaded.id);
			if (disposed) return;
			assets = [asset, ...assets];
			assetId = asset.id;
			method = 'image';
			confirmed = false;
		} catch (cause) {
			if (!disposed)
				assetsError = cause instanceof Error ? cause.message : 'อัปโหลดลายมือชื่อไม่สำเร็จ';
		} finally {
			if (!disposed) imagePending = false;
		}
	}
	async function loadReassign() {
		try {
			const result = await getCorrespondenceOptions('', { signal: controller.signal });
			if (!disposed) options = result;
		} catch (cause) {
			if (!disposed) error = cause instanceof Error ? cause.message : 'โหลดผู้รับไม่สำเร็จ';
		}
	}
	$effect(() => {
		if (action === 'reassign') void loadReassign();
	});
	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (pending) return;
		error = '';
		if (commentRequired && !comment.trim()) {
			error = 'กรอกความคิดเห็น / เหตุผล';
			return;
		}
		if (requirements.evidence && !evidence.length) {
			error = 'แนบหลักฐานประกอบการดำเนินการ';
			return;
		}
		if (recovery && !reassignFromUserId) {
			error = 'เลือกผู้รับเดิมที่ต้องเปลี่ยน';
			return;
		}
		if (action === 'reassign' && !reassignUserId) {
			error = 'เลือกผู้รับผิดชอบ';
			return;
		}
		if (signing) {
			const invalid = validateSigningDraft(
				signature,
				paths,
				method,
				assetId,
				confirmed,
				detail.document.revision
			);
			if (invalid) {
				error = invalid;
				return;
			}
		}
		pending = true;
		onpendingchange?.(true);
		try {
			const evidenceFileIds: string[] = [];
			for (const file of requirements.acceptsEvidence ? evidence : []) {
				const uploaded = await uploadFile(file, 'correspondence_document', detail.record.id);
				if (disposed) return;
				evidenceFileIds.push(uploaded.id);
			}
			const signingInput =
				signing && signature
					? {
							...signature,
							method: method as SignatureInput['method'],
							assetId: method === 'image' ? assetId : undefined,
							strokes:
								method === 'drawn'
									? paths.map((path) => ({ ...path, width: 0.006, color: '#111827' }))
									: []
						}
					: undefined;
			const acceptedAnnotationIds = new Set(
				[
					...detail.document.annotations,
					...detail.history
						.filter((entry) => entry.documentRevision === detail.document.revision)
						.flatMap((entry) => entry.annotations)
				].map((annotation) => annotation.id)
			);
			const result = await actOnCorrespondence(detail.record.id, {
				requestId: crypto.randomUUID(),
				expectedRevision,
				action,
				comment: action === 'read' ? undefined : comment.trim() || undefined,
				evidenceFileIds,
				signature: signingInput,
				reassignUserId: reassignUserId || undefined,
				reassignFromUserId: recovery ? reassignFromUserId : undefined,
				annotations: requirements.acceptsEvidence
					? annotations.filter((annotation) => !acceptedAnnotationIds.has(annotation.id))
					: []
			});
			if (!disposed) oncomplete(result);
		} catch (cause) {
			if (!disposed) error = cause instanceof Error ? cause.message : 'ดำเนินการไม่สำเร็จ';
		} finally {
			if (!disposed) {
				pending = false;
				onpendingchange?.(false);
			}
		}
	}
</script>

<form class="space-y-4" onsubmit={submit} data-testid="correspondence-action">
	<h3 class="font-semibold">{actionLabels[action]}</h3>
	{#if error}<PageState
			variant="error"
			title={error}
			description="เก็บข้อความ ลายมือชื่อ และหมายเหตุฉบับร่างไว้แล้ว"
			actionLabel="ตรวจสอบข้อมูลล่าสุด"
			onaction={onrefresh}
		/>{/if}
	{#if expectedRevision !== detail.record.revision}<div
			class="space-y-2 rounded-md border p-3 text-sm"
		>
			<p>รายการมีการเปลี่ยนแปลง กรุณาตรวจเอกสารและขั้นตอนล่าสุด</p>
			<Button
				type="button"
				variant="outline"
				onclick={() => {
					expectedRevision = detail.record.revision;
					confirmed = false;
					if (signature)
						signature = {
							...signature,
							documentRevision: detail.document.revision,
							contentHash: detail.document.contentHash
						};
				}}>ใช้ข้อมูลล่าสุดและเก็บฉบับร่าง</Button
			>
		</div>{/if}
	{#if action !== 'read'}<div class="space-y-2">
			<Label for="action-comment">ความคิดเห็น / เหตุผล{commentRequired ? ' *' : ''}</Label><Textarea
				id="action-comment"
				rows={4}
				bind:value={comment}
				disabled={pending}
			/>
		</div>{/if}
	{#if action === 'reassign'}{#if recovery}<Choice
				id="reassign-source"
				label="ผู้รับเดิมที่ต้องเปลี่ยน"
				bind:value={reassignFromUserId}
				options={pendingRecipients.map((recipient) => ({
					value: recipient.userId,
					label: recipient.displayName
				}))}
				disabled={pending}
			/>{/if}{#if options}<Choice
				id="reassign-user"
				label="ผู้รับผิดชอบใหม่"
				bind:value={reassignUserId}
				options={options.users
					.filter((user) => !pendingRecipients.some((recipient) => recipient.userId === user.id))
					.map((user) => ({ value: user.id, label: user.displayName }))}
				searchable
				disabled={pending}
			/>{:else}<Button type="button" variant="outline" onclick={loadReassign}
				>โหลดผู้รับผิดชอบ</Button
			>{/if}{/if}
	{#if requirements.acceptsEvidence}<div class="space-y-2">
			<Label for="action-evidence">หลักฐานประกอบ{requirements.evidence ? ' *' : ''}</Label><Input
				id="action-evidence"
				type="file"
				multiple
				disabled={pending}
				onchange={(event) => {
					evidence = Array.from(event.currentTarget.files ?? []);
				}}
			/>
		</div>{/if}
	{#if action === 'approve' && !needsSignature && $can.has(PERMISSIONS.CORRESPONDENCE_SIGN_OWN)}<div
			class="flex items-center gap-2"
		>
			<Checkbox id="action-sign" bind:checked={includeSignature} disabled={pending} /><Label
				for="action-sign">ลงนามพร้อมอนุมัติ</Label
			>
		</div>{/if}
	{#if signing && signature}
		<div class="space-y-4 rounded-xl border bg-muted/20 p-3">
			<div class="flex gap-2">
				<Button
					type="button"
					variant={method === 'drawn' ? 'secondary' : 'outline'}
					size="sm"
					onclick={() => selectMethod('drawn')}>วาดใหม่</Button
				><Button
					type="button"
					variant={method === 'image' ? 'secondary' : 'outline'}
					size="sm"
					onclick={() => selectMethod('image')}>เลือกลายมือชื่อที่บันทึก</Button
				>
			</div>
			{#if method === 'drawn'}<SignaturePad
					bind:paths
					disabled={pending}
					onimage={uploadImage}
				/>{:else}
				{#if assetsError}<PageState
						variant="error"
						title={assetsError}
						actionLabel="ลองอีกครั้ง"
						onaction={loadAssets}
					/>{/if}
				<Choice
					id="signature-asset"
					label="ลายมือชื่อของฉัน"
					bind:value={assetId}
					options={assets.map((asset) => ({ value: asset.id, label: asset.label }))}
					disabled={pending || assetsLoading}
				/>
				{#if assets.find((asset) => asset.id === assetId)}{@const asset = assets.find(
						(asset) => asset.id === assetId
					)!}
					<div class="rounded-md border bg-white p-3">
						<PrivateFileImage
							fileId={asset.fileId}
							resourceId={$authStore.user?.id}
							alt="ภาพลายมือชื่อที่เลือก"
							class="h-24 w-full object-contain"
						/>
					</div>{/if}
				<div class="space-y-2">
					<Label for="action-signature-image">เพิ่มภาพลายมือชื่อ</Label><Input
						id="action-signature-image"
						type="file"
						accept="image/png,image/jpeg,image/webp"
						disabled={pending || imagePending}
						onchange={(event) => {
							const file = event.currentTarget.files?.[0];
							if (file) void uploadImage(file);
						}}
					/>
				</div>
			{/if}
			<Button
				href={`${page.url.pathname}${page.url.search}#correspondence-document`}
				variant="outline">วางและปรับขนาดบนเอกสาร</Button
			>
			<details>
				<summary class="cursor-pointer text-sm font-medium"
					>ปรับตำแหน่งละเอียด / ใช้แป้นพิมพ์</summary
				>
				<div class="mt-3 grid grid-cols-2 gap-3 sm:grid-cols-3">
					{#each [{ key: 'page', label: 'หน้า', min: 1, max: 999, step: 1 }, { key: 'x', label: 'จากซ้าย (%)', min: 0, max: 100, step: 1 }, { key: 'y', label: 'จากบน (%)', min: 0, max: 100, step: 1 }, { key: 'width', label: 'ความกว้าง (%)', min: 1, max: 100, step: 1 }, { key: 'height', label: 'ความสูง (%)', min: 1, max: 100, step: 1 }] as field (field.key)}<div
							class="space-y-2"
						>
							<Label for={`placement-${field.key}`}>{field.label}</Label><Input
								id={`placement-${field.key}`}
								type="number"
								min={field.min}
								max={field.max}
								step={field.key === 'page' ? 1 : 'any'}
								value={signature[field.key as 'page' | 'x' | 'y' | 'width' | 'height'] *
									(field.key === 'page' ? 1 : 100)}
								disabled={pending}
								oninput={(event) => {
									if (signature)
										signature = {
											...signature,
											[field.key]:
												Number(event.currentTarget.value) / (field.key === 'page' ? 1 : 100)
										};
									confirmed = false;
								}}
							/>
						</div>{/each}
				</div>
			</details>
			<p class="text-xs text-muted-foreground">
				ปรับตำแหน่งและขนาดเป็นเปอร์เซ็นต์ของหน้า หรือแตะบนเอกสารเพื่อวางลายมือชื่อ
			</p>
			<div class="flex items-start gap-2">
				<Checkbox
					id="signature-confirm"
					bind:checked={confirmed}
					disabled={pending || imagePending}
				/><Label for="signature-confirm" class="leading-relaxed"
					>ยืนยันใช้ลายมือชื่อนี้กับเอกสารฉบับ {signature.documentRevision} และการ{actionLabels[
						action
					]}ครั้งนี้</Label
				>
			</div>
			<p class="text-xs text-muted-foreground">
				ระบบบันทึกผู้ลงนาม เวลา และเอกสารที่ใช้ลงนาม เป็นหลักฐานลงลายมือชื่ออิเล็กทรอนิกส์
			</p>
		</div>
	{/if}
	<div class="flex flex-wrap justify-end gap-2">
		<Button type="button" variant="outline" onclick={oncancel} disabled={pending}>ยกเลิก</Button
		><LoadingButton type="submit" loading={pending} disabled={imagePending}
			>ยืนยัน{actionLabels[action]}</LoadingButton
		>
	</div>
</form>
