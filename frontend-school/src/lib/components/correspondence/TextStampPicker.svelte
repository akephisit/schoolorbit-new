<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import {
		getCorrespondenceOptions,
		type CorrespondenceOptions,
		type CorrespondenceSummary
	} from '#lib/api/correspondence.js';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import {
		expandStamp,
		stampTokens,
		stampTemplateErrors
	} from '#lib/correspondence/text-stamps.js';
	import {
		focusCorrespondenceField,
		type CorrespondenceFieldErrors
	} from '#lib/correspondence/validation.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { LoadingButton, PageState } from '#lib/components/app-state/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import Choice from './Choice.svelte';
	import FieldError from './FieldError.svelte';
	let {
		record,
		documentRevision,
		disabled,
		oninsert
	}: {
		record: CorrespondenceSummary;
		documentRevision: number;
		disabled: boolean;
		oninsert: (text: string, border: boolean) => string | null;
	} = $props();
	let open = $state(false),
		options = $state<CorrespondenceOptions | null>(null),
		loading = $state(false),
		error = $state('');
	let templateId = $state(''),
		body = $state(''),
		instruction = $state(''),
		signerTitle = $state(''),
		schoolName = $state(''),
		concrete = $state(''),
		border = $state(false),
		prepared = $state(false);
	let fields = $state<CorrespondenceFieldErrors>({});
	const request = new LatestRequest();
	let ownerKey = '';
	$effect.pre(() => {
		const key = `${record.id}:${documentRevision}`;
		untrack(() => {
			if (key !== ownerKey) {
				ownerKey = key;
				request.abort();
				open = false;
				options = null;
				loading = false;
				prepared = false;
				concrete = '';
				fields = {};
				error = '';
			}
		});
	});
	const templates = $derived(
		options?.routingSettings.find((item) => item.register === record.register)
			?.textStampTemplates ?? []
	);
	onDestroy(() => request.abort());
	async function load() {
		const ticket = request.begin();
		loading = true;
		error = '';
		try {
			const result = await getCorrespondenceOptions('', { signal: ticket.signal }, record.register);
			if (!request.isCurrent(ticket.revision)) return;
			options = result;
			const first = result.routingSettings.find((item) => item.register === record.register)
				?.textStampTemplates?.[0];
			if (first) select(first.id);
		} catch (cause) {
			if (request.isCurrent(ticket.revision) && !isAbortError(cause))
				error = cause instanceof Error ? cause.message : 'โหลดแม่แบบข้อความไม่สำเร็จ';
		} finally {
			if (request.isCurrent(ticket.revision)) loading = false;
		}
	}
	function select(id: string) {
		templateId = id;
		const selected = templates.find((item) => item.id === id);
		body = selected?.body ?? '';
		border = selected?.border ?? false;
		signerTitle = options?.stampContext.signerTitle ?? '';
		schoolName = options?.stampContext.schoolName ?? '';
		instruction = '';
		prepared = false;
		fields = {};
		error = '';
	}
	function invalid(id: string, message: string) {
		fields = { [id]: message };
		void focusCorrespondenceField(id);
	}
	function submit(event: SubmitEvent) {
		event.preventDefault();
		if (disabled || !options) return;
		fields = {};
		error = '';
		if (!prepared) {
			if (!templateId) {
				invalid('stamp-choice', 'เลือกแม่แบบข้อความ');
				return;
			}
			const invalidBody = stampTemplateErrors({ name: 'แม่แบบ', body }).body;
			if (invalidBody) {
				invalid('stamp-body', invalidBody);
				return;
			}
			if (body.includes('{schoolName}') && !schoolName.trim()) {
				invalid('stamp-school', 'ระบุชื่อโรงเรียนสำหรับข้อความนี้');
				return;
			}
			const result = expandStamp(
				body,
				{ ...options.stampContext, schoolName: schoolName.trim() },
				record,
				{ instruction, signerTitle }
			);
			for (const [key, message] of Object.entries(result.errors))
				fields[`stamp-${key === 'signerTitle' ? 'title' : key}`] = message;
			const first = Object.keys(fields)[0];
			if (first) {
				void focusCorrespondenceField(first);
				return;
			}
			concrete = result.text;
			prepared = true;
			void focusCorrespondenceField('stamp-concrete');
		} else {
			if (!concrete.trim() || [...concrete].length > 5000) {
				invalid('stamp-concrete', 'ระบุข้อความไม่เกิน 5,000 ตัวอักษร');
				return;
			}
			const invalidPlacement = oninsert(concrete.trim(), border);
			if (invalidPlacement) {
				invalid('stamp-concrete', invalidPlacement);
				return;
			}
			open = false;
		}
	}
</script>

<Button
	type="button"
	variant="outline"
	{disabled}
	onclick={() => {
		open = true;
		if (!options) void load();
	}}>แม่แบบข้อความ / ตรารับ</Button
