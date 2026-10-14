<script lang="ts">
	import { formatBehaviorDate } from '#lib/features/behavior/presentation.js';
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
	import BehaviorFormSection from './BehaviorFormSection.svelte';
	import * as ToggleGroup from '#lib/components/ui/toggle-group/index.js';
	import { Award, ShieldAlert, LockKeyhole, Save } from '@lucide/svelte';
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
	const defaultClock = $derived(bangkokDateTime(new Date(defaultInstant)));
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
	function chooseSide(value: string) {
		const next = sides.find((candidate) => candidate === value);
		if (next && next !== side) {
			side = next;
			itemId = '';
		}
	}
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

<form class="max-w-5xl space-y-6" onsubmit={submit}>
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
	<fieldset class="min-w-0 space-y-6" disabled={pending || !!frozen}>
		<legend class="sr-only">ข้อมูลบันทึกพฤติกรรม</legend>
		<BehaviorFormSection
			step={1}
			title="รายการพฤติกรรม"
			description="เลือกประเภทและรายการ คะแนนใช้ตามเกณฑ์ของโรงเรียน"
		>
			<div class="grid gap-4 sm:grid-cols-2">
				<div class="space-y-2">
					<Label id="behavior-side-label">ประเภท</Label>
					<ToggleGroup.Root
						type="single"
						bind:value={() => side, chooseSide}
						aria-labelledby="behavior-side-label"
						class="mt-2 grid grid-cols-2 gap-2"
						disabled={pending || !!frozen}
					>
						{#each sides as option (option)}
							<ToggleGroup.Item
								value={option}
								class="min-h-[44px] min-w-[44px] gap-2 data-[state=on]:border-primary data-[state=on]:bg-primary/10 data-[state=on]:text-primary"
							>
								{#if option === 'merit'}<Award
										class="size-4 shrink-0"
										aria-hidden="true"
									/>{:else}<ShieldAlert class="size-4 shrink-0" aria-hidden="true" />{/if}
								{option === 'merit' ? 'ความดี' : 'ความผิด'}
							</ToggleGroup.Item>
						{/each}
					</ToggleGroup.Root>
				</div>
				<div>
					<BehaviorSelect
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
			<div class="flex items-start gap-3 rounded-xl border border-primary/20 bg-primary/5 p-4">
				<LockKeyhole class="mt-0.5 size-4 shrink-0 text-primary" aria-hidden="true" />
				<div>
					<p class="font-medium">คะแนนตามเกณฑ์: {item?.points ?? '—'} คะแนนต่อคน</p>
					<p class="mt-1 text-sm text-muted-foreground">จำนวนคะแนนกำหนดจากรายการที่เลือก</p>
				</div>
			</div>
		</BehaviorFormSection>
		<BehaviorFormSection
			step={2}
			title="วันเวลาและรายละเอียด"
			description="ใช้เวลาปัจจุบันเป็นค่าเริ่มต้น หรือเปลี่ยนตามวันที่เกิดเหตุ"
		>
			<Button
				class="min-h-[44px] min-w-[44px]"
				type="button"
				variant="outline"
				onclick={() => {
					defaultInstant = new Date().toISOString();

					date = defaultClock.date;
					time = defaultClock.time;
				}}>ใช้วันเวลาปัจจุบัน</Button
			>
			<div class="grid gap-4 sm:grid-cols-2">
				<div class="space-y-2">
					<Label for="behavior-date">วันที่เกิดเหตุ *</Label><DatePicker
						id="behavior-date"
						value={date}
						onValueChange={(v) => {
							if (v) date = v;
						}}
					/>
				</div>
				<div class="space-y-2">
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
					class="min-h-[44px] min-w-[44px]"
					type="button"
					variant="outline"
					loading={refreshing}
					onclick={() => void reload()}>โหลดเกณฑ์และรายชื่อใหม่</LoadingButton
				>{/if}{#if fieldErrors.date}<p class="text-sm text-destructive">{fieldErrors.date}</p>{/if}
			<div class="space-y-2">
				<Label for="behavior-detail">รายละเอียดเพิ่มเติม</Label><Textarea
					id="behavior-detail"
					bind:value={detail}
					maxlength={5000}
					placeholder="อธิบายเหตุการณ์เพิ่มเติมได้"
				/>
			</div>
		</BehaviorFormSection>
		<BehaviorFormSection
			step={3}
			title="นักเรียน"
			description="เลือกเป็นรายคน ทั้งห้อง หรือผสมจากหลายห้องได้"
		>
			<BehaviorStudentPicker
				{term}
				{date}
				initial={roster}
				bind:selected
				onRosterChange={(token) => (rosterToken = token)}
			/>{#if fieldErrors.students}<p class="text-sm text-destructive">
					{fieldErrors.students}
				</p>{/if}
		</BehaviorFormSection>
	</fieldset>
	<div
		class="rounded-xl border border-primary/20 bg-gradient-to-r from-primary/10 to-card p-4 sm:p-5"
		aria-label="สรุปก่อนบันทึก"
	>
		<p class="mb-2 text-sm font-medium text-primary">ตรวจข้อมูลก่อนบันทึก</p>
		{sideLabels[side]}คนละ {item?.points ?? '—'} คะแนน · นักเรียน {selected.length} คน · วันที่ {formatBehaviorDate(
			date
		)}
		เวลา {time}
	</div>
	{#if frozen}<p class="text-sm text-warning">
			ยังยืนยันผลการบันทึกไม่ได้ กดบันทึกอีกครั้งเพื่อตรวจคำขอเดิม โดยไม่สร้างคะแนนซ้ำ
		</p>{/if}
	<div class="flex flex-wrap justify-end gap-2 border-t pt-4">
		<Button
			class="min-h-[44px] min-w-[44px]"
			type="button"
			variant="outline"
			disabled={pending || !!frozen}
			onclick={() => void goto(`/staff/behavior?academicTermId=${term}`)}>กลับ</Button
		><LoadingButton
			class="min-h-[44px] min-w-[44px]"
			type="submit"
			loading={pending}
			disabled={refreshing}><Save class="size-4" aria-hidden="true" />บันทึกพฤติกรรม</LoadingButton
		>
	</div>
</form>
