<script lang="ts">
	import { onDestroy } from 'svelte';
	import { page } from '$app/state';
	import {
		getLeaveRequest,
		submitLeaveRequest,
		cancelLeaveRequest,
		type LeaveRequestDetail,
		type ProcessAction
	} from '#lib/api/staff-leave.js';
	import { downloadFile } from '#lib/api/files.js';
	import type { RouteLoadResult } from '#lib/navigation/route-load.js';
	import { LeaveRegion } from '#lib/staff-leave/region.svelte.js';
	import { LeaveMutationIntent } from '#lib/staff-leave/mutation-intent.js';
	import {
		leaveActionLabels,
		leaveHistoryLabel,
		leaveStatusClass,
		leaveHome,
		leaveAmount,
		leaveDate,
		leaveDateTime,
		leaveStatusLabels,
		leaveStatusVariant
	} from '#lib/staff-leave/presentation.js';
	import { safeLeaveReturn } from '#lib/staff-leave/filters.js';
	import { PERSONNEL_TYPE_LABELS } from '#lib/forms/staff-career.js';
	import { pathData } from '#lib/correspondence/geometry.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageSkeleton, PageState, LoadingButton } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import PrivateFileImage from '#lib/components/files/PrivateFileImage.svelte';
	import RequestEditor from './RequestEditor.svelte';
	import LeaveActionForm from './LeaveActionForm.svelte';
	let { source, id }: { source: Promise<RouteLoadResult<LeaveRequestDetail | null>>; id: string } =
		$props();
	const region = new LeaveRegion<LeaveRequestDetail | null>(),
		submitIntent = new LeaveMutationIntent(),
		cancelIntent = new LeaveMutationIntent();
	let editing = $state(false),
		cancelling = $state(false),
		cancelReason = $state(''),
		action = $state<ProcessAction | null>(null),
		pending = $state(''),
		error = $state('');
	let downloadPending = $state<string | null>(null),
		disposed = false;
	const detail = $derived(region.value);
	const backHref = $derived(
		safeLeaveReturn(
			page.url.searchParams.get('returnTo'),
			leaveHome(detail?.request.kind ?? 'leave')
		)
	);
	const creator = $derived($can.has(PERMISSIONS.STAFF_LEAVE_CREATE_OWN));
	const reviewer = $derived(
		$can.hasAny(PERMISSIONS.STAFF_LEAVE_UPDATE_ASSIGNED, PERMISSIONS.STAFF_LEAVE_MANAGE_SCHOOL)
	);
	const canPrint = $derived(
		$can.hasAny(PERMISSIONS.STAFF_LEAVE_DOWNLOAD_OWN, PERMISSIONS.STAFF_LEAVE_DOWNLOAD_SCHOOL)
	);
	$effect.pre(() => region.consume(source));
	onDestroy(() => {
		disposed = true;
		region.dispose();
	});
	function refreshed() {
		region.retry((signal) => getLeaveRequest(id, { signal }));
	}
	async function submit() {
		if (!detail || pending || !creator) return;
		pending = 'submit';
		error = '';
		const basis = { expectedRevision: detail.request.revision };
		try {
			const result = await submitLeaveRequest(id, { requestId: submitIntent.for(basis), ...basis });
			if (!disposed) region.patch(result);
		} catch (cause) {
			if (!disposed) error = cause instanceof Error ? cause.message : 'ส่งคำขอไม่สำเร็จ';
		} finally {
			if (!disposed) pending = '';
		}
	}
	async function cancel(event: SubmitEvent) {
		event.preventDefault();
		if (!detail || pending || !creator) return;
		if (!cancelReason.trim()) {
			error = 'กรอกเหตุผลการยกเลิก';
			return;
		}
		pending = 'cancel';
		error = '';
		const basis = { expectedRevision: detail.request.revision, reason: cancelReason.trim() };
		try {
			const result = await cancelLeaveRequest(id, { requestId: cancelIntent.for(basis), ...basis });
			if (!disposed) {
				region.patch(result);
				cancelling = false;
				cancelReason = '';
			}
		} catch (cause) {
			if (!disposed) error = cause instanceof Error ? cause.message : 'ยกเลิกคำขอไม่สำเร็จ';
		} finally {
			if (!disposed) pending = '';
		}
	}
	async function download(fileId: string, index: number) {
		if (downloadPending) return;
		downloadPending = fileId;
		error = '';
		try {
			const blob = await downloadFile(fileId, id);
			if (disposed) return;
			const url = URL.createObjectURL(blob);
			const anchor = document.createElement('a');
			anchor.href = url;
			anchor.download = `หลักฐาน-${index + 1}`;
			anchor.click();
			URL.revokeObjectURL(url);
		} catch (cause) {
			if (!disposed) error = cause instanceof Error ? cause.message : 'ดาวน์โหลดหลักฐานไม่สำเร็จ';
		} finally {
			if (!disposed) downloadPending = null;
		}
	}
	function completed(result: LeaveRequestDetail) {
		region.patch(result);
		action = null;
	}
