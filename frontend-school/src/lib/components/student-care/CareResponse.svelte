<script lang="ts">
	import { CareMutationIdentity } from '#lib/features/student-care/mutation.js';
	const mutation = new CareMutationIdentity();

	import { untrack, onDestroy } from 'svelte';
	import {
		saveCareResponse,
		acknowledgeCareAssignment,
		cancelCareAssignment,
		type CareAssignmentDetail,
		type CareRound,
		type CareAnswer,
		type CareManualScore
	} from '#lib/api/student-care.js';
	import {
		mayCancelCareAssignment,
		mayManageCareAssignment,
		careIdentity
	} from '#lib/features/student-care/access.js';
	import {
		careResponsePayload,
		responseError,
		setCareAnswer
	} from '#lib/features/student-care/drafts.js';
	import { careDate, respondentLabels } from '#lib/features/student-care/presentation.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';
	import { authStore } from '#lib/stores/auth.js';
	import * as RadioGroup from '#lib/components/ui/radio-group/index.js';
	import * as AlertDialog from '#lib/components/ui/alert-dialog/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import CareStatus from './CareStatus.svelte';
	import { LoadingButton, PageState } from '#lib/components/app-state/index.js';
	import CareSection from './CareSection.svelte';
	let { initial, round }: { initial: CareAssignmentDetail; round: CareRound } = $props();
	let detail = $state(untrack(() => initial));
	let answers = $state<CareAnswer[]>(untrack(() => structuredClone(initial.answers)));
	let scores = $state<CareManualScore[]>(untrack(() => structuredClone(initial.manualScores)));
	let note = $state(untrack(() => initial.manualNote ?? ''));
	let purpose = $state(untrack(() => !!initial.purposeAcknowledgedAt));
	let error = $state(''),
		message = $state(''),
		pending = $state(''),
		confirm = $state(false),
		reason = $state('');
	const definition = $derived(detail.instrument.definition);
	const submitted = $derived(detail.assignment.status === 'submitted');
	const canManual = $derived(
		$authStore.user?.user_type === 'staff' && mayManageCareAssignment(detail.assignment)
	);
	const editable = $derived(
		round.status === 'open' &&
			detail.assignment.status !== 'cancelled' &&
			!submitted &&
			(definition.mode === 'manual_result'
				? canManual
				: $can.has(PERMISSIONS.STUDENT_CARE_READ_OWN) &&
					detail.assignment.respondentId === $authStore.user?.id)
	);
	const mayReview = $derived(mayManageCareAssignment(detail.assignment));
	let alive = true;
	onDestroy(() => (alive = false));
	function scoreValue(key: string, value: string) {
		scores = scores.filter((s) => s.scaleKey !== key);
		if (value.trim() !== '') scores.push({ scaleKey: key, score: Number(value) });
	}
	function validateResponse(submit: boolean) {
		error = responseError(detail, answers, scores, submit) ?? '';
		if (submit && definition.mode === 'manual_result' && !note.trim())
			error = 'ระบุที่มาหรือเอกสารอ้างอิงคะแนนก่อนส่ง';
		if (submit && !purpose) error = 'กรุณาอ่านและรับทราบวัตถุประสงค์ก่อนส่งแบบประเมิน';
		return !error;
	}
	async function save(submit: boolean) {
		if (!editable || pending) return;
		if (!validateResponse(submit)) return;
		pending = submit ? 'submit' : 'draft';
		message = '';
		const identity = $careIdentity;
		try {
			const next = await saveCareResponse(
				detail.assignment.id,
				mutation.prepare(
					careResponsePayload(detail, answers, scores, note, purpose, crypto.randomUUID()),
					`response:${submit}`
				),
				submit
			);
			if (alive && identity === $careIdentity) {
				detail = next;
				message = submit ? 'ส่งแบบประเมินแล้ว' : 'บันทึกร่างบนเซิร์ฟเวอร์แล้ว';
				confirm = false;
			}
		} catch (cause) {
			if (alive && identity === $careIdentity)
				error = cause instanceof Error ? cause.message : 'บันทึกไม่ได้';
		} finally {
			if (alive && identity === $careIdentity) pending = '';
		}
	}
	async function command(kind: 'acknowledge' | 'cancel') {
		if (pending || !reason.trim()) return;
		pending = kind;
		error = '';
		const identity = $careIdentity;
		try {
			const operation = kind === 'acknowledge' ? acknowledgeCareAssignment : cancelCareAssignment;
			const assignment = await operation(
				detail.assignment.id,
				mutation.prepare(
					{
						requestId: crypto.randomUUID(),
						rowVersion: detail.assignment.rowVersion,
						reason: reason.trim()
					},
					kind
				)
			);
			if (alive && identity === $careIdentity) {
				detail = { ...detail, assignment };
				message = kind === 'acknowledge' ? 'รับทราบผลแล้ว' : 'ยกเลิกการมอบหมายแล้ว';
				reason = '';
			}
		} catch (cause) {
			if (alive && identity === $careIdentity)
				error = cause instanceof Error ? cause.message : 'ดำเนินการไม่ได้';
		} finally {
			if (alive && identity === $careIdentity) pending = '';
		}
	}
