<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { SvelteMap, SvelteSet } from 'svelte/reactivity';
	import { page } from '$app/state';
	import { authStore } from '#lib/stores/auth.js';
	import * as DropdownMenu from '#lib/components/ui/dropdown-menu/index.js';
	import { buttonVariants } from '#lib/components/ui/button/index.js';
	import DatePicker from '#lib/components/ui/date-picker/DatePicker.svelte';
	import UnsavedGuard from './UnsavedGuard.svelte';
	import {
		getCorrespondence,
		reviseCorrespondence,
		exportCorrespondence,
		type CorrespondenceDetail,
		type ReviseCorrespondenceRequest,
		type DocumentAnnotation,
		type ProcessAction,
		type SignatureInput
	} from '#lib/api/correspondence.js';
	import { downloadFile, uploadFile } from '#lib/api/files.js';
	import type { RouteLoadResult } from '#lib/navigation/route-load.js';
	import { CorrespondenceRegion } from '#lib/correspondence/region.svelte.js';
	import {
		actionLabels,
		registerLabels,
		statusLabels,
		dateLabel,
		saveBlob
	} from '#lib/correspondence/presentation.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageSkeleton, PageState, LoadingButton } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import { can } from '#lib/stores/permissions.js';
	import { workStore } from '#lib/stores/work.js';
	import { historyActionLabel } from '#lib/correspondence/drafts.js';
	import { directedActionLabel } from '#lib/correspondence/directed.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { MutationIntent } from '#lib/correspondence/mutation-intent.js';
	import { validateDocumentMetadata } from '#lib/correspondence/metadata.js';
	import DocumentViewer from './DocumentViewer.svelte';
	import ActionForm from './ActionForm.svelte';
	import SendForm from './SendForm.svelte';
	import Choice from './Choice.svelte';
	let {
		source,
		id
	}: { source: Promise<RouteLoadResult<CorrespondenceDetail | null>>; id: string } = $props();
	const region = new CorrespondenceRegion<CorrespondenceDetail | null>();
	let annotations = $state<DocumentAnnotation[]>([]),
		content = $state(''),
		draftInitialized = false,
		editBaseRevision = $state(0),
		selectedAction = $state<ProcessAction | null>(null),
		selectedAssignmentId = $state<string | undefined>(),
		sending = $state(false),
		signature = $state<SignatureInput | undefined>(),
		error = $state(''),
		savePending = $state(false),
		actionPending = $state(false),
		exportPending = $state(false),
		original = $state<File | null>(null),
		attachments = $state<File[]>([]);
	let metadata = $state({
		title: '',
		sender: '',
		documentNumber: '',
		documentDate: undefined as string | undefined,
		urgency: 'normal' as CorrespondenceDetail['record']['urgency']
	});
	let actionDirty = $state(false),
		sendPending = $state(false);
	let placementPaths = $state<import('#lib/correspondence/geometry.js').InkPath[]>([]),
		placementFileId = $state<string | undefined>();
	const uploadedFiles = new SvelteMap<File, string>();
	const revisionIntent = new MutationIntent<ReviseCorrespondenceRequest>();
	let disposed = false;
	$effect.pre(() => {
		const operation = source;
		untrack(() => region.consume(operation));
	});
	$effect.pre(() => {
		const detail = region.value;
		if (detail && !draftInitialized) {
			untrack(() => {
				draftInitialized = true;
				content = detail.document.content;
				metadata = {
					title: detail.record.title,
					sender: detail.record.sender,
					documentNumber: detail.record.documentNumber ?? '',
					documentDate: detail.record.documentDate ?? undefined,
					urgency: detail.record.urgency
				};
				editBaseRevision = detail.record.revision;
			});
		}
	});
	onDestroy(() => {
		disposed = true;
		region.dispose();
	});
	let observedWorkRevision = $workStore.revision;
	$effect.pre(() => {
		const revision = $workStore.revision;
		untrack(() => {
			if (revision !== observedWorkRevision) {
				observedWorkRevision = revision;
				refresh();
			}
		});
	});
	let selectedRevision = $state('current');
	const detail = $derived(region.value);
	const selectedDocument = $derived(
		selectedRevision === 'current'
			? detail?.document
			: detail?.revisions.find((revision) => revision.revision === Number(selectedRevision))
	);
	const viewingHistory = $derived(
		!!detail && !!selectedDocument && selectedDocument.revision !== detail.document.revision
	);
	const viewerDetail = $derived(
		detail && selectedDocument ? { ...detail, document: selectedDocument } : detail
	);
	const canRevise = $derived(
		!!detail?.canManage && ['draft', 'revision_requested'].includes(detail.record.status)
	);
	const canEditMetadata = $derived(
		!!detail &&
			canRevise &&
			detail.record.status === 'draft' &&
			!detail.instance &&
			detail.record.createdBy === $authStore.user?.id
	);
	const metadataDirty = $derived(
		!!detail &&
			canEditMetadata &&
			(metadata.title !== detail.record.title ||
				metadata.sender !== detail.record.sender ||
				metadata.documentNumber !== (detail.record.documentNumber ?? '') ||
				metadata.documentDate !== (detail.record.documentDate ?? undefined) ||
				metadata.urgency !== detail.record.urgency)
	);
	const primaryAction = $derived(
		detail?.availableActions.find(
			(action) => action === detail.instance?.steps[detail.instance.currentStep]?.definition.kind
		) ??
			detail?.availableActions.find((action) => action === 'close') ??
			detail?.availableActions.find((action) => action === 'read')
	);
	const selectedAssignment = $derived(
		detail?.instance?.directed?.assignments.find((item) => item.id === selectedAssignmentId)
	);
	const terminalCopyAction = $derived(
		selectedAssignment?.required === false &&
			!!detail &&
			['completed', 'closed'].includes(detail.record.status)
	);
	const secondaryActions = $derived(
		detail?.availableActions.filter((action) => action !== primaryAction) ?? []
	);
	const returnHref = $derived.by(() => {
		const value = page.url.searchParams.get('returnTo');
		return value &&
			(value === '/staff/correspondence' || value.startsWith('/staff/correspondence?'))
			? value
			: '/staff/correspondence';
	});
	const dirty = $derived(
		!!detail &&
			(annotations.length > 0 ||
				content !== detail.document.content ||
				!!original ||
				attachments.length > 0 ||
				metadataDirty ||
				actionDirty)
	);
	function recipientName(userId?: string | null): string {
		return (
			[
				...(detail?.instance?.steps.flatMap((step) => step.recipients) ?? []),
				...(detail?.instance?.directed?.assignments.map((assignment) => assignment.recipient) ?? [])
			].find((recipient) => recipient.userId === userId)?.displayName ?? 'ผู้รับ'
		);
	}
	function refresh() {
		region.retry((signal) => getCorrespondence(id, { signal }));
	}
	function complete(result: CorrespondenceDetail) {
		actionPending = false;
		sendPending = false;
		const preserveAnnotations =
			selectedAction !== null &&
			(['read', 'close', 'reassign'].includes(selectedAction) || terminalCopyAction);
		const current = region.value;
		const latest =
			current?.record.id === result.record.id && current.record.revision > result.record.revision
				? current
				: result;
		region.patch(latest);
		selectedAction = null;
		selectedAssignmentId = undefined;
		sending = false;
		if (!preserveAnnotations) annotations = [];
		signature = undefined;
		editBaseRevision = latest.record.revision;
		content = latest.document.content;
		metadata = {
			title: latest.record.title,
			sender: latest.record.sender,
			documentNumber: latest.record.documentNumber ?? '',
			documentDate: latest.record.documentDate ?? undefined,
			urgency: latest.record.urgency
		};
		actionDirty = false;
	}
	async function saveRevision() {
		if (!detail || savePending) return;
		if (canEditMetadata) {
			const invalid = validateDocumentMetadata(metadata);
			if (invalid) {
				error = invalid;
				return;
			}
		}
		savePending = true;
		error = '';
		try {
			let current = detail;
			if (revisionIntent.unresolved) {
				try {
					current = await reviseCorrespondence(
						id,
						revisionIntent.begin({ expectedRevision: editBaseRevision, content })
					);
					revisionIntent.accepted();
					if (disposed) return;
					region.patch(current);
					editBaseRevision = current.record.revision;
					const acceptedIds = new SvelteSet(current.document.annotations.map((item) => item.id));
					annotations = annotations.filter((item) => !acceptedIds.has(item.id));
				} catch (cause) {
					revisionIntent.rejected(cause);
					throw cause;
				}
			}
			let originalFileId = current.document.originalFileId;
			const attachmentFileIds = [...current.document.attachmentFileIds];
			if (original) {
				let fileId = uploadedFiles.get(original);
				if (!fileId) {
					const file = await uploadFile(original, 'correspondence_document', id);
					if (disposed) return;
					fileId = file.id;
					uploadedFiles.set(original, fileId);
				}
				originalFileId = fileId;
			}
			for (const attachment of attachments) {
				let fileId = uploadedFiles.get(attachment);
				if (!fileId) {
					const file = await uploadFile(attachment, 'correspondence_document', id);
					if (disposed) return;
					fileId = file.id;
					uploadedFiles.set(attachment, fileId);
				}
				if (!attachmentFileIds.includes(fileId)) attachmentFileIds.push(fileId);
			}
			const body = {
				expectedRevision: editBaseRevision,
				content,
				originalFileId,
				attachmentFileIds,
				annotations: [
					...current.document.annotations,
					...annotations.filter(
						(item) => !current.document.annotations.some((saved) => saved.id === item.id)
					)
				],
				metadata: canEditMetadata
					? {
							...metadata,
							title: metadata.title.trim(),
							sender: metadata.sender.trim(),
							documentNumber: metadata.documentNumber || null,
							documentDate: metadata.documentDate || null
						}
					: undefined
			};
			const unchanged =
				body.content === current.document.content &&
				(body.originalFileId ?? null) === (current.document.originalFileId ?? null) &&
				JSON.stringify(body.attachmentFileIds) ===
					JSON.stringify(current.document.attachmentFileIds) &&
				JSON.stringify(body.annotations) === JSON.stringify(current.document.annotations) &&
				(!body.metadata ||
					(body.metadata.title === current.record.title &&
						body.metadata.sender === current.record.sender &&
						body.metadata.urgency === current.record.urgency &&
						body.metadata.documentNumber === (current.record.documentNumber ?? null) &&
						body.metadata.documentDate === (current.record.documentDate ?? null)));
			if (unchanged) {
				complete(current);
				original = null;
				attachments = [];
				return;
			}
			const result = await reviseCorrespondence(id, revisionIntent.begin(body));
			revisionIntent.accepted();

			if (!disposed) {
				complete(result);
				original = null;
				attachments = [];
			}
		} catch (cause) {
			revisionIntent.rejected(cause);
			if (!disposed) error = cause instanceof Error ? cause.message : 'บันทึกฉบับแก้ไขไม่สำเร็จ';
		} finally {
			if (!disposed) savePending = false;
		}
	}
	async function downloadExport() {
		if (exportPending) return;
		exportPending = true;
		error = '';
		try {
			const blob = await exportCorrespondence(id);
			if (!disposed) saveBlob(blob, 'หนังสือและหลักฐานลงนาม.pdf');
		} catch (cause) {
			if (!disposed) error = cause instanceof Error ? cause.message : 'ดาวน์โหลดไม่สำเร็จ';
		} finally {
			if (!disposed) exportPending = false;
		}
	}
	async function downloadAttachment(fileId: string) {
		try {
			const blob = await downloadFile(fileId, id);
			if (!disposed) saveBlob(blob, 'เอกสารแนบ');
		} catch (cause) {
			if (!disposed) error = cause instanceof Error ? cause.message : 'ดาวน์โหลดไม่สำเร็จ';
		}
	}