</script>

<PageShell
	contentClass="space-y-4"
	title={detail?.request.typeName ?? 'รายละเอียดคำขอ'}
	{backHref}
	backLabel="กลับรายการคำขอ"
>
	{#snippet actions()}
		{#if detail}
			{#if detail.canEdit && creator}<Button
					variant="outline"
					disabled={!!pending}
					onclick={() => (editing = true)}>แก้ไขฉบับร่าง</Button
				>{/if}
			{#if detail.canSubmit && creator}<LoadingButton
					loading={pending === 'submit'}
					disabled={!!pending}
					onclick={submit}
					>{detail.request.status === 'returned'
						? 'ส่งคำขออีกครั้ง'
						: 'ส่งคำขอพิจารณา'}</LoadingButton
				>{/if}
			{#if detail.canCancel && creator}<Button
					variant="outline"
					disabled={!!pending}
					onclick={() => {
						error = '';
						cancelling = true;
					}}>{detail.request.status === 'approved' ? 'ขอยกเลิกคำขอ' : 'ถอนคำขอ'}</Button
				>{/if}
			{#if canPrint}<Button variant="outline" onclick={() => window.print()}>พิมพ์คำขอ</Button>{/if}
		{/if}
	{/snippet}
	<section aria-busy={region.loading} data-testid="leave-detail">
		{#if region.error}<PageState
				variant="error"
				title="โหลดรายละเอียดไม่สำเร็จ"
				description={region.error}
				actionLabel="ลองอีกครั้ง"
				onaction={refreshed}
			/>{/if}
		{#if error && !cancelling}<PageState
				variant="error"
				title="ดำเนินการไม่สำเร็จ"
				description={error}
				actionLabel="ตรวจสอบข้อมูลล่าสุด"
				onaction={refreshed}
			/>{/if}
		{#if region.loading && !detail}<PageSkeleton variant="detail" />{:else if detail}
			<div id="staff-leave-print" class="leave-layout">
				<h2 class="hidden print:block text-lg font-semibold">{detail.request.typeName}</h2>
				<div class="leave-primary">
					<Card.Root class="leave-summary gap-3 py-4 border-primary/20 bg-primary/5"
						><Card.Header class="px-4"
							><div class="flex flex-wrap items-center justify-between gap-3">
								<p class="text-sm font-medium">ข้อมูลคำขอ</p>
								<Badge
									variant={leaveStatusVariant(detail.request.status)}
									class={leaveStatusClass(detail.request.status)}
									>{leaveStatusLabels[detail.request.status]}</Badge
								>
							</div></Card.Header
						><Card.Content class="px-4"
							><dl class="grid gap-3 sm:grid-cols-2">
								<div>
									<dt class="text-sm text-muted-foreground">ผู้ขอ</dt>
									<dd class="break-words font-medium">{detail.request.ownerName}</dd>
								</div>
								<div>
									<dt class="text-sm text-muted-foreground">ประเภทบุคลากรที่ใช้พิจารณา</dt>
									<dd>
										{detail.request.personnelType
											? PERSONNEL_TYPE_LABELS[detail.request.personnelType]
											: 'ยังไม่มีข้อมูลประเภทบุคลากร'}
									</dd>
								</div>
								<div>
									<dt class="text-sm text-muted-foreground">ช่วงวันที่</dt>
									<dd>
										{leaveDate(detail.request.startDate)}{detail.request.endDate !==
										detail.request.startDate
											? ` – ${leaveDate(detail.request.endDate)}`
											: ''}{detail.request.dayPortion === 'morning'
											? ' · ช่วงเช้า'
											: detail.request.dayPortion === 'afternoon'
												? ' · ช่วงบ่าย'
												: ''}
									</dd>
									{#if detail.request.startTime}<dd>
											{detail.request.startTime.slice(0, 5)} – {detail.request.endTime?.slice(0, 5)}
										</dd>{/if}
								</div>
								<div>
									<dt class="text-sm text-muted-foreground">จำนวนที่ขอ</dt>
									<dd class="font-medium">
										{leaveAmount(detail.request.amount, detail.request.unit)}
									</dd>
								</div>
								<div class="sm:col-span-2">
									<dt class="text-sm text-muted-foreground">เหตุผล</dt>
									<dd class="break-words whitespace-pre-wrap">{detail.request.reason}</dd>
								</div>
							</dl></Card.Content
						></Card.Root
					>
				</div>
				<aside class="leave-side space-y-4">
					{#if detail.canAct && reviewer && detail.availableActions.length}<Card.Root
							class="leave-review gap-3 py-4 print:hidden"
							><Card.Header class="px-4"
								><Card.Title>ดำเนินการคำขอ</Card.Title>{#if detail.instance}<Card.Description
										>{detail.instance.steps[detail.instance.currentStep]?.definition.title ??
											'ตรวจสอบข้อมูลก่อนยืนยัน'}</Card.Description
									>{/if}</Card.Header
							><Card.Content class="px-4"
								>{#if action}<LeaveActionForm
										{detail}
										{action}
										oncomplete={completed}
										oncancel={() => (action = null)}
										onrefresh={refreshed}
									/>{:else}<div class="flex flex-wrap gap-2">
										{#each detail.availableActions as choice (choice)}<Button
												variant={choice === 'reject'
													? 'destructive'
													: choice === 'return_for_revision'
														? 'outline'
														: 'default'}
												onclick={() => (action = choice)}>{leaveActionLabels[choice]}</Button
											>{/each}
									</div>{/if}</Card.Content
							></Card.Root
						>{/if}
					{#if detail.instance}<Card.Root class="leave-process gap-3 py-4"
							><Card.Header class="px-4"
								><Card.Title
									>{detail.processPurpose === 'cancellation'
										? 'ขั้นตอนพิจารณาการยกเลิก'
										: 'ขั้นตอนพิจารณา'}</Card.Title
								></Card.Header
							><Card.Content class="px-4"
								><ol class="space-y-2">
									{#each detail.instance.steps as step, index (step.stepIndex)}<li
											class="rounded-md border p-2.5"
										>
											<div class="flex flex-wrap justify-between gap-2">
												<p class="font-medium">{index + 1}. {step.definition.title}</p>
												<Badge
													variant={step.completedAt
														? 'default'
														: index === detail.instance.currentStep
															? 'secondary'
															: 'outline'}
													>{step.completedAt
														? 'เสร็จแล้ว'
														: index === detail.instance.currentStep
															? 'ขั้นตอนปัจจุบัน'
															: 'รอดำเนินการ'}</Badge
												>
											</div>
											<ul class="mt-2 space-y-1 text-sm text-muted-foreground">
												{#each step.recipients as recipient (recipient.userId)}<li>
														{recipient.displayName}{recipient.completedAt ? ' · ดำเนินการแล้ว' : ''}
													</li>{/each}
											</ul>
											{#if step.definition.requiresSignature}<p
													class="mt-2 text-xs text-muted-foreground"
												>
													ขั้นตอนนี้ใช้ลายมือชื่อ
												</p>{/if}
										</li>{/each}
								</ol></Card.Content
							></Card.Root
						>{/if}
					{#if detail.policy}<Card.Root class="leave-criteria gap-3 py-4"
							><Card.Header class="px-4"
								><Card.Title
									>{detail.processPurpose === 'cancellation'
										? 'เกณฑ์ของคำขอเดิม'
										: 'เกณฑ์ที่ใช้กับคำขอ'}</Card.Title
								></Card.Header
							><Card.Content class="px-4"
								><dl class="grid gap-3 sm:grid-cols-2 lg:grid-cols-1">
									<div>
										<dt class="text-sm text-muted-foreground">แหล่งอ้างอิง</dt>
										<dd class="break-words">{detail.policy.sourceReference}</dd>
									</div>
									<div>
										<dt class="text-sm text-muted-foreground">ฉบับ / วันที่มีผล</dt>
										<dd>ฉบับ {detail.policy.version} · {leaveDate(detail.policy.effectiveFrom)}</dd>
									</div>
									<div>
										<dt class="text-sm text-muted-foreground">การนับวัน</dt>
										<dd>
											{detail.policy.countingMode === 'working_days'
												? 'วันทำงานที่โรงเรียนกำหนด'
												: 'วันปฏิทิน'}
										</dd>
									</div>
									<div>
										<dt class="text-sm text-muted-foreground">หลักฐาน</dt>
										<dd>
											{detail.policy.evidenceRequired
												? 'ต้องมีหลักฐานประกอบทุกคำขอ'
												: detail.policy.evidenceFromAmount != null
													? `ต้องแนบตั้งแต่ ${leaveAmount(detail.policy.evidenceFromAmount, detail.policy.unit)}`
													: 'แนบได้ตามความจำเป็น'}
										</dd>
									</div>
									{#if detail.policy.maxPerRequest != null}<div>
											<dt class="text-sm text-muted-foreground">จำนวนสูงสุดต่อคำขอ</dt>
											<dd>{leaveAmount(detail.policy.maxPerRequest, detail.policy.unit)}</dd>
										</div>{/if}{#if detail.policy.annualLimit != null}<div>
											<dt class="text-sm text-muted-foreground">สิทธิที่โรงเรียนกำหนดต่อรอบปี</dt>
											<dd>{leaveAmount(detail.policy.annualLimit, detail.policy.unit)}</dd>
										</div>{/if}
								</dl></Card.Content
							></Card.Root
						>{/if}
				</aside>
				<div class="leave-support space-y-4">
					{#if detail.evidenceFileIds.length}<Card.Root class="leave-evidence gap-3 py-4"
							><Card.Header class="px-4"><Card.Title>หลักฐานประกอบ</Card.Title></Card.Header
							><Card.Content class="px-4"
								><ul class="space-y-2">
									{#each detail.evidenceFileIds as fileId, index (fileId)}<li>
											<LoadingButton
												variant="outline"
												loading={downloadPending === fileId}
												disabled={!!downloadPending}
												onclick={() => download(fileId, index)}
												>ดาวน์โหลดหลักฐาน {index + 1}</LoadingButton
											>
										</li>{/each}
								</ul></Card.Content
							></Card.Root
						>{/if}
					<Card.Root class="leave-history gap-3 py-4"
						><Card.Header class="px-4"><Card.Title>ประวัติการดำเนินการ</Card.Title></Card.Header
						><Card.Content class="px-4"
							>{#if detail.history.length}<ol class="space-y-3">
									{#each detail.history as entry (entry.id)}<li
											class="space-y-2 border-b pb-3 last:border-0 last:pb-0"
										>
											<p class="font-medium">
												{leaveHistoryLabel(entry.action)}
											</p>
											<p class="text-sm text-muted-foreground">
												{entry.actorName} · {leaveDateTime(entry.createdAt)} · ฉบับ {entry.revision}
											</p>
											{#if entry.comment}<p class="break-words whitespace-pre-wrap text-sm">
													{entry.comment}
												</p>{/if}{#if entry.signature}<div class="space-y-2">
													<p class="text-sm">
														ลงลายมือชื่อและยืนยันคำขอฉบับ {entry.signature.input.documentRevision}
													</p>
													{#if entry.signature.imageFileId}<PrivateFileImage
															fileId={entry.signature.imageFileId}
															resourceId={detail.request.id}
															alt={`ลายมือชื่อ ${entry.actorName}`}
															class="h-24 w-56 object-contain"
														/>{:else}<svg
															viewBox="0 0 1000 1000"
															preserveAspectRatio="none"
															role="img"
															aria-label={`ลายมือชื่อ ${entry.actorName}`}
															class="h-24 w-56 rounded-md border bg-background text-foreground"
															>{#each entry.signature.input.strokes ?? [] as stroke, index (index)}<path
																	d={pathData(stroke.points)}
																	fill="none"
																	stroke="currentColor"
																	stroke-width="6"
																	stroke-linecap="round"
																	stroke-linejoin="round"
																/>{/each}</svg
														>{/if}
												</div>{/if}
										</li>{/each}
								</ol>{:else}<p class="text-sm text-muted-foreground">
									ยังไม่มีประวัติ
								</p>{/if}</Card.Content
						></Card.Root
					>
				</div>
			</div>
		{/if}
	</section>
</PageShell>

<Dialog.Root bind:open={editing}
	><Dialog.Content class="max-h-[90dvh] overflow-y-auto sm:max-w-2xl"
		><Dialog.Header
			><Dialog.Title>แก้ไขฉบับร่าง</Dialog.Title><Dialog.Description
				>ปรับข้อมูลและหลักฐานก่อนส่งพิจารณา</Dialog.Description
			></Dialog.Header
		>{#if editing && detail?.canEdit && creator}<RequestEditor
				kind={detail.request.kind}
				{detail}
				onrefresh={refreshed}
				oncancel={() => (editing = false)}
				onsaved={(result) => {
					region.patch(result);
					editing = false;
				}}
			/>{/if}</Dialog.Content
	></Dialog.Root
>
<Dialog.Root bind:open={cancelling}
	><Dialog.Content
		><Dialog.Header
			><Dialog.Title
				>{detail?.request.status === 'approved'
					? 'ขอยกเลิกคำขอที่อนุมัติแล้ว'
					: 'ถอนคำขอ'}</Dialog.Title
			><Dialog.Description
				>{detail?.request.status === 'approved'
					? 'คำขอยังคงมีผลและนับสิทธิ์จนกว่าจะอนุมัติการยกเลิก'
					: 'เก็บคำขอและประวัติเดิมไว้หลังถอนคำขอ'}</Dialog.Description
			></Dialog.Header
		>
		<form onsubmit={cancel} class="space-y-4">
			{#if error}<PageState
					variant="error"
					title="ยกเลิกคำขอไม่สำเร็จ"
					description={error}
					actionLabel="ตรวจสอบข้อมูลล่าสุด"
					onaction={refreshed}
				/>{/if}
			<div class="space-y-2">
				<Label for="leave-cancel-reason">เหตุผลการยกเลิก *</Label><Textarea
					id="leave-cancel-reason"
					bind:value={cancelReason}
					disabled={!!pending}
				/>
			</div>
			<div class="flex justify-end gap-2">
				<Button
					type="button"
					variant="outline"
					disabled={!!pending}
					onclick={() => (cancelling = false)}>กลับ</Button
				><LoadingButton type="submit" loading={pending === 'cancel'}>ยืนยันยกเลิกคำขอ</LoadingButton
				>
			</div>
		</form></Dialog.Content
	></Dialog.Root
>

<style>
	.leave-layout {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}
	@media (min-width: 1024px) {
		.leave-layout {
			display: grid;
			grid-template-columns: minmax(0, 1.6fr) minmax(18rem, 1fr);
			align-items: start;
		}
		.leave-primary {
			grid-column: 1;
			grid-row: 1;
		}
		.leave-side {
			grid-column: 2;
			grid-row: 1 / span 2;
		}
		.leave-support {
			grid-column: 1;
			grid-row: 2;
		}
	}

	@media print {
		:global(
			body:has(#staff-leave-print)
				*:not(:has(#staff-leave-print)):not(#staff-leave-print):not(#staff-leave-print *)
		) {
			display: none !important;
		}
		:global(body *:has(#staff-leave-print)) {
			height: auto;
			min-height: 0;
			overflow: visible;
			margin: 0;
		}
		#staff-leave-print {
			display: block;
			width: 100%;
		}
		#staff-leave-print :global([data-slot='card']) {
			margin-bottom: 10pt;
			break-inside: auto;
			gap: 8pt;
			padding-block: 10pt;
		}
		#staff-leave-print :global(li) {
			break-inside: avoid;
		}
		#staff-leave-print :global(*) {
			color: black !important;
			background-color: white !important;
			box-shadow: none !important;
		}
	}
</style>
