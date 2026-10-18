<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import {
		getLeaveOptions,
		getEffectiveLeavePolicy,
		createLeaveRequest,
		updateLeaveRequest,
		type LeaveKind,
		type LeaveRequestDetail,
		type LeavePersonalOptions,
		type LeavePolicyVersion
	} from '#lib/api/staff-leave.js';
	import { uploadFile } from '#lib/api/files.js';
	import { ApiClientError } from '#lib/api/client.js';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { LeaveMutationIntent } from '#lib/staff-leave/mutation-intent.js';
	import {
		leaveDraft,
		leaveDraftErrors,
		leavePeriodInput,
		type LeaveDraft
	} from '#lib/staff-leave/drafts.js';
	import { leaveAmount, leaveDate, leaveUnitLabels } from '#lib/staff-leave/presentation.js';
	import { PERSONNEL_TYPE_LABELS } from '#lib/forms/staff-career.js';
	import { LoadingButton, PageState, PageSkeleton } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import * as ToggleGroup from '#lib/components/ui/toggle-group/index.js';
	import DatePicker from '#lib/components/ui/date-picker/DatePicker.svelte';
	import Choice from './Choice.svelte';
	let {
		kind,
		detail,
		oncancel,
		onsaved,
		onrefresh
	}: {
		kind: LeaveKind;
		detail?: LeaveRequestDetail;
		oncancel: () => void;
		onsaved: (detail: LeaveRequestDetail) => void;
		onrefresh?: () => void;
	} = $props();
	let draft = $state<LeaveDraft>(untrack(() => leaveDraft(detail)));
	let options = $state<LeavePersonalOptions | null>(null),
		optionsError = $state(''),
		optionsLoading = $state(true);
	let policy = $state<LeavePolicyVersion | null>(null),
		policyError = $state(''),
		policyLoading = $state(false);
	let errors = $state<Partial<Record<keyof LeaveDraft, string>>>({}),
		error = $state(''),
		pending = $state(false);
	let files = $state<File[]>([]);
	let expectedRevision = $state(untrack(() => detail?.request.revision ?? 0));
	let conflict = $state(false);
	let requestedReconciliation = false;
	const stale = $derived(!!detail && expectedRevision !== detail.request.revision);
	$effect.pre(() => {
		void detail;
		untrack(() => {
			if (requestedReconciliation) {
				requestedReconciliation = false;
				conflict = false;
			}
		});
	});
	const policyRequest = new LatestRequest(),
		intent = new LeaveMutationIntent(),
		controller = new AbortController();
	let disposed = false;
	const types = $derived(options?.types.filter((type) => type.kind === kind) ?? []);
	const typeChoices = $derived([
		...types.map((type) => ({ value: type.id, label: type.name })),
		...(detail && !types.some((type) => type.id === detail.request.typeId)
			? [{ value: detail.request.typeId, label: detail.request.typeName }]
			: [])
	]);
	async function resolvePolicy() {
		policy = null;
		policyError = '';
		policyRequest.abort();
		if (!draft.typeId || !draft.startDate) {
			policyLoading = false;
			return;
		}
		const ticket = policyRequest.begin();
		policyLoading = true;
		try {
			const result = await getEffectiveLeavePolicy(draft.typeId, draft.startDate, {
				signal: ticket.signal
			});
			if (policyRequest.isCurrent(ticket.revision) && !disposed) policy = result;
		} catch (cause) {
			if (policyRequest.isCurrent(ticket.revision) && !disposed)
				policyError = cause instanceof Error ? cause.message : 'ยังไม่มีนโยบายที่ใช้ได้';
		} finally {
			if (policyRequest.isCurrent(ticket.revision) && !disposed) policyLoading = false;
		}
	}
	async function loadOptions() {
		optionsLoading = true;
		optionsError = '';
		try {
			const result = await getLeaveOptions({ signal: controller.signal });
			if (!disposed) options = result;
		} catch (cause) {
			if (!disposed) optionsError = cause instanceof Error ? cause.message : 'โหลดประเภทไม่สำเร็จ';
		} finally {
			if (!disposed) optionsLoading = false;
		}
	}
	$effect.pre(() => {
		untrack(() => {
			void loadOptions();
			void resolvePolicy();
		});
	});
	onDestroy(() => {
		disposed = true;
		controller.abort();
		policyRequest.abort();
	});
	async function save(event: SubmitEvent) {
		event.preventDefault();
		if (pending) return;
		if (stale || conflict) return;
		errors = leaveDraftErrors(draft, policy);
		error = '';
		if (Object.keys(errors).length || !policy) {
			if (!policy) error = 'เลือกประเภทและวันที่ที่มีนโยบายใช้ได้ก่อนบันทึก';
			return;
		}
		pending = true;
		try {
			if (detail) {
				for (const file of files) {
					const uploaded = await uploadFile(file, 'staff_leave_evidence', detail.request.id);
					if (disposed) return;
					draft.evidenceFileIds = [...draft.evidenceFileIds, uploaded.id];
					files = files.filter((value) => value !== file);
				}
			}
			const period = leavePeriodInput(draft, policy.unit);
			const basis = detail ? { expectedRevision, period } : { period };
			const requestId = intent.for(basis);
			const result = detail
				? await updateLeaveRequest(detail.request.id, {
						requestId,
						expectedRevision,
						period
					})
				: await createLeaveRequest({ requestId, period });
			if (!disposed) onsaved(result);
		} catch (cause) {
			if (!disposed) {
				error = cause instanceof Error ? cause.message : 'บันทึกคำขอไม่สำเร็จ';
				conflict = cause instanceof ApiClientError && cause.status === 409;
			}
		} finally {
			if (!disposed) pending = false;
		}
	}
