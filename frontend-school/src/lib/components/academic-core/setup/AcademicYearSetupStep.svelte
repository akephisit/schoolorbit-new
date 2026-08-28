<script lang="ts">
	import type {
		AcademicYear,
		CreateAcademicYearRequest,
		UpdateAcademicYearRequest
	} from '$lib/api/academic-core';
	import {
		customNameFromStored,
		normalizeSchoolDays,
		standardAcademicYearName,
		type AcademicWeekday
	} from '$lib/academic-core/foundation-presentation';
	import { Button } from '$lib/components/ui/button';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import * as Collapsible from '$lib/components/ui/collapsible';
	import { DatePicker } from '$lib/components/ui/date-picker';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { ChevronDown, Save } from 'lucide-svelte';

	const WEEKDAYS: Array<{ code: AcademicWeekday; label: string; short: string }> = [
		{ code: 'MON', label: 'วันจันทร์', short: 'จ' },
		{ code: 'TUE', label: 'วันอังคาร', short: 'อ' },
		{ code: 'WED', label: 'วันพุธ', short: 'พ' },
		{ code: 'THU', label: 'วันพฤหัสบดี', short: 'พฤ' },
		{ code: 'FRI', label: 'วันศุกร์', short: 'ศ' },
		{ code: 'SAT', label: 'วันเสาร์', short: 'ส' },
		{ code: 'SUN', label: 'วันอาทิตย์', short: 'อา' }
	];

	let {
		existing = null,
		suggestedYear,
		busy = false,
		onCreate,
		onUpdate,
		onSaved
	}: {
		existing?: AcademicYear | null;
		suggestedYear: number;
		busy?: boolean;
		onCreate: (draft: CreateAcademicYearRequest) => Promise<AcademicYear>;
		onUpdate: (id: string, draft: UpdateAcademicYearRequest) => Promise<AcademicYear>;
		onSaved: (year: AcademicYear) => void;
	} = $props();

	let year = $derived(existing?.year ?? suggestedYear);
	let customName = $derived(
		existing ? customNameFromStored(existing.name, standardAcademicYearName(existing.year)) : ''
	);
	let startDate = $derived(existing?.startDate ?? '');
	let endDate = $derived(existing?.endDate ?? '');
	let schoolDays = $derived<AcademicWeekday[]>(
		normalizeSchoolDays(existing?.schoolDays ?? ['MON', 'TUE', 'WED', 'THU', 'FRI'])
	);
	let advancedOpen = $derived(customName.length > 0);
	let errorMessage = $state('');

	const standardName = $derived(standardAcademicYearName(year));
	const displayName = $derived(customName.trim() || standardName);

	function toggleDay(day: AcademicWeekday, checked: boolean) {
		schoolDays = normalizeSchoolDays(
			checked ? [...schoolDays, day] : schoolDays.filter((item) => item !== day)
		);
	}

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		errorMessage = '';
		if (!startDate || !endDate) {
			errorMessage = 'กรุณาเลือกวันเริ่มและวันสิ้นสุดปีการศึกษา';
			return;
		}
		if (startDate > endDate) {
			errorMessage = 'วันสิ้นสุดปีการศึกษาต้องไม่อยู่ก่อนวันเริ่ม';
			return;
		}
		if (schoolDays.length === 0) {
			errorMessage = 'กรุณาเลือกวันเรียนอย่างน้อย 1 วัน';
			return;
		}
		const common = {
			year,
			customName: customName.trim() || null,
			startDate,
			endDate,
			schoolDays
		};
		try {
			const saved = existing
				? await onUpdate(existing.id, { ...common, rowVersion: existing.rowVersion })
				: await onCreate(common);
			onSaved(saved);
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'บันทึกปีการศึกษาไม่สำเร็จ';
		}
	}
</script>

<form class="space-y-5" onsubmit={submit}>
	<div class="rounded-xl border border-primary/20 bg-primary/[0.04] p-4">
		<p class="text-xs font-medium text-primary">ชื่อที่ระบบจะใช้</p>
		<p class="mt-1 text-lg font-semibold">{displayName}</p>
		<p class="mt-1 text-xs text-muted-foreground">
			ชื่อมาตรฐานจะเปลี่ยนตามปี พ.ศ. โดยอัตโนมัติ จึงไม่เกิดปี 2571 ที่ชื่อเป็น 2569
		</p>
	</div>

	<div class="grid gap-4 sm:grid-cols-2">
		<div class="space-y-1.5">
			<Label for="academic-year-value">ปีการศึกษา (พ.ศ.)</Label>
			<Input id="academic-year-value" type="number" min="2400" bind:value={year} required />
		</div>
		<div class="space-y-1.5">
			<Label for="academic-year-start">วันเริ่มปีการศึกษา</Label>
			<DatePicker
				id="academic-year-start"
				bind:value={startDate}
				ariaLabel="เลือกวันเริ่มปีการศึกษา"
				required
			/>
		</div>
		<div class="space-y-1.5 sm:col-start-2">
			<Label for="academic-year-end">วันสิ้นสุดปีการศึกษา</Label>
			<DatePicker
				id="academic-year-end"
				bind:value={endDate}
				ariaLabel="เลือกวันสิ้นสุดปีการศึกษา"
				required
			/>
		</div>
	</div>

	<fieldset class="space-y-2">
		<legend class="text-sm font-medium">วันเรียนปกติของปีนี้</legend>
		<p class="text-xs text-muted-foreground">
			ใช้ตรวจว่าคาบเรียนถูกกำหนดเฉพาะวันที่โรงเรียนเปิดเรียน
		</p>
		<div class="grid grid-cols-4 gap-2 sm:grid-cols-7">
			{#each WEEKDAYS as day (day.code)}
				<label
					class="flex cursor-pointer items-center justify-center gap-2 rounded-lg border bg-background px-2 py-2.5 text-sm hover:bg-muted/50"
					title={day.label}
				>
					<Checkbox
						checked={schoolDays.includes(day.code)}
						onCheckedChange={(checked) => toggleDay(day.code, checked ?? false)}
						aria-label={`เลือก${day.label}`}
					/>
					<span>{day.short}</span>
				</label>
			{/each}
		</div>
	</fieldset>

	<Collapsible.Root bind:open={advancedOpen} class="rounded-xl border bg-muted/20">
		<Collapsible.Trigger
			class="flex w-full items-center justify-between gap-3 px-4 py-3 text-left text-sm font-medium"
		>
			<span>ตัวเลือกเพิ่มเติม: ใช้ชื่อแสดงผลอื่น</span>
			<ChevronDown class="size-4 text-muted-foreground" aria-hidden="true" />
		</Collapsible.Trigger>
		<Collapsible.Content class="space-y-2 border-t px-4 py-4">
			<Label for="academic-year-custom-name">ชื่อแสดงผลอื่น (ไม่บังคับ)</Label>
			<Input id="academic-year-custom-name" bind:value={customName} placeholder={standardName} />
			<p class="text-xs text-muted-foreground">
				เว้นว่างเพื่อให้ระบบใช้ “{standardName}” และปรับตามปีโดยอัตโนมัติ
			</p>
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

	<Button type="submit" disabled={busy}>
		<Save class="size-4" />
		{existing ? 'บันทึกปีการศึกษา' : 'สร้างปีสำหรับวางแผน'}
	</Button>
</form>
