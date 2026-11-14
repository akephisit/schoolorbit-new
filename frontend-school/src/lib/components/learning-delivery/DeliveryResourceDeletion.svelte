<script lang="ts">
	import { onDestroy } from 'svelte';
	import { authStore } from '#lib/stores/auth.js';
	import { getAcademicContextStore } from '#lib/academic-context/store.js';
	import {
		previewOfferingDeletion,
		previewGroupDeletion,
		deleteLearningOffering,
		deleteLearningGroup,
		type DeliveryDeletionPreview,
		type DeletedDeliveryResource
	} from '#lib/api/learning-delivery.js';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { LoadingButton } from '#lib/components/app-state/index.js';
	import * as AlertDialog from '#lib/components/ui/alert-dialog/index.js';
	import { Trash2 } from '@lucide/svelte';
	let {
		academicYearId,
		academicTermId,
		offeringId,
		groupId = null,
		label,
		offeringName,
		canManage,
		onDeleted
	}: {
		academicYearId: string;
		academicTermId: string;
		offeringId: string;
		groupId?: string | null;
		label: string;
		offeringName: string;
		canManage: boolean;
		onDeleted: (result: DeletedDeliveryResource) => void | Promise<void>;
	} = $props();
	const academicContext = getAcademicContextStore();
	const year = $derived(
		$academicContext.options?.years.find((candidate) => candidate.id === academicYearId)
	);
	const term = $derived(
		$academicContext.options?.terms.find(
			(candidate) => candidate.id === academicTermId && candidate.academicYearId === academicYearId
		)
	);
	const contextMetadataReady = $derived(
		Boolean(year?.name.trim() && term?.name.trim() && offeringName.trim() && label.trim())
	);
	const yearLabel = $derived.by(() => {
		if (!year?.name.trim()) return 'ชื่อปีการศึกษายังไม่พร้อม';
		const name = year.name.trim(),
			standard = `ปีการศึกษา ${year.year}`;
		return name === standard ? name : `${name} (${standard})`;
	});
	const termLabel = $derived(
		term?.name.trim() ? `${term.name} · ลำดับ ${term.sequence}` : 'ชื่อภาคเรียนยังไม่พร้อม'
	);

	let open = $state(false),
		loading = $state(false),
		saving = $state(false),
		error = $state(''),
		preview = $state.raw<DeliveryDeletionPreview | null>(null);
	let previewEpoch = $state<number | null>(null),
		dialogEpoch = $state<number | null>(null),
		dialogContext = $state('');
	const request = new LatestRequest();
	let disposed = false,
		session = 0;
	const context = $derived(`${academicYearId}:${academicTermId}:${offeringId}:${groupId ?? ''}`);
	function current(owner: string, token: number, epoch: number) {
		return (
			!disposed &&
			open &&
			canManage &&
			context === owner &&
			session === token &&
			authStore.sessionEpoch === epoch &&
			dialogEpoch === epoch
		);
	}
	async function load() {
		if (!canManage) return;
		const owner = context,
			token = session,
			epoch = authStore.sessionEpoch,
			ticket = request.begin();
		loading = true;
		error = '';
		preview = null;
		previewEpoch = null;
		try {
			const result = await (groupId
				? previewGroupDeletion(
						groupId,
						{ academicYearId, academicTermId },
						{ signal: ticket.signal }
					)
				: previewOfferingDeletion(
						offeringId,
						{ academicYearId, academicTermId },
						{ signal: ticket.signal }
					));
			if (current(owner, token, epoch) && request.isCurrent(ticket.revision)) {
				if (
					result.academicYearId !== academicYearId ||
					result.academicTermId !== academicTermId ||
					result.learningOfferingId !== offeringId ||
					(result.learningGroupId ?? null) !== (groupId ?? null)
				) {
					throw new Error('ผลตรวจไม่ตรงกับปี ภาคเรียน หรือรายการที่ขอ กรุณาตรวจผลกระทบใหม่');
				}
				preview = result;
				previewEpoch = epoch;
			}
		} catch (failure) {
			if (
				current(owner, token, epoch) &&
				request.isCurrent(ticket.revision) &&
				!isAbortError(failure)
			)
				error = failure instanceof Error ? failure.message : 'ตรวจผลกระทบไม่สำเร็จ';
		} finally {
			if (current(owner, token, epoch) && request.isCurrent(ticket.revision)) loading = false;
		}
	}
	function show() {
		if (!canManage || saving) return;
		session++;
		dialogEpoch = authStore.sessionEpoch;
		dialogContext = context;
		open = true;
		void load();
	}
	function changeOpen(value: boolean) {
		if (saving) return;
		open = value;
		if (!value) {
			session++;
			request.abort();
			loading = false;
			preview = null;
			previewEpoch = null;
			dialogEpoch = null;
		}
	}
	async function remove() {
		if (
			!preview ||
			saving ||
			loading ||
			!canManage ||
			!contextMetadataReady ||
			previewEpoch !== authStore.sessionEpoch ||
			dialogEpoch !== authStore.sessionEpoch
		)
			return;
		const accepted = preview,
			owner = context,
			token = session,
			epoch = authStore.sessionEpoch;
		const callback = onDeleted;
		saving = true;
		error = '';
		try {
			const body = {
				academicYearId: accepted.academicYearId,
				academicTermId: accepted.academicTermId,
				rowVersion: accepted.rowVersion,
				sourceDigest: accepted.sourceDigest
			};
			const result = await (groupId
				? deleteLearningGroup(groupId, body)
				: deleteLearningOffering(offeringId, body));
			if (!current(owner, token, epoch)) return;
			await callback(result);
			if (current(owner, token, epoch)) invalidateDialog();
		} catch (failure) {
			if (current(owner, token, epoch)) {
				error = failure instanceof Error ? failure.message : 'ลบไม่สำเร็จ';
				preview = null;
				previewEpoch = null;
			}
		} finally {
			if (current(owner, token, epoch)) saving = false;
		}
	}
	function invalidateDialog() {
		request.abort();
		session++;
		open = false;
		preview = null;
		previewEpoch = null;
		dialogEpoch = null;
		loading = false;
		saving = false;
		error = '';
	}
	$effect(() => {
		void $authStore;
		const epoch = authStore.sessionEpoch;
		if (dialogEpoch !== null && (dialogEpoch !== epoch || dialogContext !== context || !canManage))
			invalidateDialog();
	});

	onDestroy(() => {
		disposed = true;
		request.abort();
	});
