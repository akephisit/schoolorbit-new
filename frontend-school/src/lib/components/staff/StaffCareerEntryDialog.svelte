<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Select from '$lib/components/ui/select';
	import { Button } from '$lib/components/ui/button';
	import { Label } from '$lib/components/ui/label';
	import { Textarea } from '$lib/components/ui/textarea';
	import { PageState } from '$lib/components/app-state';
	import { LoaderCircle, Save } from '@lucide/svelte';
	import { ApiClientError } from '$lib/api/client';
	import { LatestRequest } from '$lib/async/latest-request';
	import {
		appendStaffCareerHistory,
		correctStaffCareerHistory,
		listStaffCareerHistory,
		type StaffCareerMutationAck
	} from '$lib/api/staff-career';
	import {
		staffCareerDraft,
		careerEntryInput,
		careerFieldErrors,
		CAREER_KIND_LABELS,
		PERSONNEL_TYPE_LABELS,
		type StaffCareerEntry,
		type StaffCareerDraft,
		type StaffCareerKind,
		type StaffPersonnelType
	} from '$lib/forms/staff-career';
	import { ACADEMIC_RANK_LABELS, staffCareerEntryLabel } from '$lib/forms/staff-personnel';
	import StaffCareerDates from './StaffCareerDates.svelte';
	import StaffJobPositionPicker from './StaffJobPositionPicker.svelte';
	let {
		staffId,
		entry,
		open = $bindable(false),
		onSaved
	}: {
		staffId: string;
		entry: StaffCareerEntry | null;
		open?: boolean;
		onSaved: (ack: StaffCareerMutationAck) => void;
	} = $props();
	const keyFor = {
		personnel_type: 'personnelType',
		job_position: 'jobPosition',
		academic_rank: 'academicRank'
	} as const;
	let draft = $state<StaffCareerDraft>(staffCareerDraft(null)),
		kind = $state<StaffCareerKind>('academic_rank');
	let expected = $state<StaffCareerEntry | null>(null),
		selectedPosition = $state<StaffCareerEntry['jobPosition']>(null);
	let reason = $state(''),
		error = $state(''),
		saving = $state(false),
		conflict = $state(false);
	let appendId = '',
		activeOwner = '',
		activeEntryId = '',
		wasOpen = false,
		epoch = 0,
		disposed = false;
	const reconciliation = new LatestRequest();
	const key = $derived(keyFor[kind]),
		fieldErrors = $derived(careerFieldErrors(draft));
	function entryDraft(source: StaffCareerEntry): StaffCareerDraft {
		return staffCareerDraft({
			personnelType: source.fact.kind === 'personnel_type' ? source : null,
			jobPosition: source.fact.kind === 'job_position' ? source : null,
			academicRank: source.fact.kind === 'academic_rank' ? source : null
		});
	}
	$effect.pre(() => {
		const visible = open,
			owner = staffId,
			incoming = entry;
		untrack(() => {
			if (!visible) {
				wasOpen = false;
				epoch++;
				reconciliation.abort();
				return;
			}
			if (wasOpen && activeOwner === owner && activeEntryId === (incoming?.id ?? '')) return;
			epoch++;
			wasOpen = true;
			activeOwner = owner;
			activeEntryId = incoming?.id ?? '';
			expected = incoming ? structuredClone($state.snapshot(incoming)) : null;
			kind = incoming?.fact.kind ?? 'academic_rank';
			draft = incoming ? entryDraft(incoming) : staffCareerDraft(null);
			selectedPosition = incoming?.jobPosition ?? null;
			appendId = incoming?.id ?? crypto.randomUUID();
			reason = '';
			error = '';
			saving = false;
			conflict = false;
		});
	});
	onDestroy(() => {
		disposed = true;
		epoch++;
		reconciliation.abort();
	});
	async function save() {
		if (saving || conflict) return;
		const owner = staffId,
			revision = ++epoch;
		const current = () => !disposed && open && owner === staffId && revision === epoch;
		error = '';
		try {
			const value = draft[key];
			const fact =
				kind === 'personnel_type'
					? { kind, value: draft.personnelType.value }
					: kind === 'job_position'
						? { kind, value: draft.jobPosition.value }
						: { kind, value: draft.academicRank.value };
			const input = careerEntryInput(fact, value);
			if (expected && !reason.trim()) {
				error = 'กรุณาระบุเหตุผลการแก้ไข';
				return;
			}
			if (Array.from(reason.trim()).length > 1000 || /\p{Cc}/u.test(reason)) {
				error = 'เหตุผลต้องไม่เกิน 1000 ตัวอักษรและไม่มีอักขระควบคุม';
				return;
			}
			if (
				!expected &&
				input.fact.value === null &&
				!input.effectiveDate &&
				!input.orderDate &&
				!input.orderNumber &&
				!input.note
			) {
				error = 'กรุณาระบุข้อมูลประวัติอย่างน้อยหนึ่งรายการ';
				return;
			}
			saving = true;
			const ack = expected
				? await correctStaffCareerHistory(owner, expected.id, {
						expectedRevision: expected.revision,
						expectedIsCurrent: expected.isCurrent,
						entry: input,
						reason: reason.trim()
					})
				: await appendStaffCareerHistory(owner, { id: appendId, entry: input });
			if (current()) onSaved(ack);
		} catch (cause) {
			if (current()) {
				error = cause instanceof Error ? cause.message : 'บันทึกประวัติไม่สำเร็จ';
				conflict = cause instanceof ApiClientError && cause.status === 409;
			}
		} finally {
			if (current()) saving = false;
		}
	}
	async function reconcile() {
		if (saving) return;
		const owner = staffId,
			ticket = reconciliation.begin(),
			targetId = expected?.id ?? appendId;
		saving = true;
		try {
			let cursor: string | undefined;
			const seen = new SvelteSet<string>();
			let fresh: StaffCareerEntry | undefined;
			do {
				const page = await listStaffCareerHistory(
					owner,
					{ pageSize: 50, cursor },
					{ signal: ticket.signal }
				);
				if (disposed || owner !== staffId || !open || !reconciliation.isCurrent(ticket.revision))
					return;
				fresh = [
					...page.items,
					...[
						page.current.personnelType,
						page.current.jobPosition,
						page.current.academicRank
					].filter((item) => item !== null)
				].find((item) => item.id === targetId);
				cursor = page.nextCursor ?? undefined;
				if (cursor && seen.has(cursor)) throw new Error('โหลดประวัติไม่สำเร็จ กรุณาลองใหม่');
				if (cursor) seen.add(cursor);
			} while (!fresh && cursor);
			if (!fresh) throw new Error('ไม่พบรายการล่าสุด กรุณาปิดหน้าต่างแล้วตรวจประวัติอีกครั้ง');
			const previous = expected ? entryDraft(expected)[key] : null,
				updated = entryDraft(fresh)[key];
			if (previous)
				for (const field of ['effectiveDate', 'orderDate', 'orderNumber', 'note'] as const)
					if (draft[key][field] === previous[field]) draft[key][field] = updated[field];
			if (previous && draft[key].value === previous.value) {
				if (kind === 'personnel_type')
					draft.personnelType.value =
						fresh.fact.kind === 'personnel_type' ? fresh.fact.value : null;
				else if (kind === 'academic_rank')
					draft.academicRank.value = fresh.fact.kind === 'academic_rank' ? fresh.fact.value : null;
				else {
					draft.jobPosition.value = fresh.fact.kind === 'job_position' ? fresh.fact.value : null;
					selectedPosition = fresh.jobPosition;
				}
			}
			expected = fresh;
			conflict = false;
			error = '';
		} catch (cause) {
			if (!disposed && owner === staffId && reconciliation.isCurrent(ticket.revision))
				error = cause instanceof Error ? cause.message : 'โหลดข้อมูลล่าสุดไม่สำเร็จ';
		} finally {
			if (!disposed && owner === staffId && reconciliation.isCurrent(ticket.revision))
				saving = false;
		}
	}
