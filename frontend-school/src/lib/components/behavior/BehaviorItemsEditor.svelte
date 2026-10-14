<script lang="ts">
	import { untrack, onDestroy } from 'svelte';
	import { ApiClientError } from '#lib/api/client.js';
	import { saveBehaviorItem, type BehaviorItem, type SaveBehaviorItem } from '#lib/api/behavior.js';
	import { sides, sideLabels, bangkokDateTime } from '#lib/features/behavior/presentation.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { DatePicker } from '#lib/components/ui/date-picker/index.js';
	import { LoadingButton, PageState } from '#lib/components/app-state/index.js';
	import BehaviorSelect from './BehaviorSelect.svelte';
	import * as Table from '#lib/components/ui/table/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import BehaviorFormSection from './BehaviorFormSection.svelte';
	import { ListChecks } from '@lucide/svelte';
	let { initial }: { initial: BehaviorItem[] } = $props();
	let items = $state(untrack(() => initial));
	const blank = (): SaveBehaviorItem => ({
		requestId: crypto.randomUUID(),
		id: null,
		rowVersion: null,
		name: '',
		side: 'merit',
		enabled: true,
		effectiveFrom: bangkokDateTime().date,
		points: 1
	});
	let draft = $state(blank());
	let pending = $state(false);
	let frozen = $state.raw<SaveBehaviorItem | null>(null);
	let disposed = false;
	onDestroy(() => (disposed = true));
	let error = $state('');
	async function save(e: SubmitEvent) {
		e.preventDefault();
		pending = true;
		error = '';
		try {
			frozen ??= { ...draft };
			const result = await saveBehaviorItem(frozen);
			if (disposed) return;
			frozen = null;
			items = [...items.filter((i) => i.id !== result.id), result];
			draft = blank();
		} catch (e) {
			if (!disposed) error = e instanceof Error ? e.message : 'บันทึกไม่ได้';
			if (e instanceof ApiClientError && e.status < 500) frozen = null;
		} finally {
			if (!disposed) pending = false;
		}
	}
	function edit(item: BehaviorItem) {
		draft = {
			requestId: crypto.randomUUID(),
			id: item.id,
			rowVersion: item.rowVersion,
			name: item.name,
			side: item.side,
			enabled: item.enabled,
			effectiveFrom: bangkokDateTime().date,
			points: item.points ?? 0
		};
	}
</script>

<BehaviorFormSection
	title="รายการทำดี / ทำผิด"
	description="คะแนนในหน้าบันทึกถูกล็อกตามเกณฑ์ การเปลี่ยนเกณฑ์ไม่เปลี่ยนรายการเก่า"
	icon={ListChecks}
>
	<div class="overflow-hidden rounded-xl border">
		<Table.Root
			><Table.Header
				><Table.Row
					><Table.Head>รายการ</Table.Head><Table.Head>ประเภท</Table.Head><Table.Head
						>คะแนน</Table.Head
					><Table.Head>สถานะ</Table.Head><Table.Head></Table.Head></Table.Row
				></Table.Header
			><Table.Body
				>{#each items as item (item.id)}<Table.Row
						><Table.Cell class="whitespace-normal"
							><span class="font-medium">{item.name}</span></Table.Cell
						><Table.Cell class="whitespace-normal">{sideLabels[item.side]}</Table.Cell><Table.Cell
							class="whitespace-normal"
							><span class="font-semibold tabular-nums">{item.points ?? 'ยังไม่มีเกณฑ์'}</span
							></Table.Cell
						><Table.Cell class="whitespace-normal"
							><Badge variant={item.enabled ? 'outline' : 'secondary'}
								>{item.enabled ? 'เปิดใช้' : 'ปิดใช้'}</Badge
							></Table.Cell
						><Table.Cell class="text-end"
							><Button
								type="button"
								variant="outline"
								disabled={pending || !!frozen}
								onclick={() => edit(item)}>แก้ไข / ตั้งเกณฑ์ใหม่</Button
							></Table.Cell
						></Table.Row
					>{:else}<Table.Row
						><Table.Cell colspan={5} class="p-6 text-center text-muted-foreground"
							>ยังไม่มีรายการ</Table.Cell
						></Table.Row
					>{/each}</Table.Body
			></Table.Root
		>
	</div>
	<form class="space-y-3 border-t pt-4" onsubmit={save}>
		<h3 class="font-semibold">{draft.id ? 'แก้ไขรายการ' : 'เพิ่มรายการ'}</h3>
		{#if error}<PageState variant="error" title="บันทึกไม่สำเร็จ" description={error} />{/if}
		<fieldset disabled={pending || !!frozen} class="grid gap-3 sm:grid-cols-2">
			<div class="space-y-2">
				<Label for="behavior-item-name">ชื่อรายการ *</Label><Input
					id="behavior-item-name"
					bind:value={draft.name}
					required
					maxlength={200}
				/>
			</div>
			<div>
				<BehaviorSelect
					id="behavior-item-side"
					label="ฝั่งรายการ"
					value={draft.side}
					options={sides.map((s) => ({ value: s, label: sideLabels[s] }))}
					onValueChange={(v) => {
						const s = sides.find((s) => s === v);
						if (s) draft.side = s;
					}}
				/>
			</div>
			<div class="space-y-2">
				<Label for="behavior-item-points">คะแนน *</Label><Input
					id="behavior-item-points"
					type="number"
					min="1"
					bind:value={draft.points}
					required
				/>
			</div>
			<div class="space-y-2">
				<Label for="behavior-item-effective">วันเริ่มใช้เกณฑ์ *</Label><DatePicker
					id="behavior-item-effective"
					value={draft.effectiveFrom}
					onValueChange={(v) => {
						if (v) draft.effectiveFrom = v;
					}}
				/>
			</div>
			<label class="flex [@media(any-pointer:coarse)]:min-h-[44px] items-center gap-2"
				><Checkbox bind:checked={draft.enabled} />เปิดใช้งานรายการ</label
			>
		</fieldset>
		<div class="flex justify-end gap-2">
			<Button
				type="button"
				variant="outline"
				disabled={pending || !!frozen}
				onclick={() => (draft = blank())}>ยกเลิกการแก้ไข</Button
			><LoadingButton type="submit" loading={pending}>บันทึกรายการ</LoadingButton>
		</div>
	</form>
</BehaviorFormSection>
