<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { SvelteMap, SvelteSet } from 'svelte/reactivity';
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
		type ProcessActionRequest,
		type DirectedRoutingRequest,
		type CorrespondenceOptions
	} from '#lib/api/correspondence.js';
	import { uploadFile } from '#lib/api/files.js';
	import { PendingUpload } from '#lib/files/pending-upload.js';
	import { authStore } from '#lib/stores/auth.js';
	import { hasInk, type InkPath } from '#lib/correspondence/geometry.js';
	import { actionRequirements, validateSigningDraft } from '#lib/correspondence/drafts.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { actionLabels } from '#lib/correspondence/presentation.js';
	import { directedActionLabel } from '#lib/correspondence/directed.js';
	import { LoadingButton, PageState } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import PrivateFileImage from '#lib/components/files/PrivateFileImage.svelte';
	import SignaturePad from './SignaturePad.svelte';
	import Choice from './Choice.svelte';
	import FieldError from './FieldError.svelte';
	import {
		focusCorrespondenceField,
		type CorrespondenceFieldErrors
	} from '#lib/correspondence/validation.js';
	import DirectedRoutingFields from './DirectedRoutingFields.svelte';
	import { MutationIntent } from '#lib/correspondence/mutation-intent.js';
	let {
		detail,
		action,
		assignmentId,
		annotations,
		documentReady,
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
		assignmentId?: string;
		annotations: DocumentAnnotation[];
		documentReady: boolean;
		signature?: SignatureInput;
		oncomplete: (result: CorrespondenceDetail) => void;
		oncancel: () => void;
		onrefresh: () => void;
		onpendingchange?: (pending: boolean) => void;
		ondirtychange?: (dirty: boolean) => void;
		onpreviewchange?: (paths: InkPath[], fileId?: string, localImage?: Blob) => void;
	} = $props();
	let fieldErrors = $state<CorrespondenceFieldErrors>({});
	function invalidField(id: string, message: string) {
		fieldErrors = { [id]: message };
		error = message;
		void focusCorrespondenceField(id);
	}
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
		localImage = $state<PendingUpload | null>(null),
		localImageUrl = $state(''),
		evidence = $state<File[]>([]),
		reassignUserId = $state(''),
		reassignFromUserId = $state(''),
		options = $state<CorrespondenceOptions | null>(null);
	const selectedAssignment = $derived(
		detail.instance?.directed?.assignments.find((item) => item.id === assignmentId)
	);
	const inheritedReviewerNames = $derived.by(() => {
		const assignments = detail.instance?.directed?.assignments ?? [];
		const names: string[] = [];
		const visited = new SvelteSet<string>();
		let current = selectedAssignment;
		while (current && !visited.has(current.id)) {
			visited.add(current.id);
			if (current.reviewer) names.push(current.reviewer.displayName);
			current = assignments.find((item) => item.id === current?.parentId);
		}
		return [...new SvelteSet(names)];
	});
	let routing = $state<DirectedRoutingRequest>({
		recipients: [],
		requireReview: false,
		reviewer: null
	});
	let optionsLoading = $state(false);
	let routingInitialized = false;
	const intent = new MutationIntent<ProcessActionRequest>();
	const uploadedEvidence = new SvelteMap<File, string>();
	let uncertain = $state(false);
	const definition = $derived(detail.instance?.steps[detail.instance.currentStep]?.definition);
	const terminalCopy = $derived(
		selectedAssignment?.required === false && ['completed', 'closed'].includes(detail.record.status)
	);
	const requirements = $derived(
		terminalCopy
			? { acceptsEvidence: false, comment: false, evidence: false, signature: false }
			: actionRequirements(action, definition)
	);
	const pendingRecipients = $derived(
		detail.instance?.steps[detail.instance.currentStep]?.recipients.filter(
			(recipient) => !recipient.completedAt
		) ?? []
	);
	const recovery = $derived(
		action === 'reassign' &&
			!selectedAssignment &&
			!pendingRecipients.some((recipient) => recipient.userId === $authStore.user?.id)
	);
	const commentRequired = $derived(
		requirements.comment ||
			recovery ||
			(action === 'review' &&
				selectedAssignment?.purpose === 'decide' &&
				selectedAssignment.status === 'pending')
	);
	const needsSignature = $derived(requirements.signature);
	let includeSignature = $state(false);
	const signing = $derived(needsSignature || includeSignature);
	const actionLabel = $derived(
		selectedAssignment ? directedActionLabel(action, selectedAssignment) : actionLabels[action]
	);

	let confirmationBasis = '';
	$effect.pre(() => {
		const basis = JSON.stringify([
			annotations,
			signature,
			method,
			assetId,
			localImage?.file,
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
			!!comment ||
				paths.length > 0 ||
				!!assetId ||
				!!localImage ||
				evidence.length > 0 ||
				!!reassignUserId ||
				routing.recipients.length > 0 ||
				!!routing.requireReview
		);
	});
	$effect(() => {
		onpreviewchange?.(
			method === 'drawn' ? paths : [],
			method === 'image' ? assets.find((asset) => asset.id === assetId)?.fileId : undefined,
			method === 'image' ? localImage?.file : undefined
		);
	});
	$effect(() => {
		const image = localImage;
		if (!image) {
			localImageUrl = '';
			return;
		}
		const url = URL.createObjectURL(image.file);
		localImageUrl = url;
		return () => URL.revokeObjectURL(url);
	});
	$effect(() => {
		const userId = $authStore.user?.id;
		const recordId = detail.record.id;
		const selectedAsset = assetId;
		untrack(() => {
			if (
				localImage &&
				(localImage.scope.resourceId !== userId ||
					localImage.scope.owner !== recordId ||
					selectedAsset)
			) {
				localImage.dispose();
				localImage = null;
			}
		});
	});
	let preparedAsset: { image: PendingUpload; asset: SignatureAsset } | undefined;
	let disposed = false;
	const controller = new AbortController();
	onDestroy(() => {
		disposed = true;
		localImage?.dispose();
		controller.abort();
		onpreviewchange?.([], undefined, undefined);
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
	function prepareImage(file: File) {
		if (!$authStore.user || pending) return;
		assetsError = '';
		if (!['image/png', 'image/jpeg', 'image/webp'].includes(file.type) || !file.size) {
			assetsError = 'เลือกภาพ PNG, JPG หรือ WebP ที่มีข้อมูล';
			return;
		}
		localImage?.dispose();
		localImage = new PendingUpload(
			file,
			{
				purpose: 'correspondence_signature',
				resourceId: $authStore.user.id,
				owner: detail.record.id
			},
			uploadFile
		);
		preparedAsset = undefined;
		assetId = '';
		method = 'image';
		confirmed = false;
	}
	async function loadReassign(context = detail.record.register) {
		optionsLoading = true;
		try {
			const result = await getCorrespondenceOptions('', { signal: controller.signal }, context);
			if (!disposed) {
				options = result;
				if (!routingInitialized && action === 'route') {
					routingInitialized = true;
					const defaults = result.routingSettings.find((item) => item.register === context);
					routing = {
						recipients: [],
						requireReview: defaults?.defaultRequireReview ?? false,
						reviewer: defaults?.defaultRequireReview ? defaults.defaultReviewer : null
					};
				}
			}
		} catch (cause) {
			if (!disposed) error = cause instanceof Error ? cause.message : 'โหลดผู้รับไม่สำเร็จ';
		} finally {
			if (!disposed) optionsLoading = false;
		}
	}
	let optionsContext = '';
	$effect(() => {
		const register = detail.record.register,
			context = `${action}:${register}`;
		untrack(() => {
			if (optionsContext === context) return;
			optionsContext = context;
			if (action === 'reassign' || action === 'route') void loadReassign(register);
		});
	});
	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (pending) return;
		error = '';
		fieldErrors = {};
		if (!intent.unresolved) {
			if (commentRequired && !comment.trim()) {
				invalidField('action-comment', 'กรอกความคิดเห็น / เหตุผล');
				return;
			}
			if (requirements.evidence && !evidence.length) {
				invalidField('action-evidence', 'แนบหลักฐานประกอบการดำเนินการ');
				return;
			}
			if (recovery && !reassignFromUserId) {
				invalidField('reassign-source', 'เลือกผู้รับเดิมที่ต้องเปลี่ยน');
				return;
			}
			if (action === 'reassign' && !reassignUserId) {
				invalidField('reassign-user', 'เลือกผู้รับผิดชอบ');
				return;
			}
			if (
				action === 'route' &&
				(!routing.recipients.some((item) => item.required !== false) ||
					(routing.requireReview && !routing.reviewer))
			) {
				invalidField(
					!routing.recipients.some((item) => item.required !== false)
						? 'directed-target-recipients'
						: 'directed-reviewer',
					!routing.recipients.some((item) => item.required !== false)
						? 'เพิ่มผู้รับหลักอย่างน้อยหนึ่งรายการ'
						: 'เลือกผู้ตรวจทาน'
				);
				return;
			}
			if (signing) {
				if (!documentReady) {
					invalidField('signature-input', 'รอเปิดเอกสารให้เรียบร้อยก่อนลงนาม');
					return;
				}
				const invalid = validateSigningDraft(
					signature,
					paths,
					method,
					assetId,
					confirmed,
					detail.document.revision,
					!!localImage
				);
				if (invalid) {
					invalidField(
						!signature || (method === 'drawn' ? !hasInk(paths) : !assetId && !localImage)
							? 'signature-input'
							: !confirmed
								? 'signature-confirm'
								: 'placement-page',
						invalid
					);
					return;
				}
			}
		}
		pending = true;
		onpendingchange?.(true);
		try {
			let signingAssetId = assetId;
			if (signing && method === 'image' && localImage) {
				const image = localImage;
				const fileId = await image.commit();
				if (disposed || image !== localImage) return;
				if (preparedAsset?.image !== image) {
					const asset = await createSignatureAsset(image.file.name, fileId);
					if (disposed || image !== localImage) return;
					preparedAsset = { image, asset };
				}
				signingAssetId = preparedAsset.asset.id;
			}
			const evidenceFileIds: string[] = [];
			for (const file of requirements.acceptsEvidence ? evidence : []) {
				let fileId = uploadedEvidence.get(file);
				if (!fileId) {
					const uploaded = await uploadFile(file, 'correspondence_document', detail.record.id);
					if (disposed) return;
					fileId = uploaded.id;
					uploadedEvidence.set(file, fileId);
				}
				evidenceFileIds.push(fileId);
			}
			const signingInput =
				signing && signature
					? {
							...signature,
							method: method as SignatureInput['method'],
							assetId: method === 'image' ? signingAssetId : undefined,
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
			const body: Omit<ProcessActionRequest, 'requestId'> = {
				assignmentId,
				routing: action === 'route' ? routing : undefined,
				expectedRevision,
				action,
				comment: action === 'read' || terminalCopy ? undefined : comment.trim() || undefined,
				evidenceFileIds,
				signature: signingInput,
				reassignUserId: reassignUserId || undefined,
				reassignFromUserId: recovery ? reassignFromUserId : undefined,
				annotations: requirements.acceptsEvidence
					? annotations.filter((annotation) => !acceptedAnnotationIds.has(annotation.id))
					: []
			};
			const result = await actOnCorrespondence(
				detail.record.id,
				detail.instance?.directed || intent.unresolved
					? intent.begin(body)
					: { ...body, requestId: crypto.randomUUID() }
			);
			intent.accepted();
			if (!disposed) oncomplete(result);
		} catch (cause) {
			intent.rejected(cause);
			uncertain = intent.unresolved;
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
	<h3 class="font-semibold">{actionLabel}</h3>
	{#if selectedAssignment && ['reject', 'return_for_revision'].includes(action)}<p
			class="rounded-lg border border-warning/30 bg-warning/5 p-3 text-sm"
		>
			การดำเนินการนี้มีผลต่อหนังสือทั้งเรื่องและทุกสาขา กรุณาระบุเหตุผลก่อนยืนยัน
		</p>{/if}
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
	{#if uncertain}<p class="text-sm text-warning">
			ยังยืนยันผลการดำเนินการไม่ได้ กดยืนยันอีกครั้งเพื่อตรวจผลด้วยคำขอเดิม
		</p>{/if}
	<fieldset disabled={pending || uncertain} class="min-w-0 space-y-4">
		{#if action === 'route'}
			{#if options}<DirectedRoutingFields
					{fieldErrors}
					{options}
					bind:routing
					register={detail.record.register}
					{inheritedReviewerNames}
					disabled={pending || uncertain}
				/>
			{:else if optionsLoading}<p role="status" class="text-sm text-muted-foreground">
					กำลังโหลดผู้รับคำสั่ง…
				</p>
			{:else}<Button type="button" variant="outline" onclick={() => loadReassign()}
					>โหลดผู้รับคำสั่งอีกครั้ง</Button
				>{/if}
		{/if}
		{#if action !== 'read' && !terminalCopy}<div class="space-y-2">
				<Label for="action-comment">ความคิดเห็น / เหตุผล{commentRequired ? ' *' : ''}</Label
				><Textarea
					id="action-comment"
					aria-invalid={!!fieldErrors['action-comment']}
					aria-describedby={fieldErrors['action-comment'] ? 'action-comment-error' : undefined}
					rows={4}
					bind:value={comment}
					disabled={pending}
				/><FieldError id="action-comment-error" message={fieldErrors['action-comment']} />
			</div>{/if}
		{#if action === 'reassign'}{#if recovery}<Choice
					id="reassign-source"
					error={fieldErrors['reassign-source']}
					label="ผู้รับเดิมที่ต้องเปลี่ยน"
					bind:value={reassignFromUserId}
					options={pendingRecipients.map((recipient) => ({
						value: recipient.userId,
						label: recipient.displayName
					}))}
					disabled={pending}
				/>{/if}{#if options}<Choice
					id="reassign-user"
					error={fieldErrors['reassign-user']}
					label="ผู้รับผิดชอบใหม่"
					bind:value={reassignUserId}
					options={(selectedAssignment ? options.reviewers : options.users)
						.filter((user) =>
							selectedAssignment
								? user.id !==
									(selectedAssignment.status === 'waiting_review'
										? selectedAssignment.reviewer?.userId
										: selectedAssignment.recipient.userId)
								: !pendingRecipients.some((recipient) => recipient.userId === user.id)
						)
						.map((user) => ({ value: user.id, label: user.displayName }))}
					searchable
					disabled={pending}
				/>{:else}<Button type="button" variant="outline" onclick={() => loadReassign()}
					>โหลดผู้รับผิดชอบ</Button
				>{/if}{/if}
		{#if requirements.acceptsEvidence}<div class="space-y-2">
				<Label for="action-evidence">หลักฐานประกอบ{requirements.evidence ? ' *' : ''}</Label><Input
					id="action-evidence"
					aria-invalid={!!fieldErrors['action-evidence']}
					aria-describedby={fieldErrors['action-evidence'] ? 'action-evidence-error' : undefined}
					type="file"
					multiple
					disabled={pending}
					onchange={(event) => {
						evidence = Array.from(event.currentTarget.files ?? []);
					}}
				/><FieldError id="action-evidence-error" message={fieldErrors['action-evidence']} />
			</div>{/if}
		{#if (['approve', 'route'].includes(action) || (action === 'review' && selectedAssignment?.purpose === 'decide')) && !needsSignature && $can.has(PERMISSIONS.CORRESPONDENCE_SIGN_OWN)}<div
				class="flex items-center gap-2"
			>
				<Checkbox id="action-sign" bind:checked={includeSignature} disabled={pending} /><Label
					for="action-sign"
					>{action === 'route'
						? 'ลงนามพร้อมคำสั่ง'
						: selectedAssignment?.purpose === 'decide'
							? 'ลงนามพร้อมผลพิจารณา'
							: 'ลงนามพร้อมอนุมัติ'}</Label
				>
			</div>{/if}
		{#if signing && signature}
			{#if !documentReady && !uncertain}<p role="status" class="text-sm text-muted-foreground">
					กำลังเปิดเอกสารก่อนลงนาม หากเปิดไม่สำเร็จ ให้ลองเปิดอีกครั้งในส่วนเอกสาร
				</p>{/if}
			<div
				hidden={!documentReady && !uncertain}
				id="signature-input"
				role="group"
				aria-label="ลายมือชื่อ"
				tabindex="-1"
				aria-describedby={fieldErrors['signature-input'] ? 'signature-input-error' : undefined}
				class="space-y-4 rounded-xl border bg-muted/20 p-3"
			>
				<FieldError id="signature-input-error" message={fieldErrors['signature-input']} />
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
						onimage={prepareImage}
					/>{:else}
					{#if assetsError}<PageState
							variant="error"
							title={assetsError}
							actionLabel="ลองอีกครั้ง"
							onaction={loadAssets}
						/>{/if}
					<Choice
						id="signature-asset"
						error={fieldErrors['signature-asset']}
						label="ลายมือชื่อของฉัน"
						bind:value={assetId}
						options={assets.map((asset) => ({ value: asset.id, label: asset.label }))}
						disabled={pending || assetsLoading}
					/>
					{#if localImageUrl}<img
							src={localImageUrl}
							alt="ภาพลายมือชื่อที่เลือก"
							class="h-24 w-full rounded-md border bg-white object-contain p-3"
						/>
					{:else if assets.find((asset) => asset.id === assetId)}{@const asset = assets.find(
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
							disabled={pending}
							onchange={(event) => {
								const file = event.currentTarget.files?.[0];
								if (file) prepareImage(file);
								event.currentTarget.value = '';
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
									aria-invalid={!!fieldErrors[`placement-${field.key}`]}
									aria-describedby={fieldErrors[`placement-${field.key}`]
										? `placement-${field.key}-error`
										: undefined}
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
								/><FieldError
									id={`placement-${field.key}-error`}
									message={fieldErrors[`placement-${field.key}`]}
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
						aria-invalid={!!fieldErrors['signature-confirm']}
						aria-describedby={fieldErrors['signature-confirm']
							? 'signature-confirm-error'
							: undefined}
						bind:checked={confirmed}
						disabled={pending}
					/><Label for="signature-confirm" class="leading-relaxed"
						>ยืนยันใช้ลายมือชื่อนี้กับเอกสารฉบับ {signature.documentRevision} และการ{actionLabel}ครั้งนี้</Label
					>
				</div>
				<FieldError id="signature-confirm-error" message={fieldErrors['signature-confirm']} />
				<p class="text-xs text-muted-foreground">
					ระบบบันทึกผู้ลงนาม เวลา และเอกสารที่ใช้ลงนาม เป็นหลักฐานลงลายมือชื่ออิเล็กทรอนิกส์
				</p>
			</div>
		{/if}
	</fieldset>
	<div class="flex flex-wrap justify-end gap-2">
		<Button type="button" variant="outline" onclick={oncancel} disabled={pending}>ยกเลิก</Button
		><LoadingButton
			type="submit"
			loading={pending}
			disabled={signing && !documentReady && !uncertain}
			variant={action === 'reject' ? 'destructive' : 'default'}>ยืนยัน{actionLabel}</LoadingButton
		>
	</div>
</form>
