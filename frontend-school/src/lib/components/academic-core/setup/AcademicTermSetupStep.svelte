<script lang="ts">
	import type {
		AcademicTerm,
		AcademicTermType,
		AcademicYear,
		BellSchedule,
		CreateAcademicTermRequest,
		UpdateAcademicTermRequest
	} from '$lib/api/academic-core';
	import {
		customNameFromStored,
		standardTermName
	} from '$lib/academic-core/foundation-presentation';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import * as Collapsible from '$lib/components/ui/collapsible';
	import { DatePicker } from '$lib/components/ui/date-picker';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import { ChevronDown, Pencil, Plus, Save, X } from 'lucide-svelte';

	const TERM_TYPES: Array<{ value: AcademicTermType; label: string; description: string }> = [
		{ value: 'regular', label: 'ภาคปกติ', description: 'ภาคเรียนหลัก เช่น ภาคเรียนที่ 1 และ 2' },
		{ value: 'summer', label: 'ภาคฤดูร้อน', description: 'รอบเรียนเพิ่มเติมหลังภาคปกติ' },
		{ value: 'remedial', label: 'ภาคซ่อมเสริม', description: 'รอบสำหรับเรียนหรือประเมินซ่อมเสริม' },
		{ value: 'custom', label: 'รอบกำหนดเอง', description: 'รอบพิเศษที่ไม่ตรงกับประเภทข้างต้น' }
	];

	let {
		year,
		schedules,
		terms,
		busy = false,
		onCreate,
		onUpdate,
		onSaved
	}: {
		year: AcademicYear;
		schedules: BellSchedule[];
		terms: AcademicTerm[];
		busy?: boolean;
		onCreate: (draft: CreateAcademicTermRequest) => Promise<AcademicTerm>;
		onUpdate: (id: string, draft: UpdateAcademicTermRequest) => Promise<AcademicTerm>;
		onSaved: (term: AcademicTerm) => void;
	} = $props();

	let editing = $state<AcademicTerm | null>(null);
	let termType = $state<AcademicTermType>('regular');
	let customName = $state('');
	let startDate = $state('');
	let endDate = $state('');
	let bellScheduleId = $derived(
		schedules.find((schedule) => schedule.isDefault)?.id ?? schedules[0]?.id ?? ''
	);
	let includedInYearResult = $state(true);
	let blocksYearClosure = $state(true);
	let advancedOpen = $state(false);
	let errorMessage = $state('');

	const nextSequence = $derived(
		terms.length === 0 ? 1 : Math.max(...terms.map((term) => term.sequence)) + 1
	);
	const draftSequence = $derived(editing?.sequence ?? nextSequence);
	const previewName = $derived(customName.trim() || standardTermName(termType, draftSequence));
	const selectedSchedule = $derived(
		schedules.find((schedule) => schedule.id === bellScheduleId) ?? null
	);

	function resetDraft() {
		editing = null;
		termType = 'regular';
		customName = '';
		startDate = '';
		endDate = '';
		bellScheduleId = schedules.find((schedule) => schedule.isDefault)?.id ?? schedules[0]?.id ?? '';
		includedInYearResult = true;
		blocksYearClosure = true;
		advancedOpen = false;
	}

	function beginEdit(term: AcademicTerm) {
		editing = term;
		termType = term.termType;
		customName = customNameFromStored(term.name, standardTermName(term.termType, term.sequence));
		startDate = term.startDate;
		endDate = term.endDate;
		bellScheduleId = term.bellScheduleId;
		includedInYearResult = term.includedInYearResult;
		blocksYearClosure = term.blocksYearClosure;
		advancedOpen = customName.length > 0;
		errorMessage = '';
	}

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		errorMessage = '';
		if (!startDate || !endDate || !bellScheduleId) {
			errorMessage = 'กรุณาเลือกวันที่และตารางเวลาให้ครบ';
			return;
		}
		if (startDate > endDate) {
			errorMessage = 'วันสิ้นสุดภาคเรียนต้องไม่อยู่ก่อนวันเริ่ม';
			return;
		}
		const common = {
			termType,
			customName: customName.trim() || null,
			startDate,
			endDate,
			includedInYearResult,
			blocksYearClosure,
			bellScheduleId
		};
		try {
			const saved = editing
				? await onUpdate(editing.id, { ...common, rowVersion: editing.rowVersion })
				: await onCreate({ academicYearId: year.id, ...common });
			onSaved(saved);
			resetDraft();
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'บันทึกภาคเรียนไม่สำเร็จ';
		}
	}
</script>

