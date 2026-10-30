<script lang="ts">
	import type {
		AcademicContextSettings,
		AcademicContextSettingsMutationResponse,
		AcademicTerm,
		AcademicYear,
		UpdateAcademicContextSettingsRequest
	} from '#lib/api/academic-core.js';
	import { LoadingButton } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';

	let {
		year,
		terms,
		settings,
		canManage,
		onsave,
		onreload
	}: {
		year: AcademicYear;
		terms: AcademicTerm[];
		settings: AcademicContextSettings;
		canManage: boolean;
		onreload: () => Promise<void>;
		onsave: (
			draft: UpdateAcademicContextSettingsRequest
		) => Promise<AcademicContextSettingsMutationResponse>;
	} = $props();
	let pending = $state('');
	let error = $state('');
	const yearIsDefault = $derived(settings.defaultAcademicYearId === year.id);

	async function save(action: string, changes: Partial<UpdateAcademicContextSettingsRequest>) {
		if (!canManage || pending) return;
		pending = action;
		error = '';
		try {
			await onsave({
				rowVersion: settings.rowVersion,
				defaultAcademicYearId: settings.defaultAcademicYearId,
				defaultAcademicTermId: settings.defaultAcademicTermId,
				years: [],
				terms: [],
				...changes
			});
		} catch (cause) {
			error = cause instanceof Error ? cause.message : 'บันทึกตัวเลือกไม่สำเร็จ';
		} finally {
			pending = '';
		}
	}
</script>

<section
	class="space-y-4 rounded-2xl border bg-card p-4 sm:p-5"
	aria-label="ตัวเลือกและค่าเริ่มต้น"
>
	<div>
		<h3 class="font-semibold">ตัวเลือกและค่าเริ่มต้นของโรงเรียน</h3>
		<p class="mt-1 text-xs text-muted-foreground">
			ปีและภาคเรียนใหม่ซ่อนอยู่จนกว่าจะเลือกแสดง ค่าเริ่มต้นใช้เมื่อเปิดหน้าที่ยังไม่ได้เลือกบริบท
			ข้อมูลและแบบฟอร์มที่เปิดอยู่ยังใช้ปีและภาคเรียนเดิม
		</p>
	</div>
	<div class="flex flex-wrap items-center justify-between gap-3" role="group" aria-label={year.name}>
		<div class="flex flex-wrap items-center gap-2">
			<strong class="text-sm">{year.name}</strong>
			<Badge variant="outline">{year.isVisible ? 'แสดงในตัวเลือก' : 'ซ่อนจากตัวเลือก'}</Badge>
			{#if yearIsDefault}<Badge variant="secondary">ค่าเริ่มต้นของโรงเรียน</Badge>{/if}
		</div>
		{#if canManage}
			<div class="flex flex-wrap gap-2">
				<LoadingButton
					type="button"
					size="sm"
					variant="outline"
					loading={pending === 'year-visibility'}
					loadingLabel="กำลังบันทึก..."
					disabled={!!pending || yearIsDefault}
					title={yearIsDefault ? 'เปลี่ยนค่าเริ่มต้นก่อนซ่อนปีนี้' : undefined}
					onclick={() =>
						save('year-visibility', { years: [{ id: year.id, isVisible: !year.isVisible }] })}
					>{year.isVisible ? 'ซ่อนจากตัวเลือก' : 'แสดงในตัวเลือก'}</LoadingButton
				>
				<LoadingButton
					type="button"
					size="sm"
					variant="outline"
					loading={pending === 'year-default'}
					loadingLabel="กำลังตั้งค่า..."
					disabled={!!pending || !year.isVisible || yearIsDefault}
					onclick={() =>
						save('year-default', { defaultAcademicYearId: year.id, defaultAcademicTermId: null })}
					>ตั้งเป็นค่าเริ่มต้น</LoadingButton
				>
			</div>
		{/if}
	</div>
	{#if !year.isVisible}
		<p class="text-xs text-muted-foreground">
			เมื่อซ่อนปีนี้ ภาคเรียนทั้งหมดในปีนี้จะไม่ปรากฏในตัวเลือกด้วย
		</p>
	{/if}
	{#each terms as term (term.id)}
		<div class="flex flex-wrap items-center justify-between gap-3 border-t pt-3" role="group" aria-label={term.name}>
			<div class="flex flex-wrap items-center gap-2 text-sm">
				<span>{term.name}</span>
				<Badge variant="outline"
					>{term.isVisible && year.isVisible ? 'แสดงในตัวเลือก' : 'ซ่อนจากตัวเลือก'}</Badge
				>
				{#if settings.defaultAcademicTermId === term.id}<Badge variant="secondary"
						>ค่าเริ่มต้นของโรงเรียน</Badge
					>{/if}
			</div>
			{#if canManage}
				<div class="flex flex-wrap gap-2">
					<LoadingButton
						type="button"
						size="sm"
						variant="outline"
						loading={pending === `term-visibility:${term.id}`}
						loadingLabel="กำลังบันทึก..."
						disabled={!!pending || settings.defaultAcademicTermId === term.id}
						title={settings.defaultAcademicTermId === term.id
							? 'เปลี่ยนค่าเริ่มต้นก่อนซ่อนภาคเรียนนี้'
							: undefined}
						onclick={() =>
							save(`term-visibility:${term.id}`, {
								terms: [{ id: term.id, isVisible: !term.isVisible }]
							})}>{term.isVisible ? 'ซ่อนจากตัวเลือก' : 'แสดงในตัวเลือก'}</LoadingButton
					>
					<LoadingButton
						type="button"
						size="sm"
						variant="outline"
						loading={pending === `term-default:${term.id}`}
						loadingLabel="กำลังตั้งค่า..."
						disabled={!!pending ||
							!year.isVisible ||
							!term.isVisible ||
							settings.defaultAcademicTermId === term.id}
						onclick={() =>
							save(`term-default:${term.id}`, {
								defaultAcademicYearId: year.id,
								defaultAcademicTermId: term.id
							})}>ตั้งเป็นค่าเริ่มต้น</LoadingButton
					>
				</div>
			{/if}
		</div>
	{/each}
	{#if error}<p
			role="alert"
			class="rounded-lg border border-destructive/30 bg-destructive/5 p-3 text-sm text-destructive"
		>
			{error}
		</p>
		<Button type="button" size="sm" variant="outline" disabled={!!pending} onclick={onreload}>โหลดข้อมูลล่าสุด</Button>{/if}
</section>
