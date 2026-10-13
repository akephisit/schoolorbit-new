<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { goto } from '$app/navigation';
	import {
		behaviorItems,
		behaviorStudents,
		createBehaviorBatch,
		type BehaviorItem,
		type BehaviorStudent,
		type BehaviorStudentPage,
		type CreateBehaviorBatch
	} from '#lib/api/behavior.js';
	import { ApiClientError } from '#lib/api/client.js';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import {
		bangkokDateTime,
		resolveBehaviorOccurredAt,
		sideLabels,
		sides
	} from '#lib/features/behavior/presentation.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import { DatePicker } from '#lib/components/ui/date-picker/index.js';
	import { LoadingButton, PageState } from '#lib/components/app-state/index.js';
	import BehaviorSelect from './BehaviorSelect.svelte';
	import BehaviorStudentPicker from './BehaviorStudentPicker.svelte';
	let {
		term,
		initialItems,
		initialRoster,
		date: initialDate
	}: {
		term: string;
		initialItems: BehaviorItem[];
		initialRoster: BehaviorStudentPage;
		date: string;
	} = $props();
	let items = $state(untrack(() => initialItems));
	let roster = $state(untrack(() => initialRoster));
	let date = $state(untrack(() => initialDate));
	let loadedDate = $state(untrack(() => initialDate));
	let defaultInstant = $state(untrack(() => new Date().toISOString()));
	let defaultClock = $state(untrack(() => bangkokDateTime(new Date(defaultInstant))));
	let time = $state(untrack(() => defaultClock.time));
	let side = $state(sides[0]);
	let itemId = $state('');
	let detail = $state('');
	let selected = $state<BehaviorStudent[]>([]);
	let rosterToken = $state(untrack(() => initialRoster.rosterToken));
	let pending = $state(false);
	let refreshing = $state(false);
	let error = $state('');
	let receipt = $state('');
	let attempted = $state(false);
	let frozen = $state.raw<CreateBehaviorBatch | null>(null);
	const request = new LatestRequest();
	let disposed = false;
	onDestroy(() => {
		disposed = true;
		request.abort();
	});
	const item = $derived(items.find((i) => i.id === itemId));
	const fieldErrors = $derived({
		item: attempted && (!item || !item.versionId) ? 'เลือกรายการที่มีเกณฑ์ในวันที่เกิดเหตุ' : '',
		students: attempted && !selected.length ? 'เลือกนักเรียนอย่างน้อยหนึ่งคน' : '',
		date: attempted && date !== loadedDate ? 'โหลดเกณฑ์และรายชื่อสำหรับวันที่ใหม่ก่อนบันทึก' : ''
	});
	async function reload() {
		const token = request.begin();
		const requestedDate = date;
		const requestedTerm = term;
		refreshing = true;
		error = '';
		try {
			const [nextItems, nextRoster] = await Promise.all([
				behaviorItems({ date: requestedDate }, { signal: token.signal }),
				behaviorStudents(
					{ academicTermId: requestedTerm, date: requestedDate, limit: 50 },
					{ signal: token.signal }
				)
			]);
			if (!request.isCurrent(token.revision) || date !== requestedDate || term !== requestedTerm)
				return;
			items = nextItems;
			roster = nextRoster;
			rosterToken = nextRoster.rosterToken;
			loadedDate = requestedDate;
			frozen = null;
		} catch (e) {
			if (request.isCurrent(token.revision))
				error = e instanceof Error ? e.message : 'โหลดข้อมูลไม่ได้';
		} finally {
			if (request.isCurrent(token.revision)) refreshing = false;
		}
	}
	async function submit(event: SubmitEvent) {
		event.preventDefault();
		attempted = true;
		if (
			pending ||
			refreshing ||
			fieldErrors.item ||
			fieldErrors.students ||
			fieldErrors.date ||
			!item?.versionId
		)
			return;
		error = '';
		try {
			if (!frozen)
				frozen = {
					requestId: crypto.randomUUID(),
					academicTermId: term,
					itemId: item.id,
					itemVersionId: item.versionId,
					itemRowVersion: item.rowVersion,
					occurredAt: resolveBehaviorOccurredAt(date, time, defaultInstant),
					detail: detail.trim() || null,
					studentIds: selected.map((s) => s.id),
					rosterToken
				};
			pending = true;
			const result = await createBehaviorBatch(frozen);
			if (disposed) return;
			receipt = `บันทึก${sideLabels[result.side]}คนละ ${result.pointsPerStudent} คะแนน ให้ ${result.affectedStudents} คนแล้ว`;
			frozen = null;
			selected = [];
			detail = '';
			attempted = false;
		} catch (e) {
			if (!disposed) error = e instanceof Error ? e.message : 'บันทึกไม่ได้';
			if (e instanceof ApiClientError && e.status < 500) {
				frozen = null;
				if (e.status === 409) error += ' กรุณาโหลดเกณฑ์และรายชื่อใหม่ ตรวจคะแนน แล้วบันทึกอีกครั้ง';
			}
		} finally {
			if (!disposed) pending = false;
		}
	}