</script>

<form onsubmit={save} class="space-y-4" data-testid="leave-request-editor">
	{#if stale}<div class="space-y-2 rounded-md border p-3 text-sm">
			<p>คำขอมีการเปลี่ยนแปลง ตรวจสอบข้อมูลล่าสุดก่อนบันทึกฉบับร่างที่แก้ไข</p>
			<Button
				type="button"
				variant="outline"
				onclick={() => {
					if (detail) expectedRevision = detail.request.revision;
					conflict = false;
					error = '';
				}}>ใช้ข้อมูลล่าสุดและเก็บฉบับร่าง</Button
			>
		</div>{/if}
	{#if optionsError}<PageState
			variant="error"
			title="โหลดประเภทคำขอไม่สำเร็จ"
			description={optionsError}
			actionLabel="ลองอีกครั้ง"
			onaction={loadOptions}
		/>{/if}
	{#if optionsLoading && !options}<PageSkeleton variant="form" />{:else}
		{#if !types.length && !detail}<PageState
				title="ยังไม่มีประเภทคำขอที่เปิดใช้งาน"
				description="ติดต่อผู้ดูแลให้ตั้งค่าประเภทและนโยบายของโรงเรียน"
			/>{:else}
			{#if options?.personnelType}<p class="text-sm text-muted-foreground">
					{PERSONNEL_TYPE_LABELS[options.personnelType]} · ตรวจเกณฑ์ตามวันที่เริ่มคำขอ
				</p>{/if}
			<div>
				<Choice
					id="leave-type"
					label="ประเภทคำขอ *"
					bind:value={draft.typeId}
					options={typeChoices}
					disabled={pending}
					onchange={() => resolvePolicy()}
				/>{#if errors.typeId}<p class="mt-1 text-sm text-destructive">{errors.typeId}</p>{/if}
			</div>
			<div class="grid grid-cols-2 gap-3">
				<div class="space-y-2">
					<Label for="leave-start-date">วันที่เริ่ม *</Label><DatePicker
						id="leave-start-date"
						ariaLabel="วันที่เริ่ม"
						bind:value={draft.startDate}
						disabled={pending}
						onValueChange={(value) => {
							if (!draft.endDate) draft.endDate = value;
							void resolvePolicy();
						}}
					/>{#if errors.startDate}<p class="text-sm text-destructive">{errors.startDate}</p>{/if}
				</div>
				<div class="space-y-2">
					<Label for="leave-end-date">วันที่สิ้นสุด *</Label><DatePicker
						id="leave-end-date"
						ariaLabel="วันที่สิ้นสุด"
						bind:value={draft.endDate}
						disabled={pending}
					/>{#if errors.endDate}<p class="text-sm text-destructive">{errors.endDate}</p>{/if}
				</div>
			</div>
			<section aria-busy={policyLoading} data-testid="leave-effective-policy">
				{#if policyLoading}<p role="status" class="text-sm text-muted-foreground">
						กำลังตรวจสอบนโยบายตามวันที่เริ่ม...
					</p>{:else if policyError}<PageState
						variant="error"
						title="ไม่มีนโยบายที่ใช้ได้กับคำขอนี้"
						description={policyError}
						actionLabel="ตรวจสอบอีกครั้ง"
						onaction={resolvePolicy}
					/>{:else if policy}
					<div class="space-y-1 rounded-lg border bg-muted/20 p-3 text-xs">
						<p class="text-sm font-medium">
							นโยบายฉบับ {policy.version} · {leaveUnitLabels[policy.unit]}
						</p>
						<p>อ้างอิง: {policy.sourceReference} · มีผล {leaveDate(policy.effectiveFrom)}</p>
						<p>
							{policy.countingMode === 'working_days'
								? 'นับตามวันทำงานและวันยกเว้นที่โรงเรียนกำหนด'
								: 'นับตามวันปฏิทิน'}{policy.evidenceRequired ? ' · ต้องแนบหลักฐานก่อนส่ง' : ''}
						</p>
						{#if policy.maxPerRequest != null}<p>
								ไม่เกิน {leaveAmount(policy.maxPerRequest, policy.unit)} ต่อคำขอ
							</p>{/if}{#if policy.annualLimit != null}<p>
								สิทธิที่โรงเรียนกำหนด: {leaveAmount(policy.annualLimit, policy.unit)} ต่อรอบปี
							</p>{/if}{#if policy.evidenceFromAmount != null && !policy.evidenceRequired}<p>
								แนบหลักฐานเมื่อขอตั้งแต่ {leaveAmount(policy.evidenceFromAmount, policy.unit)}
							</p>{/if}{#if policy.minimumNoticeDays != null}<p>
								ยื่นล่วงหน้าอย่างน้อย {policy.minimumNoticeDays} วัน
							</p>{/if}
					</div>
				{/if}
			</section>
			{#if policy?.unit === 'half_day'}<div class="space-y-2">
					<Label>ช่วงวัน *</Label><ToggleGroup.Root
						class="flex-wrap"
						type="single"
						aria-label="ช่วงวัน"
						value={draft.dayPortion}
						onValueChange={(value) => {
							if (value === 'full_day' || value === 'morning' || value === 'afternoon')
								draft.dayPortion = value;
						}}
						disabled={pending}
						><ToggleGroup.Item value="full_day">เต็มวัน</ToggleGroup.Item><ToggleGroup.Item
							value="morning">ครึ่งวันเช้า</ToggleGroup.Item
						><ToggleGroup.Item value="afternoon">ครึ่งวันบ่าย</ToggleGroup.Item></ToggleGroup.Root
					>
				</div>{/if}
			{#if policy?.unit === 'minute'}<div class="grid grid-cols-2 gap-3">
					<div class="space-y-2">
						<Label for="leave-start-time">เวลาเริ่ม *</Label><Input
							id="leave-start-time"
							type="time"
							bind:value={draft.startTime}
							disabled={pending}
						/>{#if errors.startTime}<p class="text-sm text-destructive">{errors.startTime}</p>{/if}
					</div>
					<div class="space-y-2">
						<Label for="leave-end-time">เวลาสิ้นสุด *</Label><Input
							id="leave-end-time"
							type="time"
							bind:value={draft.endTime}
							disabled={pending}
						/>{#if errors.endTime}<p class="text-sm text-destructive">{errors.endTime}</p>{/if}
					</div>
				</div>{/if}
			<div class="space-y-2">
				<Label for="leave-reason">เหตุผล *</Label><Textarea
					id="leave-reason"
					rows={3}
					bind:value={draft.reason}
					disabled={pending}
					aria-invalid={!!errors.reason}
				/>{#if errors.reason}<p class="text-sm text-destructive">{errors.reason}</p>{/if}
			</div>
			{#if detail}<div class="space-y-2">
					<Label for="leave-evidence">เพิ่มหลักฐาน {policy?.evidenceRequired ? '*' : ''}</Label
					><Input
						id="leave-evidence"
						type="file"
						multiple
						disabled={pending}
						accept="application/pdf,image/png,image/jpeg,image/webp"
						onchange={(event) => (files = Array.from(event.currentTarget.files ?? []))}
					/>
					<p class="text-xs text-muted-foreground">
						PDF หรือภาพ ระบบจะตรวจสอบไฟล์ก่อนใช้ประกอบคำขอ
					</p>
					{#each draft.evidenceFileIds as id, index (id)}<div
							class="flex items-center justify-between gap-2 rounded-md border p-2 text-sm"
						>
							<span>หลักฐานเดิม {index + 1}</span><Button
								type="button"
								variant="ghost"
								size="sm"
								disabled={pending}
								onclick={() =>
									(draft.evidenceFileIds = draft.evidenceFileIds.filter((value) => value !== id))}
								>นำออกจากฉบับร่าง</Button
							>
						</div>{/each}{#each files as file, index (`${file.name}-${index}`)}<p
							class="text-sm text-muted-foreground"
						>
							{file.name}
						</p>{/each}
				</div>{/if}
		{/if}
	{/if}
	{#if error}<PageState
			variant="error"
			title="บันทึกคำขอไม่สำเร็จ"
			description={error}
			actionLabel={onrefresh ? 'ตรวจสอบข้อมูลล่าสุด' : undefined}
			onaction={() => {
				requestedReconciliation = true;
				onrefresh?.();
			}}
		/>{/if}
	<div class="sticky bottom-0 flex flex-wrap justify-end gap-2 border-t bg-background pt-3">
		<Button type="button" variant="outline" disabled={pending} onclick={oncancel}>ยกเลิก</Button
		><LoadingButton
			type="submit"
			loading={pending}
			disabled={!policy || policyLoading || !typeChoices.length || stale || conflict}
			>บันทึกฉบับร่าง</LoadingButton
		>
	</div>
</form>
