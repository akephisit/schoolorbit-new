<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import {
		behaviorStudents,
		type BehaviorStudent,
		type BehaviorStudentPage
	} from '#lib/api/behavior.js';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { PageState, LoadingButton } from '#lib/components/app-state/index.js';
	import BehaviorSelect from './BehaviorSelect.svelte';
	let {
		term,
		date,
		initial,
		selected = $bindable([]),
		onRosterChange
	}: {
		term: string;
		date: string;
		initial: BehaviorStudentPage;
		selected?: BehaviorStudent[];
		onRosterChange: (token: string) => void;
	} = $props();
	let page = $state(untrack(() => initial));
	let search = $state('');
	let grade = $state('');
	let room = $state('');
	let offset = $state(0);
	let pending = $state(false);
	let error = $state('');
	let previousDate = untrack(() => date);
	let previousInitial = untrack(() => initial);
	const request = new LatestRequest();
	const limit = 50;
	onDestroy(() => request.abort());
	$effect.pre(() => {
		if (initial !== previousInitial) {
			previousInitial = initial;
			page = initial;
			offset = 0;
		}
		if (date !== previousDate) {
			previousDate = date;
			request.abort();
			pending = false;
			error = 'วันที่เปลี่ยน กรุณาโหลดรายชื่อใหม่และตรวจนักเรียนที่เลือก';
			page = { ...page, items: [] };
		}
	});
	function add(students: BehaviorStudent[]) {
		const all = new SvelteMap(selected.map((s) => [s.id, s]));
		for (const s of students) all.set(s.id, s);
		selected = [...all.values()];
	}
	async function load(next = 0, all = false) {
		const token = request.begin();
		pending = true;
		error = '';
		try {
			const result = await behaviorStudents(
				{
					academicTermId: term,
					date,
					search: search.trim() || undefined,
					gradeLevelId: grade || undefined,
					homeroomId: room || undefined,
					offset: all ? 0 : next,
					limit: all ? 10000 : limit
				},
				{ signal: token.signal }
			);
			if (!request.isCurrent(token.revision)) return;
			if (all) {
				if (result.total > result.items.length)
					throw new Error('รายชื่อเกินขนาดที่เลือกได้ กรุณาเลือกเป็นกลุ่มย่อย');
				add(result.items);
			} else {
				page = result;
				offset = next;
			}
			onRosterChange(result.rosterToken);
		} catch (e) {
			if (request.isCurrent(token.revision))
				error = e instanceof Error ? e.message : 'โหลดรายชื่อไม่ได้';
		} finally {
			if (request.isCurrent(token.revision)) pending = false;
		}
	}
</script>

<section class="space-y-4" aria-label="เลือกนักเรียน">
	<div class="grid gap-3 rounded-xl border bg-muted/20 p-3 sm:grid-cols-3 sm:p-4">
		<div>
			<BehaviorSelect
				id="behavior-grade"
				label="ระดับชั้น"
				value={grade}
				options={[
					{ value: '', label: 'ทุกระดับชั้น' },
					...page.options.gradeLevels.map((o) => ({ value: o.id, label: o.label }))
				]}
				onValueChange={(v) => {
					grade = v;
					room = '';
					void load();
				}}
			/>
		</div>
		<div>
			<BehaviorSelect
				id="behavior-room"
				label="ห้องเรียน"
				value={room}
				options={[
					{ value: '', label: 'ทุกห้อง' },
					...page.options.homerooms.map((o) => ({ value: o.id, label: o.label }))
				]}
				onValueChange={(v) => {
					room = v;
					void load();
				}}
			/>
		</div>
		<div class="space-y-2">
			<Label for="behavior-search">ชื่อหรือรหัสนักเรียน</Label>
			<div class="flex gap-2">
				<Input id="behavior-search" bind:value={search} /><Button
					type="button"
					onclick={() => void load()}
					variant="outline"
					disabled={pending}>ค้นหา</Button
				>
			</div>
		</div>
	</div>
	{#if error}<PageState
			variant="error"
			title="ตรวจสอบรายชื่อ"
			description={error}
			actionLabel="โหลดรายชื่อใหม่"
			onaction={() => void load()}
		/>{/if}
	<div class="flex flex-wrap items-center gap-2 border-b pb-3">
		<LoadingButton
			type="button"
			variant="outline"
			loading={pending}
			onclick={() => void load(0, true)}>เลือกทั้งหมดตามตัวกรอง {page.total} คน</LoadingButton
		><span aria-live="polite">เลือกไว้ {selected.length} คน</span><Button
			type="button"
			variant="ghost"
			onclick={() => (selected = [])}>ล้างการเลือก</Button
		>
	</div>
	<div class="max-h-80 divide-y overflow-y-auto rounded-xl border bg-card" aria-busy={pending}>
		{#each page.items as student (student.id)}<label
				class="flex [@media(any-pointer:coarse)]:min-h-[44px] cursor-pointer items-center gap-3 px-3 py-2 transition-colors hover:bg-accent/50 has-[[data-state=checked]]:bg-primary/5"
				><Checkbox
					aria-label={`เลือก ${student.name}`}
					checked={selected.some((s) => s.id === student.id)}
					onCheckedChange={(checked) =>
						checked ? add([student]) : (selected = selected.filter((s) => s.id !== student.id))}
				/><span class="min-w-0 flex-1 break-words"
					>{student.name}<span class="block text-xs text-muted-foreground"
						>{student.studentCode ?? '—'} · {student.gradeLevel} · {student.homeroom}</span
					></span
				></label
			>{:else}<p class="p-4 text-muted-foreground">ไม่พบรายชื่อ</p>{/each}
	</div>
	<div class="flex items-center justify-between gap-3 text-sm">
		<Button
			type="button"
			variant="outline"
			disabled={pending || offset === 0}
			onclick={() => void load(Math.max(0, offset - limit))}>ก่อนหน้า</Button
		><span>{page.total ? offset + 1 : 0}–{Math.min(offset + limit, page.total)} / {page.total}</span
		><Button
			type="button"
			variant="outline"
			disabled={pending || offset + limit >= page.total}
			onclick={() => void load(offset + limit)}>ถัดไป</Button
		>
	</div>
	{#if selected.length}<details class="rounded-xl border bg-muted/20 px-4">
			<summary class="[@media(any-pointer:coarse)]:min-h-[44px] cursor-pointer py-2"
				>ตรวจรายชื่อที่เลือก {selected.length} คน</summary
			>
			<ul class="max-h-60 divide-y overflow-y-auto pb-2">
				{#each selected as student (student.id)}<li
						class="flex [@media(any-pointer:coarse)]:min-h-[44px] items-center justify-between gap-2"
					>
						<span>{student.name} · {student.homeroom}</span><Button
							type="button"
							variant="ghost"
							aria-label={`นำ ${student.name} ออก`}
							onclick={() => (selected = selected.filter((s) => s.id !== student.id))}>นำออก</Button
						>
					</li>{/each}
			</ul>
		</details>{/if}
</section>