</script>

<PageShell
	title={detail?.record.title ?? 'รายละเอียดหนังสือ'}
	backHref={returnHref}
	description={detail
		? `${registerLabels[detail.record.register]} · ${statusLabels[detail.record.status]}`
		: undefined}
>
	{#snippet actions()}{#if detail && $can.has(PERMISSIONS.CORRESPONDENCE_DOWNLOAD_ASSIGNED)}<LoadingButton
				variant="outline"
				loading={exportPending}
				onclick={downloadExport}>ดาวน์โหลด PDF พร้อมหลักฐาน</LoadingButton
			>{/if}{/snippet}
	<UnsavedGuard {dirty} pending={savePending || actionPending || sendPending} />
	<section data-testid="correspondence-detail" aria-busy={region.loading}>
		{#if region.error}<PageState
				variant="error"
				title={region.error}
				actionLabel="ลองอีกครั้ง"
				onaction={refresh}
			/>{/if}
		{#if region.loading && !detail}<PageSkeleton />{:else if detail}
			{#if error}<div class="mb-4">
					<PageState
						variant="error"
						title={error}
						description="ข้อมูลฉบับร่างยังอยู่"
						actionLabel="ตรวจสอบข้อมูลล่าสุด"
						onaction={refresh}
					/>
				</div>{/if}

			<div class="grid min-w-0 gap-6 xl:grid-cols-[minmax(0,1.7fr)_minmax(20rem,1fr)]">
				<section class="min-w-0 space-y-4" aria-label="เอกสาร" id="correspondence-document">
					<Choice
						id="document-revision"
						label="ฉบับเอกสารที่แสดง"
						bind:value={selectedRevision}
						options={[
							{ value: 'current', label: `ฉบับปัจจุบัน (${detail.document.revision})` },
							...detail.revisions
								.filter((revision) => revision.revision !== detail.document.revision)
								.map((revision) => ({
									value: String(revision.revision),
									label: `ฉบับ ${revision.revision} · ${dateLabel(revision.createdAt)}`
								}))
						]}
					/>
					{#if viewingHistory && viewerDetail}<p
							class="rounded-lg border border-warning/30 bg-warning/5 p-3 text-sm"
						>
							กำลังอ่านเอกสารย้อนหลัง เลือกฉบับปัจจุบันเพื่อดำเนินการหนังสือ
						</p>
						<DocumentViewer detail={viewerDetail} editable={false} />{:else}
						<DocumentViewer
							{detail}
							bind:draft={annotations}
							disabled={savePending || actionPending || sending}
							editable={canRevise ||
								(!(
									detail.instance?.directed &&
									['completed', 'closed'].includes(detail.record.status)
								) &&
									detail.availableActions.some(
										(action) => !['read', 'close', 'reassign'].includes(action)
									))}
							placement={signature}
							{placementPaths}
							{placementFileId}
							onplacementchange={(value) => {
								signature = value;
							}}
							onplacement={(point, page) => {
								if (signature)
									signature = {
										...signature,
										x: Math.min(point.x, 1 - signature.width),
										y: Math.min(point.y, 1 - signature.height),
										page
									};
							}}
						/>
					{/if}
					<div class="flex justify-end">
						<Button
							href={`${page.url.pathname}${page.url.search}#correspondence-action`}
							variant="outline">กลับไปการดำเนินการ</Button
						>
					</div>
					{#if canRevise && !viewingHistory}<Card.Root
							><Card.Header
								><Card.Title>ฉบับร่างแก้ไข</Card.Title><Card.Description
									>บันทึกเป็นฉบับใหม่เพื่อเก็บเอกสารและหลักฐานเดิม</Card.Description
								></Card.Header
							><Card.Content class="space-y-4">
								{#if canEditMetadata}<div class="grid gap-4 sm:grid-cols-2">
										<div class="space-y-2 sm:col-span-2">
											<Label for="draft-title">เรื่อง *</Label><Input
												id="draft-title"
												bind:value={metadata.title}
												disabled={savePending || sending}
												required
											/>
										</div>
										<div class="space-y-2">
											<Label for="draft-sender">ผู้ส่ง / หน่วยงานต้นทาง *</Label><Input
												id="draft-sender"
												bind:value={metadata.sender}
												disabled={savePending || sending}
												required
											/>
										</div>
										<Choice
											id="draft-urgency"
											label="ความเร่งด่วน"
											bind:value={metadata.urgency}
											disabled={savePending || sending}
											options={[
												{ value: 'normal', label: 'ปกติ' },
												{ value: 'urgent', label: 'ด่วน' },
												{ value: 'very_urgent', label: 'ด่วนมาก' }
											]}
										/>
										<div class="space-y-2">
											<Label for="draft-number">เลขที่หนังสือต้นทาง</Label><Input
												id="draft-number"
												bind:value={metadata.documentNumber}
												disabled={savePending || sending}
											/>
										</div>
										<div class="space-y-2">
											<Label for="draft-date">วันที่หนังสือ</Label><DatePicker
												id="draft-date"
												bind:value={metadata.documentDate}
												disabled={savePending || sending}
												clearable
											/>
										</div>
									</div>{/if}
								<div class="space-y-2">
									<Label for="revision-content">เนื้อหาเอกสาร</Label><Textarea
										id="revision-content"
										bind:value={content}
										rows={6}
										disabled={savePending || actionPending || sending}
									/>
								</div>
								<div class="space-y-2">
									<Label for="revision-original">แทนที่เอกสารต้นฉบับ</Label><Input
										id="revision-original"
										type="file"
										accept="application/pdf,image/*"
										disabled={savePending || actionPending || sending}
										onchange={(event) => {
											original = event.currentTarget.files?.[0] ?? null;
										}}
									/>
								</div>
								<div class="space-y-2">
									<Label for="revision-attachments">เพิ่มไฟล์แนบ</Label><Input
										id="revision-attachments"
										type="file"
										multiple
										disabled={savePending || actionPending || sending}
										onchange={(event) => {
											attachments = Array.from(event.currentTarget.files ?? []);
										}}
									/>
								</div>
								{#if editBaseRevision !== detail.record.revision}<p class="text-sm text-warning">
										รายการเปลี่ยนแปลงแล้ว กรุณาตรวจข้อมูลล่าสุดก่อนบันทึก
									</p>
									<Button
										type="button"
										variant="outline"
										onclick={() => {
											editBaseRevision = detail.record.revision;
										}}>ใช้ข้อมูลล่าสุดและเก็บฉบับร่าง</Button
									>{/if}
								<div class="flex justify-end">
									<LoadingButton
										loading={savePending}
										disabled={actionPending || sending}
										onclick={saveRevision}
										>บันทึกฉบับแก้ไข{annotations.length
											? ` (${annotations.length} หมายเหตุ)`
											: ''}</LoadingButton
									>
								</div></Card.Content
							></Card.Root
						>{/if}
				</section>
				<aside
					class="order-first min-w-0 space-y-4 xl:order-last"
					aria-label="ข้อมูลและการดำเนินการ"
				>
					<Card.Root data-testid="correspondence-current-step" id="correspondence-action"
						><Card.Header><Card.Title>การดำเนินการ</Card.Title></Card.Header><Card.Content
							class="space-y-4"
						>
							{#if detail.instance}{@const current =
									detail.instance.steps[detail.instance.currentStep]}{#if current}<div
										class="space-y-2 rounded-lg border border-primary/20 bg-primary/5 p-3"
									>
										<h3 class="font-medium">
											{detail.instance.currentStep + 1}. {current.definition.title}
										</h3>
										<p class="text-sm text-muted-foreground">
											{current.definition.completionPolicy === 'all'
												? 'ผู้รับทุกคนต้องดำเนินการ'
												: 'ผู้รับคนใดคนหนึ่งดำเนินการได้'}{current.dueAt
												? ` · กำหนด ${dateLabel(current.dueAt)}`
												: ''}
										</p>
										<ul class="space-y-2 text-sm" aria-label="ผู้รับขั้นตอนปัจจุบัน">
											{#each current.recipients as recipient, index (`${current.stepIndex}:${index}:${recipient.userId}`)}<li
													class="flex justify-between gap-2"
												>
													<span class="break-words">{recipient.displayName}</span><span
														class="shrink-0 text-muted-foreground"
														>{recipient.completedAt
															? 'ดำเนินการแล้ว'
															: recipient.readAt
																? 'อ่านแล้ว'
																: 'รอดำเนินการ'}</span
													>
												</li>{/each}
										</ul>
									</div>{/if}{/if}
							{#if detail.instance && !detail.instance.directed}<details
									class="rounded-lg border p-3"
									open
								>
									<summary class="cursor-pointer text-sm font-medium"
										>เส้นทางทั้งหมด ({detail.instance.steps.length} ขั้นตอน)</summary
									>
									<ol
										class="mt-3 grid gap-3 sm:grid-cols-2 xl:grid-cols-3"
										aria-label="ลำดับการดำเนินงาน"
									>
										{#each detail.instance.steps as step (step.stepIndex)}{@const current =
												detail.instance.status === 'active' &&
												step.stepIndex === detail.instance.currentStep}
											<li
												class="space-y-1 rounded-lg border p-3 text-sm"
												class:border-primary={current}
												aria-current={current ? 'step' : undefined}
											>
												<p class="font-medium">{step.stepIndex + 1}. {step.definition.title}</p>
												<p class="text-muted-foreground">
													{step.completedAt
														? 'เสร็จแล้ว'
														: current
															? 'ขั้นตอนปัจจุบัน'
															: detail.instance.status === 'revision_requested' &&
																  step.stepIndex === detail.instance.currentStep
																? 'พักรอแก้ไข'
																: 'ยังไม่ถึงขั้นตอน'}
												</p>
												<p class="break-words">
													{step.recipients.map((recipient) => recipient.displayName).join(', ')}
												</p>
												{#if step.dueAt}<p class="text-muted-foreground">
														กำหนด {dateLabel(step.dueAt)}
													</p>{/if}
											</li>{/each}
									</ol>
								</details>{/if}
							{#if detail.instance?.directed}
								<section class="space-y-3" aria-label="สถานะการมอบหมาย">
									<p class="text-sm font-medium">
										{detail.record.status === 'completed'
											? 'ผู้รับที่จำเป็นและการตรวจทานครบแล้ว เรื่องดำเนินการเสร็จสิ้น ไม่ต้องส่งกลับตรวจรับ'
											: 'ติดตามผู้รับแต่ละรายการ เรื่องจะเสร็จเมื่อผู้รับที่จำเป็นและการตรวจทานครบ'}
									</p>
									<ol class="space-y-3" aria-label="สาขาการดำเนินการ">
										{#each detail.instance.directed.assignments as assignment (assignment.id)}
											{@const parentAssignment = detail.instance.directed.assignments.find(
												(item) => item.id === assignment.parentId
											)}
											{@const actions =
												detail.assignmentActions.find((item) => item.assignmentId === assignment.id)
													?.actions ?? []}
											<li class="space-y-2 rounded-lg border p-3 text-sm">
												<p class="break-words font-medium">
													{assignment.recipient.displayName} · {assignment.purpose === 'decide'
														? 'ผู้พิจารณาสั่งการ'
														: !assignment.required
															? 'สำเนาเพื่อทราบ'
															: assignment.purpose === 'execute'
																? 'ให้ดำเนินการ'
																: 'ผู้รับหลักเพื่อทราบ'}
												</p>
												{#if parentAssignment}<p class="break-words text-muted-foreground">
														มอบหมายจาก {parentAssignment.recipient.displayName}
													</p>{/if}
												<p>
													{assignment.status === 'completed'
														? 'งานรายการนี้เสร็จแล้ว'
														: assignment.status === 'waiting_children'
															? 'รอผู้รับที่จำเป็นดำเนินการ'
															: assignment.status === 'waiting_review'
																? 'รอผู้ตรวจทานตรวจผล'
																: assignment.purpose === 'decide'
																	? 'รอพิจารณาสั่งการ'
																	: assignment.purpose === 'execute'
																		? 'รอดำเนินการ'
																		: 'รอรับทราบ'}
												</p>
												{#if assignment.reviewer}<p class="break-words">
														ตรวจทานโดย {assignment.reviewer.displayName}
													</p>{/if}
												{#if assignment.dueAt}<p class="text-muted-foreground">
														กำหนด {dateLabel(assignment.dueAt)}
													</p>{/if}
												{#if assignment.allowOnwardAssignment && assignment.required}<p
														class="text-muted-foreground"
													>
														ผู้รับมอบหมายต่อได้
													</p>{/if}
												{#if actions.length && !selectedAction}<div class="flex flex-wrap gap-2">
														{#each actions as action (action)}<Button
																type="button"
																size="sm"
																variant={action === 'reject'
																	? 'destructive'
																	: ['read', 'reassign', 'report', 'return_for_revision'].includes(
																				action
																		  )
																		? 'outline'
																		: 'default'}
																disabled={viewingHistory || actionPending}
																onclick={() => {
																	selectedAssignmentId = assignment.id;
																	selectedAction = action;
																}}>{directedActionLabel(action, assignment)}</Button
															>{/each}
													</div>{/if}
											</li>
										{/each}
									</ol>
								</section>
							{/if}
							{#if annotations.length && selectedAction && (['read', 'close', 'reassign'].includes(selectedAction) || terminalCopyAction)}<p
									role="status"
									class="rounded-lg border border-warning/30 bg-warning/5 p-3 text-sm"
								>
									หมายเหตุบนเอกสารยังเป็นฉบับร่าง การดำเนินการนี้จะไม่ส่งหมายเหตุ
									เก็บไว้เพื่อส่งพร้อมการดำเนินการที่รองรับ
								</p>{/if}
							<fieldset disabled={viewingHistory} class="min-w-0 space-y-4">
								{#if sending}<SendForm
										onrefresh={refresh}
										onpendingchange={(value) => {
											sendPending = value;
										}}
										{detail}
										oncomplete={complete}
										ondirtychange={(value) => {
											actionDirty = value;
										}}
										oncancel={() => {
											sending = false;
											actionDirty = false;
										}}
									/>{:else if selectedAction}<ActionForm
										{detail}
										action={selectedAction}
										assignmentId={selectedAssignmentId}
										{annotations}
										bind:signature
										oncomplete={complete}
										oncancel={() => {
											selectedAction = null;
											selectedAssignmentId = undefined;
											actionDirty = false;
											signature = undefined;
										}}
										onrefresh={refresh}
										ondirtychange={(value) => {
											actionDirty = value;
										}}
										onpreviewchange={(paths, fileId) => {
											placementPaths = paths;
											placementFileId = fileId;
										}}
										onpendingchange={(value) => {
											actionPending = value;
										}}
									/>{:else}
									{#if canRevise}<Button
											onclick={() => {
												if (
													annotations.length ||
													metadataDirty ||
													content !== detail.document.content ||
													original ||
													attachments.length
												) {
													error = 'บันทึกฉบับแก้ไขก่อนส่งหนังสือ';
													return;
												}
												sending = true;
											}}>เลือกผู้รับและส่งหนังสือ</Button
										>{/if}
									{#if detail.instance?.directed && detail.availableActions.includes('close')}<div
											class="space-y-2 border-t pt-3"
										>
											<p class="text-sm text-muted-foreground">
												ปิดเก็บเรื่องได้ตามต้องการ การดำเนินการนี้เป็นงานจัดเก็บเพิ่มเติม
											</p>
											<Button
												type="button"
												variant="outline"
												onclick={() => {
													selectedAssignmentId = undefined;
													selectedAction = 'close';
												}}>ปิดเก็บเรื่อง (ไม่บังคับ)</Button
											>
										</div>{/if}
									<div class="flex flex-wrap gap-2">
										{#if primaryAction && !detail.instance?.directed}<Button
												onclick={() => {
													selectedAction = primaryAction ?? null;
												}}>{actionLabels[primaryAction]}</Button
											>{/if}
										{#if secondaryActions.length && !detail.instance?.directed}<DropdownMenu.Root
												><DropdownMenu.Trigger
													class={buttonVariants({ variant: 'outline' })}
													disabled={viewingHistory}>เพิ่มเติม</DropdownMenu.Trigger
												><DropdownMenu.Content align="end">
													{#each secondaryActions as action (action)}<DropdownMenu.Item
															variant={action === 'reject' ? 'destructive' : 'default'}
															onSelect={() => {
																selectedAction = action;
															}}>{actionLabels[action]}</DropdownMenu.Item
														>{/each}
												</DropdownMenu.Content></DropdownMenu.Root
											>{/if}
									</div>

									{#if !canRevise && !detail.availableActions.length}<p
											class="text-sm text-muted-foreground"
										>
											ยังไม่มีรายการที่ต้องดำเนินการสำหรับคุณ
										</p>{/if}
								{/if}
							</fieldset>
						</Card.Content></Card.Root
					>
					<Card.Root
						><Card.Header><Card.Title>ข้อมูลหนังสือ</Card.Title></Card.Header><Card.Content
							><dl class="grid grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-3 text-sm">
								<dt class="text-muted-foreground">ทะเบียน</dt>
								<dd>
									{registerLabels[detail.record.register]}
									{detail.record.registerNumber}/{detail.record.registerYear}
								</dd>
								<dt class="text-muted-foreground">เลขที่ต้นทาง</dt>
								<dd class="break-words">{detail.record.documentNumber ?? '—'}</dd>
								<dt class="text-muted-foreground">ผู้ส่ง</dt>
								<dd class="break-words">{detail.record.sender}</dd>
								<dt class="text-muted-foreground">สถานะ</dt>
								<dd><Badge variant="outline">{statusLabels[detail.record.status]}</Badge></dd>
								<dt class="text-muted-foreground">เอกสาร</dt>
								<dd>ฉบับ {detail.document.revision}</dd>
							</dl>
							{#if detail.sourceLinks.length}<ul class="mt-4 space-y-2">
									{#each detail.sourceLinks as link, index (`${index}:${link.url}`)}<li>
											<a
												href={link.url}
												target="_blank"
												rel="noopener noreferrer"
												class="break-words text-sm text-primary underline">{link.label}</a
											>
										</li>{/each}
								</ul>{/if}{#if detail.document.attachmentFileIds.length}<div class="mt-4 space-y-2">
									<h3 class="text-sm font-medium">ไฟล์แนบ</h3>
									{#each detail.document.attachmentFileIds as fileId, index (`${index}:${fileId}`)}<Button
											type="button"
											variant="outline"
											size="sm"
											onclick={() => downloadAttachment(fileId)}>เอกสารแนบ {index + 1}</Button
										>{/each}
								</div>{/if}</Card.Content
						></Card.Root
					>

					{#if detail.signatures.length}<Card.Root
							><Card.Header><Card.Title>หลักฐานลงนาม</Card.Title></Card.Header><Card.Content
								><ul class="space-y-4 text-sm">
									{#each detail.signatures as signed (signed.id)}<li>
											<p class="font-medium">{signed.signerName}</p>
											{#if signed.signerRole}<p>{signed.signerRole}</p>{/if}
											<p class="text-muted-foreground">
												{dateLabel(signed.signedAt)} · เอกสารฉบับ {signed.signature
													.documentRevision}
											</p>
										</li>{/each}
								</ul></Card.Content
							></Card.Root
						>{/if}
					<Card.Root
						><Card.Header><Card.Title>ประวัติการดำเนินการ</Card.Title></Card.Header><Card.Content
							><ol class="space-y-4 text-sm">
								{#each detail.history as entry (entry.id)}<li class="space-y-1 border-l-2 pl-3">
										<p class="font-medium">
											{historyActionLabel(entry.action)} · {entry.actorName}
										</p>
										<p class="text-muted-foreground">{dateLabel(entry.createdAt)}</p>
										{#if entry.reassignedFromUserId && entry.reassignedToUserId}<p>
												เปลี่ยนผู้รับจาก {entry.reassignedFromName ??
													recipientName(entry.reassignedFromUserId)} เป็น {entry.reassignedToName ??
													recipientName(entry.reassignedToUserId)}
											</p>{/if}
										{#if entry.assignmentId || entry.assignmentRecipientName}<p
												class="text-muted-foreground"
											>
												รายการของ {entry.assignmentRecipientName ??
													detail.instance?.directed?.assignments.find(
														(item) => item.id === entry.assignmentId
													)?.recipient.displayName ??
													'ผู้รับ'}
											</p>{/if}
										{#if entry.routedRecipients?.length}<p class="break-words text-sm">
												มอบหมายถึง {entry.routedRecipients
													.map((recipient) => recipient.displayName)
													.join(', ')}
											</p>{/if}
										{#if entry.comment}<p class="whitespace-pre-wrap break-words">
												{entry.comment}
											</p>{/if}{#if entry.annotations.length}<p class="text-muted-foreground">
												{entry.annotations.length} หมายเหตุบนเอกสาร
											</p>{/if}{#each entry.evidenceFileIds as fileId, index (`${index}:${fileId}`)}<Button
												type="button"
												variant="outline"
												size="sm"
												onclick={() => downloadAttachment(fileId)}>หลักฐาน {index + 1}</Button
											>{/each}
									</li>{/each}
							</ol></Card.Content
						></Card.Root
					>
					<details class="rounded-xl border bg-card p-4">
						<summary class="cursor-pointer text-sm font-medium"
							>ประวัติฉบับเอกสาร ({detail.revisions.length})</summary
						>
						<ul class="mt-3 space-y-3 text-sm">
							{#each detail.revisions as revision (revision.revision)}<li>
									<p>ฉบับ {revision.revision} · {dateLabel(revision.createdAt)}</p>
									{#if revision.originalFileId}<Button
											type="button"
											variant="outline"
											size="sm"
											onclick={() => downloadAttachment(revision.originalFileId!)}
											>ดาวน์โหลดต้นฉบับ</Button
										>{/if}
								</li>{/each}
						</ul>
					</details>
				</aside>
			</div>
		{/if}
	</section>
</PageShell>
