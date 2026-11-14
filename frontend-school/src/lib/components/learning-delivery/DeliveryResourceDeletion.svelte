<script lang="ts">
	import { onDestroy } from 'svelte';
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
		canManage,
		onDeleted
	}: {
		academicYearId: string;
		academicTermId: string;
		offeringId: string;
		groupId?: string | null;
		label: string;
		canManage: boolean;
		onDeleted: (result: DeletedDeliveryResource) => void | Promise<void>;
	} = $props();
	let open = $state(false),
		loading = $state(false),
		saving = $state(false),
		error = $state(''),
		preview = $state.raw<DeliveryDeletionPreview | null>(null);
	const request = new LatestRequest();
	let disposed = false,
		session = 0;
	const context = $derived(`${academicYearId}:${academicTermId}:${offeringId}:${groupId ?? ''}`);
	function current(owner: string, token: number) {
		return !disposed && open && canManage && context === owner && session === token;
	}
	async function load() {
		if (!canManage) return;
		const owner = context,
			token = session,
			ticket = request.begin();
		loading = true;
		error = '';
		preview = null;
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
			if (current(owner, token) && request.isCurrent(ticket.revision)) preview = result;
		} catch (failure) {
			if (current(owner, token) && request.isCurrent(ticket.revision) && !isAbortError(failure))
				error = failure instanceof Error ? failure.message : 'ตรวจผลกระทบไม่สำเร็จ';
		} finally {
			if (current(owner, token) && request.isCurrent(ticket.revision)) loading = false;
		}
	}
	function show() {
		if (!canManage || saving) return;
		session++;
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
		}
	}
	async function remove() {
		if (!preview || saving || loading || !canManage) return;
		const accepted = preview,
			owner = context,
			token = session;
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
			if (!current(owner, token)) return;
			await callback(result);
			if (current(owner, token)) {
				open = false;
				session++;
			}
		} catch (failure) {
			if (current(owner, token)) {
				error = failure instanceof Error ? failure.message : 'ลบไม่สำเร็จ';
				preview = null;
			}
		} finally {
			if (!disposed && context === owner && session === token) saving = false;
			else if (!disposed && !open) saving = false;
		}
	}
	$effect(() => {
		void context;
		if (!open) return;
		return () => {
			request.abort();
			session++;
			preview = null;
			loading = false;
		};
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
		{#if loading}<p role="status">กำลังตรวจผลกระทบ...</p>{:else if preview}
			<dl class="grid grid-cols-2 gap-3 text-sm">
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
				disabled={!preview || loading}
				onclick={remove}>ยืนยันลบถาวร</LoadingButton
			></AlertDialog.Footer
		>
	</AlertDialog.Content></AlertDialog.Root
>
