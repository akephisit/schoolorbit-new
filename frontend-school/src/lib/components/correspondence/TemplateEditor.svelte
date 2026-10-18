<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import {
		saveProcessTemplate,
		type ProcessTemplate,
		type ProcessTemplateStep,
		type CorrespondenceOptions
	} from '#lib/api/correspondence.js';
	import type { TemplateSeed } from '#lib/correspondence/template-presets.js';
	import { stepLabels } from '#lib/correspondence/presentation.js';
	import { LoadingButton, PageState } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import Choice from './Choice.svelte';
	import Recipients from './Recipients.svelte';
	let {
		template,
		seed,
		options,
		onsaved,
		oncancel
	}: {
		template?: ProcessTemplate;
		seed?: TemplateSeed;
		options: CorrespondenceOptions;
		onsaved: (template: ProcessTemplate) => void;
		oncancel: () => void;
	} = $props();
	function emptyStep(): ProcessTemplateStep {
		return {
			title: 'รับทราบ',
			kind: 'acknowledge',
			completionPolicy: 'all',
			recipients: [],
			requiresSignature: false,
			requiresComment: false,
			requiresEvidence: false,
			requiresDelegatedAuthority: false
		};
	}
	let title = $state(untrack(() => template?.title ?? seed?.title ?? '')),
		active = $state(untrack(() => template?.active ?? true)),
		steps = $state<ProcessTemplateStep[]>(
			untrack(
				() =>
					(template?.steps ?? seed?.steps)?.map((step) => ({
						...step,
						recipients: (step.recipients ?? []).map((recipient) => ({ ...recipient }))
					})) ?? [emptyStep()]
			)
		),
		pending = $state(false),
		error = $state('');
	const code = untrack(() => template?.code ?? `flow_${crypto.randomUUID().replaceAll('-', '')}`);
	let disposed = false;
	onDestroy(() => {
		disposed = true;
	});
	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (pending) return;
		error = '';
		if (!title.trim() || !steps.length || steps.some((step) => !step.title.trim())) {
			error = 'ระบุชื่อแม่แบบและชื่อทุกขั้นตอน';
			return;
		}
		pending = true;
		try {
			const saved = await saveProcessTemplate(
				{
					requestId: crypto.randomUUID(),
					expectedRevision: template?.version,
					code,
					title: title.trim(),
					steps,
					active
				},
				template?.id
			);
			if (!disposed) onsaved(saved);
		} catch (cause) {
			if (!disposed) error = cause instanceof Error ? cause.message : 'บันทึกแม่แบบไม่สำเร็จ';
		} finally {
			if (!disposed) pending = false;
		}
	}
	function move(index: number, delta: number) {
		const copy = [...steps];
		[copy[index], copy[index + delta]] = [copy[index + delta], copy[index]];
		steps = copy;
	}
</script>

