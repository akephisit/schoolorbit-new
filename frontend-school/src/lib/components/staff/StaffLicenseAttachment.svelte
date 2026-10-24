<script lang="ts">
	import { Trash2 } from '@lucide/svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import LocalDocumentPreview from '#lib/components/files/LocalDocumentPreview.svelte';
	import StaffLicensePreview from './StaffLicensePreview.svelte';
	let {
		value = $bindable(null),
		file = $bindable(null),
		staffId,
		disabled = false
	}: {
		value: string | null;
		file?: File | null;
		staffId: string;
		disabled?: boolean;
	} = $props();
	let error = $state(''),
		removing = $state(false);
	let fileInputKey = $state(0);
	function select(event: Event) {
		const input = event.currentTarget as HTMLInputElement;
		const selected = input.files?.[0];
		fileInputKey++;
		if (!selected || disabled) return;
		error = '';
		if (!['application/pdf', 'image/jpeg', 'image/png', 'image/webp'].includes(selected.type)) {
			error = 'เลือกไฟล์ PDF, JPG, PNG หรือ WebP';
			return;
		}
		if (selected.size === 0 || selected.size > 10 * 1024 * 1024) {
			error = 'เลือกไฟล์ที่มีข้อมูลและขนาดไม่เกิน 10 MB';
			return;
		}
		file = selected;
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
		{#if value || file}<div class="flex items-center gap-2">
				{#if value}{#key `${staffId}:${value}`}<StaffLicensePreview
							fileId={value}
							{staffId}
						/>{/key}{/if}
				<Button
					type="button"
					variant="ghost"
					size="icon-sm"
					aria-label="นำเอกสารใบประกอบวิชาชีพออก"
					{disabled}
					onclick={() => (removing = true)}><Trash2 class="size-4" /></Button
				>
			</div>{/if}
	</div>
	{#if file}<LocalDocumentPreview {file} />{/if}
	<div class="flex flex-wrap items-center gap-2">
		<Label for="staff-license-document" class="sr-only"
			>{value ? 'เปลี่ยนไฟล์ใบประกอบวิชาชีพ' : 'แนบใบประกอบวิชาชีพ'}</Label
		>
		{#key fileInputKey}<Input
				id="staff-license-document"
				type="file"
				accept="application/pdf,image/jpeg,image/png,image/webp"
				{disabled}
				onchange={select}
				class="h-auto min-w-0 flex-1"
				aria-describedby="staff-license-file-help"
			/>{/key}
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
					file = null;
					removing = false;
				}}>นำออก</Button
			></Dialog.Footer
		></Dialog.Content
	></Dialog.Root
>
