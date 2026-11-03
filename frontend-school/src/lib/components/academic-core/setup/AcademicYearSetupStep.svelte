<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import type {
		AcademicYear,
		CreateAcademicYearRequest,
		UpdateAcademicYearRequest
	} from '#lib/api/academic-core.js';
	import {
		customNameFromStored,
		standardAcademicYearName
	} from '#lib/academic-core/foundation-presentation.js';
	import { yearDraftBase, yearDraftMetadataMatches } from '#lib/academic-core/context-drafts.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Collapsible from '#lib/components/ui/collapsible/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { ChevronDown, Save } from '@lucide/svelte';

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

	let year = $state(untrack(() => existing?.year ?? suggestedYear));
	let customName = $state(
		untrack(() =>
			existing ? customNameFromStored(existing.name, standardAcademicYearName(existing.year)) : ''
		)
	);
	let advancedOpen = $state(untrack(() => customName.length > 0));
	let errorMessage = $state('');

	let draftBase = $state(untrack(() => (existing ? yearDraftBase(existing) : null)));
	const metadataChanged = $derived(
		!!existing && !!draftBase && !yearDraftMetadataMatches(draftBase, existing)
	);

	const standardName = $derived(standardAcademicYearName(year));
	const displayName = $derived(customName.trim() || standardName);

	let disposed = false;
	onDestroy(() => {
		disposed = true;
	});

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (busy || metadataChanged) return;
		errorMessage = '';
		const common = {
			year,
			customName: customName.trim() || null,
			defaultSchoolDays: existing?.defaultSchoolDays ?? ['MON', 'TUE', 'WED', 'THU', 'FRI']
		};
		const identity = appIdentityKey();
		try {
			const saved = existing
				? await onUpdate(existing.id, { ...common, rowVersion: existing.rowVersion })
				: await onCreate(common);
			if (disposed || identity !== appIdentityKey()) return;
			onSaved(saved);
		} catch (error) {
			if (disposed || identity !== appIdentityKey()) return;
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
	</div>

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

	{#if metadataChanged && existing}
		<div
			role="alert"
			class="space-y-2 rounded-lg border border-amber-500/30 bg-amber-500/5 p-3 text-sm"
		>
			<p class="font-medium">ข้อมูลปีการศึกษาที่บันทึกไว้เปลี่ยนระหว่างแก้ไข</p>
			<p>
				ข้อมูลล่าสุด: {existing.name} · ปี {existing.year}
			</p>
			<p>ฉบับร่างของคุณยังอยู่ ตรวจสอบกับข้อมูลล่าสุดก่อนบันทึก</p>
			<Button
				type="button"
				variant="outline"
				size="sm"
				disabled={busy}
				onclick={() => {
					if (existing) draftBase = yearDraftBase(existing);
				}}>ตรวจสอบแล้ว ใช้ข้อมูลล่าสุดเป็นฐาน</Button
			>
		</div>
	{/if}

	{#if errorMessage}
		<p
			role="alert"
			class="rounded-lg border border-destructive/30 bg-destructive/5 p-3 text-sm text-destructive"
		>
			{errorMessage}
		</p>
	{/if}

	<Button type="submit" disabled={busy || metadataChanged}>
		<Save class="size-4" />
		{existing ? 'บันทึกปีการศึกษา' : 'สร้างปีสำหรับวางแผน'}
	</Button>
</form>
