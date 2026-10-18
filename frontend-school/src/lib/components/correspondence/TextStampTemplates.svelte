<script lang="ts">
	import { tick } from 'svelte';
	import type { CorrespondenceTextStampTemplate } from '#lib/api/correspondence.js';
	import { correspondenceUuidV7 } from '#lib/correspondence/uuid-v7.js';
	import { stampTokens } from '#lib/correspondence/text-stamps.js';
	import {
		focusCorrespondenceField,
		type CorrespondenceFieldErrors
	} from '#lib/correspondence/validation.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import FieldError from './FieldError.svelte';
	let {
		templates = $bindable([]),
		errors,
		disabled
	}: {
		templates?: CorrespondenceTextStampTemplate[];
		errors: CorrespondenceFieldErrors;
		disabled: boolean;
	} = $props();
	let addedId = $state('');
	function add() {
		const id = correspondenceUuidV7();
		templates = [...templates, { id, name: '', body: '', border: false }];
		addedId = id;
		void focusCorrespondenceField(`stamp-template-${id}-name`);
	}
	async function insertToken(template: CorrespondenceTextStampTemplate, token: string) {
		const input = document.getElementById(`stamp-template-${template.id}-body`);
		const start =
			input instanceof HTMLTextAreaElement ? input.selectionStart : template.body.length;
		const end = input instanceof HTMLTextAreaElement ? input.selectionEnd : start;
		template.body = `${template.body.slice(0, start)}{${token}}${template.body.slice(end)}`;
		await tick();
		if (input instanceof HTMLTextAreaElement) {
			input.focus();
			input.setSelectionRange(start + token.length + 2, start + token.length + 2);
		}
	}
</script>

<section class="space-y-3" aria-label="แม่แบบข้อความบนเอกสาร">
	<div class="flex flex-wrap items-center justify-between gap-2">
		<h3 class="font-medium">แม่แบบข้อความบนเอกสาร</h3>
		<Button
			type="button"
			variant="outline"
			size="sm"
			disabled={disabled || templates.length >= 30}
			onclick={add}>เพิ่มแม่แบบข้อความ</Button
		>
	</div>
	<p class="text-sm text-muted-foreground">
		เลือกใช้บนเอกสารแล้วแก้ข้อความและตำแหน่งได้ การบันทึกที่นี่ไม่เปลี่ยนข้อความที่ใช้ไปแล้ว
	</p>
	{#each templates as template (template.id)}
		<details open={addedId === template.id} class="rounded-lg border p-3">
			<summary class="cursor-pointer font-medium">{template.name || 'แม่แบบข้อความใหม่'}</summary>
			<div class="mt-3 space-y-3">
				<div class="space-y-2">
					<Label for={`stamp-template-${template.id}-name`}>ชื่อแม่แบบข้อความ *</Label><Input
						id={`stamp-template-${template.id}-name`}
						bind:value={template.name}
						{disabled}
						aria-invalid={!!errors[`stamp-template-${template.id}-name`]}
						aria-describedby={errors[`stamp-template-${template.id}-name`]
							? `stamp-template-${template.id}-name-error`
							: undefined}
					/><FieldError
						id={`stamp-template-${template.id}-name-error`}
						message={errors[`stamp-template-${template.id}-name`]}
					/>
				</div>
				<div class="space-y-2">
					<Label for={`stamp-template-${template.id}-body`}>ข้อความแม่แบบ *</Label><Textarea
						id={`stamp-template-${template.id}-body`}
						bind:value={template.body}
						rows={5}
						{disabled}
						aria-invalid={!!errors[`stamp-template-${template.id}-body`]}
						aria-describedby={errors[`stamp-template-${template.id}-body`]
							? `stamp-template-${template.id}-body-error`
							: undefined}
					/><FieldError
						id={`stamp-template-${template.id}-body-error`}
						message={errors[`stamp-template-${template.id}-body`]}
					/>
				</div>
				<div class="flex flex-wrap gap-1" aria-label="แทรกข้อมูลในแม่แบบ">
					{#each stampTokens as token (token.value)}<Button
							type="button"
							variant="outline"
							size="sm"
							{disabled}
							onclick={() => insertToken(template, token.value)}>{token.label}</Button
						>{/each}
				</div>
				<div class="flex items-center gap-2">
					<Checkbox
						id={`stamp-template-${template.id}-border`}
						bind:checked={template.border}
						{disabled}
					/><Label for={`stamp-template-${template.id}-border`}>แสดงกรอบข้อความ</Label>
				</div>
				<div class="flex justify-end">
					<Button
						type="button"
						variant="ghost"
						size="sm"
						{disabled}
						aria-label={`ลบแม่แบบข้อความ ${template.name || 'ใหม่'}`}
						onclick={() => {
							templates = templates.filter((item) => item.id !== template.id);
						}}>ลบแม่แบบข้อความ</Button
					>
				</div>
			</div>
		</details>
	{/each}
	{#if !templates.length}<p class="text-sm text-muted-foreground">
			ยังไม่มีแม่แบบข้อความ เพิ่มเพื่อใช้ซ้ำบนเอกสาร
		</p>{/if}
</section>