>
<Dialog.Root
	bind:open
	onOpenChange={(value) => {
		if (!value) {
			request.abort();
			loading = false;
		}
	}}
>
	<Dialog.Content class="max-h-[90dvh] overflow-auto sm:max-w-xl">
		<Dialog.Header
			><Dialog.Title>เพิ่มข้อความจากแม่แบบ</Dialog.Title><Dialog.Description
				>ตรวจข้อความก่อนวางบนเอกสาร แล้วย้ายหรือปรับขนาดกรอบได้</Dialog.Description
			></Dialog.Header
		>
		{#if error}<PageState
				variant="error"
				title={error}
				actionLabel="ลองอีกครั้ง"
				onaction={load}
			/>{/if}
		{#if loading}<p role="status" class="text-sm text-muted-foreground">
				กำลังโหลดแม่แบบข้อความ…
			</p>{:else if options}
			<form class="space-y-4" onsubmit={submit}>
				{#if !prepared}
					<Choice
						id="stamp-choice"
						label="แม่แบบข้อความ"
						value={templateId}
						onValueChange={select}
						options={templates.map((item) => ({ value: item.id, label: item.name }))}
						error={fields['stamp-choice']}
						{disabled}
					/>
					{#if !templates.length}<p class="text-sm text-muted-foreground">
							ยังไม่มีแม่แบบข้อความสำหรับหนังสือประเภทนี้
						</p>{/if}
					{#if body.includes('{schoolName}')}<div class="space-y-2">
							<Label for="stamp-school">ชื่อโรงเรียน *</Label><Input
								id="stamp-school"
								bind:value={schoolName}
								aria-invalid={!!fields['stamp-school']}
								aria-describedby={fields['stamp-school'] ? 'stamp-school-error' : undefined}
							/><FieldError id="stamp-school-error" message={fields['stamp-school']} />
						</div>{/if}
					{#if body.includes('{instruction}')}<div class="space-y-2">
							<Label for="stamp-instruction">ข้อความสั่งการ *</Label><Textarea
								id="stamp-instruction"
								bind:value={instruction}
								rows={3}
								aria-invalid={!!fields['stamp-instruction']}
								aria-describedby={fields['stamp-instruction']
									? 'stamp-instruction-error'
									: undefined}
							/><FieldError id="stamp-instruction-error" message={fields['stamp-instruction']} />
						</div>{/if}
					{#if body.includes('{signerTitle}')}<div class="space-y-2">
							<Label for="stamp-title">ตำแหน่งที่แสดง *</Label><Input
								id="stamp-title"
								bind:value={signerTitle}
								aria-invalid={!!fields['stamp-title']}
								aria-describedby={fields['stamp-title'] ? 'stamp-title-error' : undefined}
							/><FieldError id="stamp-title-error" message={fields['stamp-title']} />
						</div>{/if}
					<details>
						<summary class="cursor-pointer text-sm font-medium">แก้ไขแม่แบบเฉพาะครั้งนี้</summary>
						<div class="mt-3 space-y-2">
							<Label for="stamp-body">ข้อความแม่แบบ</Label><Textarea
								id="stamp-body"
								bind:value={body}
								rows={5}
								aria-invalid={!!fields['stamp-body']}
								aria-describedby={fields['stamp-body'] ? 'stamp-body-error' : undefined}
							/><FieldError id="stamp-body-error" message={fields['stamp-body']} />
							<div class="flex flex-wrap gap-1">
								{#each stampTokens as token (token.value)}<Button
										type="button"
										variant="outline"
										size="sm"
										onclick={() => {
											body += `{${token.value}}`;
										}}>{token.label}</Button
									>{/each}
							</div>
						</div>
					</details>
				{:else}<div class="space-y-2">
						<Label for="stamp-concrete">ข้อความที่จะวาง *</Label><Textarea
							id="stamp-concrete"
							bind:value={concrete}
							rows={7}
							aria-invalid={!!fields['stamp-concrete']}
							aria-describedby={fields['stamp-concrete'] ? 'stamp-concrete-error' : undefined}
						/><FieldError id="stamp-concrete-error" message={fields['stamp-concrete']} />
					</div>
					<div class="flex items-center gap-2">
						<Checkbox id="stamp-border" bind:checked={border} /><Label for="stamp-border"
							>แสดงกรอบข้อความ</Label
						>
					</div>{/if}
				<div class="flex justify-end gap-2">
					{#if prepared}<Button
							type="button"
							variant="outline"
							onclick={() => {
								prepared = false;
							}}>ย้อนกลับ</Button
						>{:else}<Button type="button" variant="outline" onclick={() => (open = false)}
							>ยกเลิก</Button
						>{/if}<LoadingButton type="submit" {disabled}
						>{prepared ? 'วางบนเอกสาร' : 'ตรวจข้อความ'}</LoadingButton
					>
				</div>
			</form>
		{/if}
	</Dialog.Content>
</Dialog.Root>
