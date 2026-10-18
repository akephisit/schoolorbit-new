<script lang="ts">
	import { CareMutationIdentity } from '#lib/features/student-care/mutation.js';
	const mutation = new CareMutationIdentity();

	import { LatestRequest } from '#lib/async/latest-request.js';
	import { untrack, onDestroy } from 'svelte';
	import {
		addCareCaseEvent,
		careCaseEvents,
		type CareCaseEvent,
		type CareCaseDetail,
		type CareCaseStatus,
		type CareEventKind
	} from '#lib/api/student-care.js';
	import { mayManageCareCase, careIdentity } from '#lib/features/student-care/access.js';
	import {
		careDate,
		eventLabels,
		caseLabels,
		careLink
	} from '#lib/features/student-care/presentation.js';

	import { Label } from '#lib/components/ui/label/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import { DatePicker } from '#lib/components/ui/date-picker/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { LoadingButton, PageState } from '#lib/components/app-state/index.js';
	import CareSection from './CareSection.svelte';
	import CareSelect from './CareSelect.svelte';
	let { initial, term }: { initial: CareCaseDetail; term?: string | null } = $props();
	let detail = $state(untrack(() => initial)),
		kind = $state<CareEventKind>(
			untrack(() => (initial.case.status === 'closed' ? 'reopened' : 'note'))
		),
		status = $state<CareCaseStatus>(
			untrack(() => (initial.case.status === 'closed' ? 'open' : initial.case.status))
		),
		note = $state(''),
		nextFollowUp = $state(untrack(() => initial.case.nextFollowUp ?? '')),
		pending = $state(false),
		error = $state('');
	let events = $state<CareCaseEvent[]>(untrack(() => initial.events));
	let olderPending = $state(false),
		olderError = $state(''),
		hasOlder = $state(untrack(() => initial.events.length === 100));
	const canManage = $derived(mayManageCareCase(detail.case));
	const historyRequest = new LatestRequest();
	let alive = true;
	async function older() {
		if (olderPending || !hasOlder) return;
		olderPending = true;
		olderError = '';
		const identity = $careIdentity;
		const token = historyRequest.begin();
		try {
			const rows = await careCaseEvents(
				detail.case.id,
				{ afterId: events.at(-1)?.id, limit: 100 },
				{ signal: token.signal }
			);
			if (alive && identity === $careIdentity) {
				events = [...events, ...rows.filter((row) => !events.some((event) => event.id === row.id))];
				hasOlder = rows.length === 100;
			}
		} catch (cause) {
			if (alive && identity === $careIdentity)
				olderError = cause instanceof Error ? cause.message : 'โหลดประวัติไม่ได้';
		} finally {
			if (alive && identity === $careIdentity) olderPending = false;
		}
	}

	onDestroy(() => {
		alive = false;
		historyRequest.abort();
	});
	function changeKind(value: string) {
		const next = Object.keys(eventLabels).find((k) => k === value);
		if (
			next === 'note' ||
			next === 'contact' ||
			next === 'referral' ||
			next === 'follow_up' ||
			next === 'acknowledgement' ||
			next === 'closure' ||
			next === 'reopened'
		) {
			kind = next;
			if (next === 'closure') status = 'closed';
			else if (next === 'reopened' || status === 'closed') status = 'open';
		}
	}
	async function save(event: SubmitEvent) {
		event.preventDefault();
		if (pending || !canManage) return;
		if (!note.trim()) {
			error = 'กรอกรายละเอียดการดำเนินการ';
			return;
		}
		pending = true;
		error = '';
		const identity = $careIdentity;
		try {
			const next = await addCareCaseEvent(
				detail.case.id,
				mutation.prepare(
					{
						requestId: crypto.randomUUID(),
						rowVersion: detail.case.rowVersion,
						kind,
						note: note.trim(),
						status,
						nextFollowUp: nextFollowUp || null
					},
					'addCareCaseEvent'
				)
			);
			if (alive && identity === $careIdentity) {
				detail = next;
				kind = next.case.status === 'closed' ? 'reopened' : 'note';
				status = next.case.status === 'closed' ? 'open' : next.case.status;
				events = [
					...new Map([...events, ...next.events].map((event) => [event.id, event])).values()
				].sort((a, b) => b.id.localeCompare(a.id));
				note = '';
			}
		} catch (cause) {
			if (alive && identity === $careIdentity)
				error = cause instanceof Error ? cause.message : 'บันทึกไม่ได้';
		} finally {
			if (alive && identity === $careIdentity) pending = false;
		}
	}