</script>

<div class="max-w-4xl space-y-5">
	<CareSection
		title={detail.assignment.instrumentName}
		description={`${detail.assignment.studentName} · ${respondentLabels[detail.assignment.respondent]} · รุ่น ${detail.instrument.version}`}
	>
		<div class="flex flex-wrap gap-2">
			<CareStatus
				status={{ kind: 'assignment', value: detail.assignment.status }}
			/>{#if submitted}<span class="text-sm text-muted-foreground"
					>ส่งเมื่อ {careDate(detail.assignment.submittedAt)}</span
				>{/if}
		</div>
		<p class="whitespace-pre-wrap text-sm">{detail.purpose}</p>
		<p class="text-sm text-muted-foreground">
			กำหนดส่ง {careDate(round.dueDate)}{round.status !== 'open' ? ' · รอบนี้ปิดรับคำตอบแล้ว' : ''}
		</p>
		{#if definition.instructions}<p class="whitespace-pre-wrap text-sm text-muted-foreground">
				{definition.instructions}
			</p>{/if}
		{#if editable}<div class="flex items-start gap-2">
				<Checkbox id="response-purpose" bind:checked={purpose} /><Label for="response-purpose"
					>อ่านและรับทราบวัตถุประสงค์การประเมินแล้ว</Label
				>
			</div>{/if}
		<p class="text-xs text-muted-foreground">
			ผลนี้ใช้ประกอบการดูแลช่วยเหลือ ไม่ใช่การวินิจฉัย ข้อมูลเข้าถึงได้ตามสิทธิ์ของผู้รับผิดชอบ
		</p>
	</CareSection>
	{#if definition.mode === 'manual_result' && !canManual && !submitted && detail.assignment.status !== 'cancelled' && round.status === 'open'}<PageState
			title="แบบประเมินกระดาษ"
			description="ทำแบบประเมินตามคำแนะนำของโรงเรียน ผู้รับผิดชอบจะบันทึกคะแนนจากเอกสารให้"
		/>{:else if editable}
		<form
			onsubmit={(event) => {
				event.preventDefault();
				if (validateResponse(true)) confirm = true;
			}}
			class="space-y-5"
		>
			{#if definition.mode === 'digital'}
				<CareSection
					title="คำตอบ"
					description={`ตอบแล้ว ${answers.length} จาก ${definition.questions.length} ข้อ สามารถบันทึกร่างและกลับมาตอบต่อได้`}
				>
					{#each definition.questions as question, i (question.key)}<fieldset
							class="space-y-3 border-b pb-4 last:border-0"
						>
							<legend class="mb-3 text-sm font-medium">{i + 1}. {question.text}</legend
							><RadioGroup.Root
								value={answers.find((a) => a.questionKey === question.key)?.optionKey ?? ''}
								onValueChange={(value) => (answers = setCareAnswer(answers, question.key, value))}
								aria-label={`ข้อ ${i + 1}`}
								class="flex flex-wrap gap-4"
							>
								{#each question.options as option (option.key)}<div class="flex items-center gap-2">
										<RadioGroup.Item
											id={`${question.key}-${option.key}`}
											value={option.key}
											disabled={!!pending}
										/><Label for={`${question.key}-${option.key}`}>{option.label}</Label>
									</div>{/each}
							</RadioGroup.Root><Button
								type="button"
								variant="ghost"
								size="sm"
								disabled={!!pending}
								onclick={() => (answers = setCareAnswer(answers, question.key, ''))}
								>ล้างคำตอบข้อ {i + 1}</Button
							>
						</fieldset>{/each}
				</CareSection>
			{:else}<CareSection
					title="คะแนนจากแบบกระดาษ"
					description="กรอกเฉพาะคะแนนมาตรวัดพื้นฐาน ระบบจะคำนวณมาตรรวมตามเกณฑ์รุ่นนี้"
				>
					{#each definition.scales.filter((s) => !s.sourceScaleKeys.length) as scale (scale.key)}<div
							class="space-y-2"
						>
							<Label for={`manual-${scale.key}`}>{scale.label}</Label><Input
								id={`manual-${scale.key}`}
								type="number"
								step={1}
								value={scores.find((s) => s.scaleKey === scale.key)?.score ?? ''}
								oninput={(event) => scoreValue(scale.key, event.currentTarget.value)}
								disabled={!!pending}
							/>
						</div>{/each}
					<div class="space-y-2">
						<Label for="manual-note">ที่มา / เอกสารอ้างอิงคะแนน *</Label><Textarea
							id="manual-note"
							bind:value={note}
							maxlength={5000}
							disabled={!!pending}
						/>
					</div>
				</CareSection>{/if}
			<div class="flex flex-wrap justify-end gap-2">
				<LoadingButton
					type="button"
					variant="outline"
					loading={pending === 'draft'}
					disabled={!!pending}
					onclick={() => save(false)}>บันทึกร่าง</LoadingButton
				><Button type="submit" disabled={!!pending}>ตรวจและส่งแบบประเมิน</Button>
			</div>
		</form>
	{:else if submitted}
		{#if detail.answers.length}<CareSection title="คำตอบที่ส่งแล้ว"
				><dl class="space-y-4">
					{#each definition.questions.filter( (question) => detail.answers.some((answer) => answer.questionKey === question.key) ) as question (question.key)}{@const answer =
							detail.answers.find((answer) => answer.questionKey === question.key)}
						<div class="space-y-1 border-b pb-3 last:border-0">
							<dt class="text-sm font-medium">{question.text}</dt>
							<dd class="text-sm text-muted-foreground">
								{question.options.find((option) => option.key === answer?.optionKey)?.label ??
									'ไม่มีตัวเลือกที่แสดงได้'}
							</dd>
						</div>{/each}
				</dl></CareSection
			>{/if}
		{#if detail.manualScores.length || detail.manualNote}<CareSection
				title="คะแนนและที่มาจากแบบกระดาษ"
				>{#if detail.manualScores.length}<dl class="space-y-2">
						{#each detail.manualScores as score (score.scaleKey)}<div
								class="flex flex-wrap justify-between gap-2 text-sm"
							>
								<dt>
									{definition.scales.find((scale) => scale.key === score.scaleKey)?.label ??
										score.scaleKey}
								</dt>
								<dd>{score.score}</dd>
							</div>{/each}
					</dl>{/if}{#if detail.manualNote}<p class="whitespace-pre-wrap break-words text-sm">
						{detail.manualNote}
					</p>{/if}</CareSection
			>{/if}
		<CareSection title="ผลการประเมิน"
			>{#if mayReview && detail.assignment.urgent}<p
					class="rounded-lg border border-warning/30 bg-warning/5 p-3 text-sm font-medium"
				>
					ควรติดตามเร่งด่วนตามเกณฑ์แบบประเมิน
				</p>{/if}
			{#if detail.results.length}<dl class="space-y-3">
					{#each detail.results as result (result.scaleKey)}<div
							class="flex flex-wrap items-center justify-between gap-2 rounded-lg border p-3"
						>
							<dt class="font-medium">
								{definition.scales.find((s) => s.key === result.scaleKey)?.label ?? result.scaleKey}
							</dt>
							<dd>{result.score} · {result.bandLabel}</dd>
						</div>{/each}
				</dl>{:else}<p class="text-sm text-muted-foreground">
					ส่งแบบประเมินแล้ว รายละเอียดผลแสดงเฉพาะผู้มีสิทธิ์
				</p>{/if}
			<p class="text-xs text-muted-foreground">
				การแปลผลอ้างอิงแบบประเมินรุ่น {detail.instrument.version} ไม่ใช่การวินิจฉัย
			</p>
		</CareSection>
	{:else if detail.assignment.status === 'cancelled'}<PageState
			title="ยกเลิกการมอบหมายแล้ว"
		/>{:else if round.status !== 'open'}<PageState
			title="รอบนี้ปิดรับคำตอบแล้ว"
			description="แบบร่างที่บันทึกไว้ยังคงอยู่บนเซิร์ฟเวอร์"
		/>{:else}<PageState
			variant="permission"
			title="อ่านรายละเอียดได้ แต่ไม่สามารถแก้คำตอบนี้"
		/>{/if}
	{#if $authStore.user?.user_type === 'staff' && ((mayReview && submitted && !detail.assignment.acknowledgedAt) || mayCancelCareAssignment(detail.assignment))}<CareSection
			title="การดำเนินการของผู้รับผิดชอบ"
		>
			<div class="space-y-2">
				<Label for="care-command-reason">บันทึก / เหตุผล *</Label><Textarea
					id="care-command-reason"
					bind:value={reason}
					maxlength={1000}
				/>
			</div>
			<div class="flex flex-wrap gap-2">
				{#if mayReview && submitted && !detail.assignment.acknowledgedAt}<LoadingButton
						loading={pending === 'acknowledge'}
						disabled={!!pending || !reason.trim()}
						onclick={() => command('acknowledge')}>รับทราบผลและติดตาม</LoadingButton
					>{/if}{#if mayCancelCareAssignment(detail.assignment)}<LoadingButton
						variant="outline"
						loading={pending === 'cancel'}
						disabled={!!pending || !reason.trim()}
						onclick={() => command('cancel')}>ยกเลิกการมอบหมาย</LoadingButton
					>{/if}
			</div>
		</CareSection>{/if}
	{#if $authStore.user?.user_type === 'staff' && mayReview && submitted}<Button
			variant="outline"
			href={`/staff/student-care/cases/new?sourceAssignmentId=${detail.assignment.id}`}
			>เปิดการดูแลจากแบบประเมิน</Button
		>{/if}
	{#if message}<p role="status" class="text-sm text-success">{message}</p>{/if}
	{#if error}<PageState variant="error" title="ดำเนินการไม่สำเร็จ" description={error} />{/if}
</div>
<AlertDialog.Root bind:open={confirm}
	><AlertDialog.Content
		><AlertDialog.Header
			><AlertDialog.Title>ยืนยันส่งแบบประเมิน</AlertDialog.Title><AlertDialog.Description
				>หลังส่งแล้วจะไม่สามารถแก้ไขคำตอบผ่านแบบร่างได้ กรุณาตรวจคำตอบและวัตถุประสงค์ให้เรียบร้อย</AlertDialog.Description
			></AlertDialog.Header
		>{#if error}<PageState
				variant="error"
				title="ส่งแบบประเมินไม่ได้"
				description={error}
			/>{/if}<AlertDialog.Footer
			><AlertDialog.Cancel disabled={!!pending}>กลับไปตรวจ</AlertDialog.Cancel><LoadingButton
				loading={pending === 'submit'}
				disabled={!!pending}
				onclick={() => save(true)}>ยืนยันส่งแบบประเมิน</LoadingButton
			></AlertDialog.Footer
		></AlertDialog.Content
	></AlertDialog.Root
>
