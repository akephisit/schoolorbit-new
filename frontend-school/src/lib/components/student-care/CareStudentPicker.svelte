<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { careStudents, type CareStudentOption } from '#lib/api/student-care.js';
	import { careIdentity } from '#lib/features/student-care/access.js';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { LoadingButton, PageState } from '#lib/components/app-state/index.js';
	let {
		initial,
		term,
		date,
		value = [],
		onchange,
		multiple = false
	}: {
		initial: CareStudentOption[];
		term: string;
		date: string;
		value?: CareStudentOption[];
		onchange: (students: CareStudentOption[]) => void;
		multiple?: boolean;
	} = $props();
	let students = $state(untrack(() => initial)),
		search = $state(''),
		applied = $state(''),
		pending = $state(false),
		error = $state(''),
		after = $state('');
	const request = new LatestRequest();
	onDestroy(() => request.abort());
	async function load(next = false) {
		const token = request.begin();
		const identity = $careIdentity;
		pending = true;
		error = '';
		try {
			const result = await careStudents(
				{
					academicTermId: term,
					date,
					search: search.trim() || undefined,
					afterId: next ? students.at(-1)?.id : undefined,
					limit: 50
				},
				{ signal: token.signal }
			);
			if (request.isCurrent(token.revision) && identity === $careIdentity) {
				students = result;
				applied = search.trim();
				after = next ? 'next' : '';
			}
		} catch (cause) {
			if (request.isCurrent(token.revision) && identity === $careIdentity)
				error = cause instanceof Error ? cause.message : 'โหลดรายชื่อไม่ได้';
		} finally {
			if (request.isCurrent(token.revision) && identity === $careIdentity) pending = false;
		}
	}
	function select(person: CareStudentOption, checked: boolean) {
		onchange(
			checked
				? multiple
					? [...value.filter((s) => s.id !== person.id), person]
					: [person]
				: value.filter((s) => s.id !== person.id)
		);
	}
</script>

<fieldset class="space-y-3">
	<legend class="text-sm font-medium">นักเรียน *</legend>
	<div class="flex flex-wrap gap-2">
		<Input
			class="min-w-40 flex-1"
			aria-label="ค้นหานักเรียน"
			bind:value={search}
			placeholder="ชื่อหรือรหัสนักเรียน"
			onkeydown={(event) => {
				if (event.key === 'Enter') {
					event.preventDefault();
					void load();
				}
			}}
		/><LoadingButton type="button" variant="outline" loading={pending} onclick={() => load()}
			>ค้นหา</LoadingButton
		>
	</div>
	<div class="max-h-64 space-y-2 overflow-y-auto rounded-lg border p-3" aria-busy={pending}>
		{#each students as student (student.id)}<div class="flex items-start gap-2">
				<Checkbox
					id={`student-care-${student.id}`}
					checked={value.some((s) => s.id === student.id)}
					onCheckedChange={(checked) => select(student, checked)}
					disabled={pending}
				/><Label for={`student-care-${student.id}`} class="min-w-0 break-words"
					>{student.name}<span class="ml-2 font-normal text-muted-foreground"
						>{student.studentCode ?? ''} · {student.homeroom}</span
					></Label
				>
			</div>{/each}
		{#if !students.length}<p class="text-sm text-muted-foreground">
				{applied ? 'ไม่พบรายชื่อตามคำค้น' : 'ไม่มีนักเรียนในขอบเขตที่เข้าถึงได้'}
			</p>{/if}
	</div>
	<div class="flex flex-wrap items-center justify-between gap-2">
		<span class="text-sm text-muted-foreground">เลือก {value.length} คน</span>
		<div class="flex gap-2">
			{#if after}<Button type="button" variant="ghost" disabled={pending} onclick={() => load()}
					>หน้าแรก</Button
				>{/if}{#if students.length === 50}<Button
					type="button"
					variant="outline"
					disabled={pending}
					onclick={() => load(true)}>ถัดไป</Button
				>{/if}
		</div>
	</div>
	{#if value.length}<div class="flex flex-wrap gap-2">
			{#each value as student (student.id)}<Button
					type="button"
					variant="secondary"
					size="sm"
					onclick={() => select(student, false)}
					aria-label={`นำ ${student.name} ออก`}>{student.name} ×</Button
				>{/each}
		</div>{/if}
	{#if error}<PageState
			variant="error"
			title="โหลดรายชื่อไม่ได้"
			description={error}
			actionLabel="ลองโหลดรายชื่อใหม่"
			onaction={() => load()}
		/>{/if}
</fieldset>