</script>

<form class="mx-auto max-w-4xl space-y-6" onsubmit={submit}>
	{#if error}<PageState
			variant="error"
			title="บันทึกไม่สำเร็จ"
			description={error}
		/>{/if}{#if receipt}<p
			role="status"
			class="rounded-md border border-success/30 bg-success/10 p-4"
		>
			{receipt}
		</p>{/if}
	<fieldset class="space-y-4" disabled={pending || !!frozen}>
		<legend class="mb-3 font-semibold">รายการพฤติกรรม</legend>
		<div class="grid gap-4 sm:grid-cols-2">
			<div>
				<Label for="behavior-side">ประเภท</Label><BehaviorSelect
					id="behavior-side"
					label="ประเภท"
					value={side}
					options={sides.map((s) => ({ value: s, label: sideLabels[s] }))}
					onValueChange={(v) => {
						const match = sides.find((s) => s === v);
						if (match) {
							side = match;
							itemId = '';
						}
					}}
				/>
			</div>
			<div>
				<Label for="behavior-item">รายการ *</Label><BehaviorSelect
					id="behavior-item"
					label="รายการพฤติกรรม"
					value={itemId}
					options={items
						.filter((i) => i.side === side && i.enabled && i.versionId !== null)
						.map((i) => ({ value: i.id, label: `${i.name} · ${i.points} คะแนน` }))}
					onValueChange={(v) => (itemId = v)}
				/>{#if fieldErrors.item}<p class="text-sm text-destructive">{fieldErrors.item}</p>{/if}
			</div>
		</div>
		<Button
			type="button"
			variant="outline"
			onclick={() => {
				defaultInstant = new Date().toISOString();
				defaultClock = bangkokDateTime(new Date(defaultInstant));
				date = defaultClock.date;
				time = defaultClock.time;
			}}>ใช้วันเวลาปัจจุบัน</Button
		>
		<p class="text-sm text-muted-foreground">
			คะแนนตามเกณฑ์: {item?.points ?? '—'} คะแนน · ครูแก้จำนวนคะแนนในหน้าบันทึกไม่ได้
		</p>
		<div class="grid gap-4 sm:grid-cols-2">
			<div>
				<Label for="behavior-date">วันที่เกิดเหตุ *</Label><DatePicker
					id="behavior-date"
					value={date}
					onValueChange={(v) => {
						if (v) date = v;
					}}
				/>
			</div>
			<div>
				<Label for="behavior-time">เวลาเกิดเหตุ *</Label><Input
					id="behavior-time"
					type="time"
					step="1"
					bind:value={time}
					required
				/>
			</div>
		</div>
		{#if date !== loadedDate || error}<LoadingButton
				type="button"
				variant="outline"
				loading={refreshing}
				onclick={() => void reload()}>โหลดเกณฑ์และรายชื่อใหม่</LoadingButton
			>{/if}{#if fieldErrors.date}<p class="text-sm text-destructive">{fieldErrors.date}</p>{/if}
		<div>
			<Label for="behavior-detail">รายละเอียดเพิ่มเติม</Label><Textarea
				id="behavior-detail"
				bind:value={detail}
				maxlength={5000}
				placeholder="อธิบายเหตุการณ์เพิ่มเติมได้"
			/>
		</div>
		<BehaviorStudentPicker
			{term}
			{date}
			initial={roster}
			bind:selected
			onRosterChange={(token) => (rosterToken = token)}
		/>{#if fieldErrors.students}<p class="text-sm text-destructive">{fieldErrors.students}</p>{/if}
	</fieldset>
	<div class="rounded-md border bg-primary/5 p-4">
		{sideLabels[side]}คนละ {item?.points ?? '—'} คะแนน · นักเรียน {selected.length} คน · วันที่ {date}
		เวลา {time}
	</div>
	{#if frozen}<p class="text-sm text-warning">
			ยังยืนยันผลการบันทึกไม่ได้ กดบันทึกอีกครั้งเพื่อตรวจคำขอเดิม โดยไม่สร้างคะแนนซ้ำ
		</p>{/if}
	<div class="flex justify-end gap-2">
		<Button
			type="button"
			variant="outline"
			disabled={pending || !!frozen}
			onclick={() => void goto(`/staff/behavior?academicTermId=${term}`)}>กลับ</Button
		><LoadingButton type="submit" loading={pending} disabled={refreshing}
			>บันทึกพฤติกรรม</LoadingButton
		>
	</div>
</form>