</script>

{#if canManage}<Button variant="destructive" size="sm" onclick={show}
		><Trash2 class="size-4" />{groupId ? 'ลบกลุ่ม' : 'ลบรายการเปิดสอน'}</Button
	>{/if}
<AlertDialog.Root {open} onOpenChange={changeOpen}
	><AlertDialog.Content class="max-h-[calc(100dvh-2rem)] overflow-y-auto"
		><AlertDialog.Header
			><AlertDialog.Title>ลบ {label}</AlertDialog.Title><AlertDialog.Description
				>ตรวจผลกระทบก่อนลบถาวร ข้อมูลคะแนนและเช็คชื่อรายคาบที่เป็นของรายการนี้จะถูกลบด้วย</AlertDialog.Description
			></AlertDialog.Header
		>
		<section aria-label="บริบทการลบ" class="rounded-xl border bg-muted/30 p-3 text-sm">
			<dl class="grid grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-2">
				<dt>ปีการศึกษา</dt>
				<dd class="break-words">{yearLabel}</dd>
				<dt>ภาคเรียน</dt>
				<dd class="break-words">{termLabel}</dd>
				<dt>รายการเปิดสอน</dt>
				<dd class="break-words">{offeringName.trim() || 'ชื่อรายการเปิดสอนยังไม่พร้อม'}</dd>
				{#if groupId}<dt>กลุ่มเรียน</dt>
					<dd class="break-words">{label.trim() || 'ชื่อกลุ่มเรียนยังไม่พร้อม'}</dd>{/if}
			</dl>
			{#if !contextMetadataReady}<p role="status" class="mt-3 text-muted-foreground">
					ข้อมูลชื่อปีการศึกษา ภาคเรียน หรือรายการยังไม่ครบ จึงยังยืนยันลบไม่ได้
				</p>{/if}
		</section>
		{#if loading}<p role="status">กำลังตรวจผลกระทบ...</p>{:else if preview}
			<dl aria-label="ผลกระทบการลบ" class="grid grid-cols-2 gap-3 text-sm">
				<dt>กลุ่มเรียน</dt>
				<dd>{preview.impact.learningGroups}</dd>
				<dt>การมอบหมายครู</dt>
				<dd>{preview.impact.teacherAssignments}</dd>
				<dt>สมาชิกกลุ่ม</dt>
				<dd>{preview.impact.memberships}</dd>
				<dt>คาบเรียน</dt>
				<dd>{preview.impact.timetableBlocks}</dd>
				<dt>รายการสอบ</dt>
				<dd>{preview.impact.examScheduleItems}</dd>
				<dt>คะแนน</dt>
				<dd>{preview.impact.assessmentScores}</dd>
				<dt>คาบเช็คชื่อ</dt>
				<dd>{preview.impact.attendanceSessions}</dd>
				<dt>สรุปเช็คชื่อ</dt>
				<dd>{preview.impact.attendanceSummaries}</dd>
				<dt>ผลการเรียน</dt>
				<dd>{preview.impact.officialResults}</dd>
				<dt>คำร้องแก้ผล</dt>
				<dd>{preview.impact.resultCorrections}</dd>
				<dt>ผลรวมภาคเรียน</dt>
				<dd>{preview.impact.termAggregates}</dd>
				<dt>ผลรวมรายปี</dt>
				<dd>{preview.impact.annualAggregates}</dd>
				<dt>ข้อมูลนิเทศที่เลิกเชื่อมโยง</dt>
				<dd>{preview.impact.supervisionLinks}</dd>
				<dt>พฤติกรรมจากเช็คชื่อ</dt>
				<dd>{preview.impact.derivedBehaviorEvents}</dd>
				<dt>ไฟล์หลักฐาน</dt>
				<dd>{preview.impact.evidenceFiles}</dd>
			</dl>{/if}
		{#if error}<p role="alert" class="text-sm text-destructive">{error}</p>{/if}
		{#if !loading && !preview}<Button variant="outline" disabled={saving} onclick={load}
				>ตรวจผลกระทบอีกครั้ง</Button
			>{/if}
		<AlertDialog.Footer
			><AlertDialog.Cancel disabled={saving}>ยกเลิก</AlertDialog.Cancel><LoadingButton
				variant="destructive"
				loading={saving}
				loadingLabel="กำลังลบ"
				disabled={!preview || loading || !contextMetadataReady}
				onclick={remove}>ยืนยันลบถาวร</LoadingButton
			></AlertDialog.Footer
		>
	</AlertDialog.Content></AlertDialog.Root
>