</script>

<div class="max-w-4xl space-y-5">
	<CareSection title={detail.case.title} description={detail.case.studentName}
		><Badge variant="outline">{caseLabels[detail.case.status]}</Badge>
		<p class="text-sm text-muted-foreground">กำหนดติดตาม: {careDate(detail.case.nextFollowUp)}</p>
		<div class="flex flex-wrap gap-2">
			{#if canManage && detail.case.status !== 'closed'}<Button
					variant="outline"
					href={careLink(`/cases/${detail.case.id}/edit`, term)}>แก้ผู้รับผิดชอบและแผน</Button
				>{/if}{#if detail.case.sourceAssignmentId}<Button
					variant="ghost"
					href={`/student-care/assignments/${detail.case.sourceAssignmentId}`}
					>แบบประเมินต้นทาง</Button
				>{/if}{#if detail.case.sourceVisitId}<Button
					variant="ghost"
					href={careLink(`/visits/${detail.case.sourceVisitId}`, term)}>เยี่ยมบ้านต้นทาง</Button
				>{/if}
		</div></CareSection
	><CareSection title="ประวัติการดูแล"
		><ol class="space-y-4">
			{#each events as event (event.id)}<li class="border-l-2 border-primary/20 pl-4">
					<div class="flex flex-wrap items-center gap-2">
						<span class="text-sm font-semibold">{eventLabels[event.kind]}</span><time
							class="text-xs text-muted-foreground"
							datetime={event.createdAt}>{careDate(event.createdAt)}</time
						>
					</div>
					<p class="mt-2 whitespace-pre-wrap break-words text-sm">{event.note}</p>
				</li>{/each}
		</ol>
		{#if hasOlder}<LoadingButton variant="outline" loading={olderPending} onclick={older}
				>โหลดประวัติก่อนหน้า</LoadingButton
			>{/if}{#if olderError}<PageState
				variant="error"
				title="โหลดประวัติก่อนหน้าไม่ได้"
				description={olderError}
				actionLabel="ลองโหลดประวัติใหม่"
				onaction={older}
			/>{/if}
		{#if !events.length}<p class="text-sm text-muted-foreground">
				ยังไม่มีบันทึกเหตุการณ์
			</p>{/if}</CareSection
	>
	{#if canManage}<form onsubmit={save} class="space-y-5">
			<CareSection title="เพิ่มบันทึกการดูแล"
				><div class="grid gap-4 sm:grid-cols-2">
					<CareSelect
						id="case-event-kind"
						label="ประเภทการดำเนินการ"
						value={kind}
						options={Object.entries(eventLabels)
							.filter(([value]) =>
								detail.case.status === 'closed' ? value === 'reopened' : value !== 'reopened'
							)
							.map(([value, label]) => ({ value, label }))}
						onValueChange={changeKind}
					/><CareSelect
						id="case-event-status"
						label="สถานะหลังบันทึก"
						value={status}
						options={Object.entries(caseLabels)
							.filter(([value]) => (kind === 'closure' ? value === 'closed' : value !== 'closed'))
							.map(([value, label]) => ({ value, label }))}
						onValueChange={(value) => {
							if (value === 'open' || value === 'monitoring' || value === 'closed') status = value;
						}}
					/>
				</div>
				<div class="space-y-2">
					<Label for="case-event-note">รายละเอียดการดำเนินการ *</Label><Textarea
						id="case-event-note"
						bind:value={note}
						required
						maxlength={5000}
					/>
				</div>
				<div class="space-y-2">
					<Label for="case-event-followup">กำหนดติดตามครั้งต่อไป</Label><DatePicker
						id="case-event-followup"
						ariaLabel="กำหนดติดตามครั้งต่อไป"
						value={nextFollowUp}
						onValueChange={(value) => (nextFollowUp = value ?? '')}
						clearable
					/>
				</div></CareSection
			>{#if error}<PageState
					variant="error"
					title="บันทึกเหตุการณ์ไม่ได้"
					description={error}
				/>{/if}
			<div class="flex justify-end">
				<LoadingButton type="submit" loading={pending}>บันทึกการดำเนินการ</LoadingButton>
			</div>
		</form>{/if}
</div>
