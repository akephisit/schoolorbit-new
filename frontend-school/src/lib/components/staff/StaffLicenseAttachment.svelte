<script lang="ts">
	import { Upload, Trash2 } from '@lucide/svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { uploadFile } from '#lib/api/files.js';
	import StaffLicensePreview from './StaffLicensePreview.svelte';
	let {
		value = $bindable(null),
		staffId,
		disabled = false,
		onPendingChange
	}: {
		value: string | null;
		staffId: string;
		disabled?: boolean;
		onPendingChange?: (pending: boolean) => void;
	} = $props();
	let pending = $state(false),
		error = $state(''),
		filename = $state(''),
		uploadedId = $state<string | null>(null),
		removing = $state(false);
	let epoch = 0;
	let fileInputKey = $state(0);
	$effect(() => {
		// Superseded uploads remain temporary and are reclaimed by Files retention.
		if (!staffId || disabled) pending = false;
		return () => {
			epoch++;
			pending = false;
			onPendingChange?.(false);
		};
	});
	async function upload(event: Event) {
		const input = event.currentTarget as HTMLInputElement;
		const file = input.files?.[0];
		fileInputKey++;
		if (!file || disabled || pending) return;
		error = '';
		if (!['application/pdf', 'image/jpeg', 'image/png', 'image/webp'].includes(file.type)) {
			error = 'เลือกไฟล์ PDF, JPG, PNG หรือ WebP';
			return;
		}
		if (file.size === 0 || file.size > 10 * 1024 * 1024) {
			error = 'เลือกไฟล์ที่มีข้อมูลและขนาดไม่เกิน 10 MB';
			return;
		}
		const revision = ++epoch,
			resource = staffId;
		pending = true;
		onPendingChange?.(true);
		try {
			const uploaded = await uploadFile(file, 'teaching_license', resource);
			if (revision !== epoch || resource !== staffId || disabled) return;
			value = uploaded.id;
			uploadedId = uploaded.id;
			filename = uploaded.displayFilename;
		} catch (cause) {
			if (revision === epoch) error = cause instanceof Error ? cause.message : 'อัปโหลดไม่สำเร็จ';
		} finally {
			if (revision === epoch) {
				pending = false;
				onPendingChange?.(false);
			}
		}
	}
</script>

<section class="space-y-2 rounded-lg border bg-muted/10 p-3" aria-label="เอกสารใบประกอบวิชาชีพ">
	<div class="flex flex-wrap items-center justify-between gap-2">
		<div class="min-w-0">
			<h3 class="text-sm font-medium">เอกสารใบประกอบวิชาชีพ</h3>
			<p class="mt-1 text-xs text-muted-foreground">
				PDF, JPG, PNG หรือ WebP · ไม่เกิน 10 MB · 1 ไฟล์
			</p>
		</div>
		{#if value}<div class="flex items-center gap-2">
				{#key `${staffId}:${value}`}<StaffLicensePreview fileId={value} {staffId} />{/key}
				<Button
					type="button"
					variant="ghost"
					size="icon-sm"
					aria-label="นำเอกสารใบประกอบวิชาชีพออก"
					disabled={disabled || pending}
					onclick={() => (removing = true)}><Trash2 class="size-4" /></Button
				>
			</div>{/if}
	</div>
	{#if filename && value && value === uploadedId}<p class="text-xs break-all">{filename}</p>{/if}
	<div class="flex flex-wrap items-center gap-2">
		<Label for="staff-license-document" class="sr-only"
			>{value ? 'เปลี่ยนไฟล์ใบประกอบวิชาชีพ' : 'แนบใบประกอบวิชาชีพ'}</Label
		>
		{#key fileInputKey}<Input
				id="staff-license-document"
				type="file"
				accept="application/pdf,image/jpeg,image/png,image/webp"
				disabled={disabled || pending}
				onchange={upload}
				class="h-auto min-w-0 flex-1"
				aria-describedby="staff-license-file-help"
			/>{/key}
		{#if pending}<span role="status" class="flex items-center gap-1 text-xs"
				><Upload class="size-4 animate-pulse" />กำลังอัปโหลด…</span
			>{/if}
	</div>
	<p id="staff-license-file-help" class="text-xs text-muted-foreground">
		กดบันทึกการเปลี่ยนแปลงเพื่อยืนยันไฟล์ที่แนบหรือการนำออก
	</p>
	{#if error}<p role="alert" class="text-sm text-destructive">{error}</p>{/if}
</section>
<Dialog.Root bind:open={removing}
	><Dialog.Content
		><Dialog.Header
			><Dialog.Title>นำเอกสารออก?</Dialog.Title><Dialog.Description
				>เอกสารเดิมจะถูกนำออกเมื่อบันทึกการเปลี่ยนแปลง</Dialog.Description
			></Dialog.Header
		><Dialog.Footer
			><Button variant="outline" onclick={() => (removing = false)}>ยกเลิก</Button><Button
				variant="destructive"
				onclick={() => {
					value = null;
					filename = '';
					removing = false;
				}}>นำออก</Button
			></Dialog.Footer
		></Dialog.Content
	></Dialog.Root
>