<div class="space-y-5">
	<div class="space-y-2">
		{#each terms as term (term.id)}
			<div
				class="flex flex-wrap items-center justify-between gap-3 rounded-xl border bg-background p-4"
			>
				<div>
					<div class="flex flex-wrap items-center gap-2">
						<p class="font-medium">{term.name}</p>
						<Badge variant="outline"
							>{term.status === 'planning' ? 'ฉบับเตรียมการ' : term.status}</Badge
						>
					</div>
					<p class="mt-1 text-xs text-muted-foreground">
						{term.startDate} – {term.endDate} · {schedules.find(
							(schedule) => schedule.id === term.bellScheduleId
						)?.name ?? 'ไม่พบตารางเวลาที่อ้างอิง'}
					</p>
				</div>
				{#if term.status === 'planning'}
					<Button type="button" size="sm" variant="outline" onclick={() => beginEdit(term)}>
						<Pencil class="size-4" /> แก้ไข
					</Button>
				{/if}
			</div>
		{:else}
			<p class="rounded-xl border border-dashed p-5 text-sm text-muted-foreground">
				ยังไม่มีภาคเรียน สร้างหลังจากกำหนดตารางเวลาและคาบเรียนแล้ว
			</p>
		{/each}
	</div>

	<form class="space-y-5 rounded-xl border bg-card p-4" onsubmit={submit}>
		<div class="flex items-start justify-between gap-3">
			<div>
				<p class="font-semibold">{editing ? `แก้ไข ${editing.name}` : 'เพิ่มภาคเรียน'}</p>
				<p class="mt-1 text-xs text-muted-foreground">
					ระบบจัดลำดับและสร้างชื่อมาตรฐานให้เอง: <strong>{previewName}</strong>
				</p>
			</div>
			{#if editing}
				<Button
					type="button"
					size="icon-sm"
					variant="ghost"
					onclick={resetDraft}
					aria-label="ยกเลิกการแก้ไขภาคเรียน"
				>
					<X class="size-4" />
				</Button>
			{/if}
		</div>

		<div class="space-y-1.5">
			<Label for="term-type">ประเภทภาคเรียน</Label>
			<Select.Root type="single" bind:value={termType}>
				<Select.Trigger id="term-type" class="w-full">
					{TERM_TYPES.find((option) => option.value === termType)?.label ?? 'เลือกประเภท'}
				</Select.Trigger>
				<Select.Content>
					{#each TERM_TYPES as option (option.value)}
						<Select.Item value={option.value}>
							{option.label} — {option.description}
						</Select.Item>
					{/each}
				</Select.Content>
			</Select.Root>
		</div>

		<div class="grid gap-4 sm:grid-cols-2">
			<div class="space-y-1.5">
				<Label for="term-start">วันเริ่มภาคเรียน</Label>
				<DatePicker
					id="term-start"
					bind:value={startDate}
					ariaLabel="เลือกวันเริ่มภาคเรียน"
					required
				/>
			</div>
			<div class="space-y-1.5">
				<Label for="term-end">วันสิ้นสุดภาคเรียน</Label>
				<DatePicker
					id="term-end"
					bind:value={endDate}
					ariaLabel="เลือกวันสิ้นสุดภาคเรียน"
					required
				/>
			</div>
		</div>

		<div class="space-y-1.5">
			<Label for="term-bell-schedule">ตารางเวลาที่ใช้</Label>
			<Select.Root type="single" bind:value={bellScheduleId}>
				<Select.Trigger id="term-bell-schedule" class="w-full">
					{selectedSchedule?.name ?? 'เลือกตารางเวลา'}
				</Select.Trigger>
				<Select.Content>
					{#each schedules as schedule (schedule.id)}
						<Select.Item value={schedule.id}>
							{schedule.name}{schedule.isDefault ? ' · ตารางหลัก' : ''}
						</Select.Item>
					{/each}
				</Select.Content>
			</Select.Root>
		</div>

		<Collapsible.Root bind:open={advancedOpen} class="rounded-xl border bg-muted/20">
			<Collapsible.Trigger
				class="flex w-full items-center justify-between gap-3 px-4 py-3 text-left text-sm font-medium"
			>
				<span>ตัวเลือกเพิ่มเติม</span>
				<ChevronDown class="size-4 text-muted-foreground" aria-hidden="true" />
			</Collapsible.Trigger>
			<Collapsible.Content class="space-y-4 border-t px-4 py-4">
				<div class="space-y-1.5">
					<Label for="term-custom-name">ชื่อแสดงผลอื่น (ไม่บังคับ)</Label>
					<Input id="term-custom-name" bind:value={customName} placeholder={previewName} />
					<p class="text-xs text-muted-foreground">
						เว้นว่างเพื่อใช้ชื่อที่ระบบสร้างตามประเภทและลำดับจริง
					</p>
				</div>
				<label class="flex cursor-pointer items-start gap-3 text-sm">
					<Checkbox bind:checked={includedInYearResult} class="mt-0.5" />
					<span>
						<strong class="block font-medium">รวมผลภาคเรียนนี้ในผลทั้งปี</strong>
						<span class="text-xs text-muted-foreground"
							>ใช้เมื่อระบบผลการเรียนพร้อมใช้งานในอนาคต</span
						>
					</span>
				</label>
				<label class="flex cursor-pointer items-start gap-3 text-sm">
					<Checkbox bind:checked={blocksYearClosure} class="mt-0.5" />
					<span>
						<strong class="block font-medium">ต้องจัดการภาคเรียนนี้ก่อนปิดปี</strong>
						<span class="text-xs text-muted-foreground">เป็นกติกาสำหรับขั้นตอนปิดปีในอนาคต</span>
					</span>
				</label>
			</Collapsible.Content>
		</Collapsible.Root>

		{#if errorMessage}
			<p
				role="alert"
				class="rounded-lg border border-destructive/30 bg-destructive/5 p-3 text-sm text-destructive"
			>
				{errorMessage}
			</p>
		{/if}

		<Button type="submit" disabled={busy || schedules.length === 0}>
			{#if editing}<Save class="size-4" /> บันทึกภาคเรียน{:else}<Plus class="size-4" /> เพิ่มภาคเรียน{/if}
		</Button>
	</form>
</div>