<form class="space-y-5" onsubmit={submit} data-testid="process-template-editor">
	{#if error}<PageState
			variant="error"
			title={error}
			description="เก็บการแก้ไขไว้แล้ว ตรวจสอบรุ่นแม่แบบล่าสุดก่อนลองอีกครั้ง"
		/>{/if}
	<div class="space-y-2">
		<Label for="template-title">ชื่อแม่แบบ *</Label><Input
			id="template-title"
			bind:value={title}
			required
			disabled={pending}
		/>
	</div>
	<p class="text-sm text-muted-foreground">
		{template ? `แก้ไขจากรุ่น ${template.version} · ` : ''}การบันทึกจะสร้างรุ่นใหม่
		หนังสือที่ส่งแล้วใช้ลำดับเดิม
	</p>
	{#each steps as step, index (index)}<section class="space-y-4 rounded-xl border bg-card p-4">
			<div class="flex flex-wrap items-center justify-between gap-2">
				<h3 class="font-medium">ขั้นตอน {index + 1}</h3>
				<div class="flex gap-1">
					<Button
						type="button"
						variant="ghost"
						size="sm"
						disabled={pending || index === 0}
						onclick={() => move(index, -1)}
						aria-label={`เลื่อนขั้นตอน ${index + 1} ขึ้น`}>ขึ้น</Button
					><Button
						type="button"
						variant="ghost"
						size="sm"
						disabled={pending || index === steps.length - 1}
						onclick={() => move(index, 1)}
						aria-label={`เลื่อนขั้นตอน ${index + 1} ลง`}>ลง</Button
					><Button
						type="button"
						variant="ghost"
						size="sm"
						disabled={pending || steps.length === 1}
						onclick={() => {
							steps = steps.filter((_, current) => current !== index);
						}}
						aria-label={`ลบขั้นตอน ${index + 1}`}>ลบ</Button
					>
				</div>
			</div>
			<div class="space-y-2">
				<Label for={`step-title-${index}`}>ชื่อขั้นตอน *</Label><Input
					id={`step-title-${index}`}
					bind:value={step.title}
					required
					disabled={pending}
				/>
			</div>
			<div class="grid gap-3 sm:grid-cols-3">
				<Choice
					id={`step-kind-${index}`}
					label="การดำเนินการ"
					bind:value={step.kind}
					options={Object.entries(stepLabels).map(([value, label]) => ({ value, label }))}
					disabled={pending}
				/><Choice
					id={`step-policy-${index}`}
					label="การผ่านขั้นตอน"
					bind:value={step.completionPolicy}
					options={[
						{ value: 'all', label: 'ผู้รับทุกคน' },
						{ value: 'any', label: 'คนใดคนหนึ่ง' }
					]}
					disabled={pending}
				/>
				<div class="space-y-2">
					<Label for={`step-days-${index}`}>กำหนดภายใน (วัน)</Label><Input
						id={`step-days-${index}`}
						type="number"
						min={1}
						max={365}
						value={step.dueAfterDays ?? undefined}
						disabled={pending}
						oninput={(event) => {
							step.dueAfterDays = event.currentTarget.value
								? Number(event.currentTarget.value)
								: undefined;
						}}
					/>
				</div>
			</div>
			<Recipients
				bind:recipients={step.recipients}
				{options}
				disabled={pending}
				prefix={`template-step-${index}`}
			/>
			<p class="text-xs text-muted-foreground">เว้นผู้รับไว้เพื่อให้ผู้ส่งเลือกตอนส่งหนังสือ</p>
			<div class="grid gap-3 sm:grid-cols-2">
				{#each [{ key: 'requiresSignature', label: 'ต้องลงลายมือชื่อ' }, { key: 'requiresComment', label: 'ต้องมีความคิดเห็น' }, { key: 'requiresEvidence', label: 'ต้องแนบหลักฐาน' }, { key: 'requiresDelegatedAuthority', label: 'ต้องมีอำนาจดำเนินการตามสิทธิ์' }] as field (field.key)}<div
						class="flex items-center gap-2"
					>
						<Checkbox
							id={`step-${index}-${field.key}`}
							checked={step[
								field.key as
									| 'requiresSignature'
									| 'requiresComment'
									| 'requiresEvidence'
									| 'requiresDelegatedAuthority'
							]}
							onCheckedChange={(value) => {
								step[
									field.key as
										| 'requiresSignature'
										| 'requiresComment'
										| 'requiresEvidence'
										| 'requiresDelegatedAuthority'
								] = value === true;
							}}
							disabled={pending}
						/><Label for={`step-${index}-${field.key}`}>{field.label}</Label>
					</div>{/each}
			</div>
		</section>{/each}
	<Button
		type="button"
		variant="outline"
		disabled={pending}
		onclick={() => {
			steps = [...steps, emptyStep()];
		}}>เพิ่มขั้นตอน</Button
	>
	<div class="flex items-center gap-2">
		<Checkbox id="template-active" bind:checked={active} disabled={pending} /><Label
			for="template-active">เปิดให้ใช้แม่แบบนี้</Label
		>
	</div>
	<div class="flex justify-end gap-2">
		<Button type="button" variant="outline" onclick={oncancel} disabled={pending}>ยกเลิก</Button
		><LoadingButton type="submit" loading={pending}>บันทึกแม่แบบรุ่นใหม่</LoadingButton>
	</div>
</form>
