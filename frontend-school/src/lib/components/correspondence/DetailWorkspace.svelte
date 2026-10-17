<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import {
		getCorrespondence,
		reviseCorrespondence,
		exportCorrespondence,
		type CorrespondenceDetail,
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
	import { PERMISSIONS } from '#lib/permissions/registry.js';
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
		sending = $state(false),
		signature = $state<SignatureInput | undefined>(),
		error = $state(''),
		savePending = $state(false),
		actionPending = $state(false),
		exportPending = $state(false),
		original = $state<File | null>(null),
		attachments = $state<File[]>([]);
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
	function recipientName(userId?: string | null): string {
		return (
			detail?.instance?.steps
				.flatMap((step) => step.recipients)
				.find((recipient) => recipient.userId === userId)?.displayName ?? 'ผู้รับ'
		);
	}
	function refresh() {
		region.retry((signal) => getCorrespondence(id, { signal }));
	}
	function complete(result: CorrespondenceDetail) {
		actionPending = false;
		const preserveAnnotations =
			selectedAction !== null && ['read', 'close', 'reassign'].includes(selectedAction);
		const current = region.value;
		const latest =
			current?.record.id === result.record.id && current.record.revision > result.record.revision
				? current
				: result;
		region.patch(latest);
		selectedAction = null;
		sending = false;
		if (!preserveAnnotations) annotations = [];
		signature = undefined;
		editBaseRevision = latest.record.revision;
		content = latest.document.content;
	}
	async function saveRevision() {
		if (!detail || savePending) return;
		savePending = true;
		error = '';
		try {
			let originalFileId = detail.document.originalFileId;
			const attachmentFileIds = [...detail.document.attachmentFileIds];
			if (original) {
				const file = await uploadFile(original, 'correspondence_document', id);
				if (disposed) return;
				originalFileId = file.id;
			}
			for (const attachment of attachments) {
				const file = await uploadFile(attachment, 'correspondence_document', id);
				if (disposed) return;
				attachmentFileIds.push(file.id);
			}
			const result = await reviseCorrespondence(id, {
				requestId: crypto.randomUUID(),
				expectedRevision: editBaseRevision,
				content,
				originalFileId,
				attachmentFileIds,
				annotations: [...detail.document.annotations, ...annotations]
			});
			if (!disposed) {
				complete(result);
				original = null;
				attachments = [];
			}
		} catch (cause) {
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
	backHref="/staff/correspondence"
	description={detail
		? `${registerLabels[detail.record.register]} · ${statusLabels[detail.record.status]}`
		: undefined}
>
	{#snippet actions()}{#if detail && $can.has(PERMISSIONS.CORRESPONDENCE_DOWNLOAD_ASSIGNED)}<LoadingButton
				variant="outline"
				loading={exportPending}
				onclick={downloadExport}>ดาวน์โหลด PDF พร้อมหลักฐาน</LoadingButton
			>{/if}{/snippet}
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
				<section class="min-w-0 space-y-4" aria-label="เอกสาร">
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
								detail.availableActions.some(
									(action) => !['read', 'close', 'reassign'].includes(action)
								)}
							placement={signature}
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
					{#if canRevise && !viewingHistory}<Card.Root
							><Card.Header
								><Card.Title>ฉบับร่างแก้ไข</Card.Title><Card.Description
									>บันทึกเป็นฉบับใหม่เพื่อเก็บเอกสารและหลักฐานเดิม</Card.Description
								></Card.Header
							><Card.Content class="space-y-4"
								><div class="space-y-2">
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
				<aside class="min-w-0 space-y-4" aria-label="ข้อมูลและการดำเนินการ">
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
					<Card.Root data-testid="correspondence-current-step"
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
										<ul class="space-y-2 text-sm">
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
							<fieldset disabled={viewingHistory} class="min-w-0 space-y-4">
								{#if sending}<SendForm
										{detail}
										oncomplete={complete}
										oncancel={() => {
											sending = false;
										}}
									/>{:else if selectedAction}<ActionForm
										{detail}
										action={selectedAction}
										{annotations}
										bind:signature
										oncomplete={complete}
										oncancel={() => {
											selectedAction = null;
											signature = undefined;
										}}
										onrefresh={refresh}
										onpendingchange={(value) => {
											actionPending = value;
										}}
									/>{:else}
									{#if canRevise}<Button
											onclick={() => {
												if (
													annotations.length ||
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
									<div class="flex flex-wrap gap-2">
										{#each detail.availableActions as action (action)}<Button
												variant={action === 'reject'
													? 'destructive'
													: action === 'return_for_revision' ||
														  action === 'reassign' ||
														  action === 'read'
														? 'outline'
														: 'default'}
												onclick={() => {
													selectedAction = action;
												}}>{actionLabels[action]}</Button
											>{/each}
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
												เปลี่ยนผู้รับจาก {recipientName(entry.reassignedFromUserId)} เป็น {recipientName(
													entry.reassignedToUserId
												)}
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