</script>

<Dialog.Root bind:open>
	<Dialog.Content
		class="max-h-[85dvh] overflow-y-auto sm:max-w-2xl"
		showCloseButton={!saving}
		onEscapeKeydown={(event) => {
			if (saving) event.preventDefault();
		}}
		onInteractOutside={(event) => {
			if (saving) event.preventDefault();
		}}
	>
		<Dialog.Header
			><Dialog.Title>{expected ? 'แก้ไขประวัติบุคลากร' : 'เพิ่มประวัติย้อนหลัง'}</Dialog.Title
			><Dialog.Description
				>{expected
					? `แก้รายการ${expected.isCurrent ? 'ปัจจุบัน' : 'ย้อนหลัง'} · ${staffCareerEntryLabel(expected)}`
					: 'บันทึกข้อมูลตามเอกสารย้อนหลัง รายการนี้จะไม่เปลี่ยนข้อมูลปัจจุบัน'}</Dialog.Description
			></Dialog.Header
		>
		<form
			class="space-y-5"
			onsubmit={(event) => {
				event.preventDefault();
				void save();
			}}
		>
			<div class="space-y-2">
				<Label for="career-history-kind">ประเภทข้อมูล</Label><Select.Root
					type="single"
					value={kind}
					onValueChange={(code) => {
						if (code in CAREER_KIND_LABELS) kind = code as StaffCareerKind;
					}}
					disabled={saving || expected !== null}
					><Select.Trigger id="career-history-kind" class="w-full" aria-label="ประเภทข้อมูล"
						>{CAREER_KIND_LABELS[kind]}</Select.Trigger
					><Select.Content
						>{#each Object.entries(CAREER_KIND_LABELS) as [code, label] (code)}<Select.Item
								value={code}>{label}</Select.Item
							>{/each}</Select.Content
					></Select.Root
				>
			</div>
			{#if kind === 'job_position'}<StaffJobPositionPicker
					label="ตำแหน่งงาน"
					bind:value={draft.jobPosition.value}
					bind:selected={selectedPosition}
					disabled={saving}
				/>
			{:else}<div class="space-y-2">
					<Label for="career-history-value">{CAREER_KIND_LABELS[kind]}</Label><Select.Root
						type="single"
						value={draft[key].value ?? 'unspecified'}
						onValueChange={(code) => {
							if (kind === 'personnel_type')
								draft.personnelType.value =
									code === 'unspecified' ? null : (code as StaffPersonnelType);
							else
								draft.academicRank.value =
									code === 'unspecified' ? null : (code as keyof typeof ACADEMIC_RANK_LABELS);
						}}
						disabled={saving}
						><Select.Trigger
							id="career-history-value"
							class="w-full"
							aria-label={CAREER_KIND_LABELS[kind]}
							>{draft[key].value
								? kind === 'personnel_type'
									? PERSONNEL_TYPE_LABELS[draft.personnelType.value!]
									: ACADEMIC_RANK_LABELS[draft.academicRank.value!]
								: 'ยังไม่ระบุ'}</Select.Trigger
						><Select.Content
							><Select.Item value="unspecified">ยังไม่ระบุ</Select.Item
							>{#each Object.entries(kind === 'personnel_type' ? PERSONNEL_TYPE_LABELS : ACADEMIC_RANK_LABELS) as [code, label] (code)}<Select.Item
									value={code}>{label}</Select.Item
								>{/each}</Select.Content
						></Select.Root
					>
				</div>{/if}
			<StaffCareerDates
				bind:value={draft[key]}
				prefix={kind}
				disabled={saving}
				errors={fieldErrors}
			/>
			{#if expected}<div class="space-y-2">
					<Label for="career-history-reason"
						>เหตุผลการแก้ไข <span class="text-destructive">*</span></Label
					><Textarea
						id="career-history-reason"
						bind:value={reason}
						disabled={saving}
						aria-label="เหตุผลการแก้ไข"
						rows={2}
					/>
				</div>{/if}
			{#if error}<PageState variant="error" title={error} />{/if}
			{#if conflict}<div class="space-y-2 rounded-xl border bg-muted/40 p-3">
					<p class="text-sm">
						ร่างและเหตุผลของคุณยังอยู่ โหลดรายการล่าสุดแล้วตรวจสอบก่อนบันทึกอีกครั้ง
					</p>
					<Button type="button" variant="outline" disabled={saving} onclick={reconcile}
						>โหลดข้อมูลล่าสุดและตรวจสอบร่าง</Button
					>
				</div>{/if}
			<Dialog.Footer
				><Button type="button" variant="outline" disabled={saving} onclick={() => (open = false)}
					>ยกเลิก</Button
				><Button
					type="submit"
					disabled={saving ||
						conflict ||
						Object.keys(fieldErrors).some((field) => field.startsWith(`${kind}.`))}
					>{#if saving}<LoaderCircle class="size-4 animate-spin" />{:else}<Save
							class="size-4"
						/>{/if}{expected ? 'บันทึกการแก้ไข' : 'บันทึกประวัติ'}</Button
				></Dialog.Footer
			>
		</form>
	</Dialog.Content>
</Dialog.Root>
