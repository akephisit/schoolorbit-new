<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import {
		getCorrespondenceOptions,
		sendCorrespondence,
		type CorrespondenceDetail,
		type CorrespondenceOptions,
		type ProcessStepRecipients
	} from '#lib/api/correspondence.js';
	import { stepLabels } from '#lib/correspondence/presentation.js';
	import { LoadingButton, PageState } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import DatePicker from '#lib/components/ui/date-picker/DatePicker.svelte';
	import Choice from './Choice.svelte';
	import Recipients from './Recipients.svelte';
	let {
		detail,
		oncomplete,
		oncancel
	}: {
		detail: CorrespondenceDetail;
		oncomplete: (detail: CorrespondenceDetail) => void;
		oncancel: () => void;
	} = $props();
	let options = $state<CorrespondenceOptions | null>(null),
		loading = $state(true),
		error = $state(''),
		pending = $state(false),
		templateId = $state(''),
		steps = $state<ProcessStepRecipients[]>([]),
		dueDate = $state<string | undefined>();
	let expectedRevision = $state(untrack(() => detail.record.revision));
	let disposed = false;
	const controller = new AbortController();
	onDestroy(() => {
		disposed = true;
		controller.abort();
	});
	async function loadOptions() {
		loading = true;
		error = '';
		try {
			const result = await getCorrespondenceOptions('', { signal: controller.signal });
			if (!disposed) options = result;
		} catch (cause) {
			if (!disposed) error = cause instanceof Error ? cause.message : 'โหลดตัวเลือกไม่สำเร็จ';
		} finally {
			if (!disposed) loading = false;
		}
	}
	$effect(() => {
		void loadOptions();
	});
	const template = $derived(options?.templates.find((item) => item.id === templateId));
	function selectTemplate() {
		if (!template) return;
		steps = template.steps.map((step, stepIndex) => ({
			stepIndex,
			recipients: (step.recipients ?? []).map((recipient) => ({ ...recipient }))
		}));
	}
	$effect.pre(() => {
		const selected = templateId;
		if (selected) selectTemplate();
	});
	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (pending || !template) return;
		error = '';
		if (steps.some((step) => !step.recipients.length)) {
			error = 'เลือกผู้รับให้ครบทุกขั้นตอน';
			return;
		}
		pending = true;
		try {
			const result = await sendCorrespondence(detail.record.id, {
				requestId: crypto.randomUUID(),
				expectedRevision,
				templateId,
				stepRecipients: steps,
				dueAt: dueDate ? `${dueDate}T23:59:59+07:00` : undefined
			});
			if (!disposed) oncomplete(result);
		} catch (cause) {
			if (!disposed) error = cause instanceof Error ? cause.message : 'ส่งหนังสือไม่สำเร็จ';
		} finally {
			if (!disposed) pending = false;
		}
	}
</script>

<form class="space-y-4" onsubmit={submit} data-testid="correspondence-send">
	<h3 class="font-semibold">ส่งหนังสือดำเนินการ</h3>
	{#if expectedRevision !== detail.record.revision}<p class="text-sm text-warning">
			เอกสารเปลี่ยนแปลงแล้ว กรุณาตรวจสอบฉบับปัจจุบัน
		</p>
		<Button
			type="button"
			variant="outline"
			onclick={() => {
				expectedRevision = detail.record.revision;
			}}>ใช้ข้อมูลล่าสุดและเก็บผู้รับที่เลือก</Button
		>{/if}
	{#if error}<PageState
			variant="error"
			title={error}
			actionLabel={options ? undefined : 'ลองอีกครั้ง'}
			onaction={loadOptions}
		/>{/if}
	{#if loading}<p role="status" class="text-sm text-muted-foreground">
			กำลังโหลดแม่แบบและผู้รับ…
		</p>{:else if options}
		<Choice
			id="send-template"
			label="ลำดับการดำเนินงาน"
			bind:value={templateId}
			disabled={pending}
			options={options.templates
				.filter((item) => item.active)
				.map((item) => ({ value: item.id, label: item.title }))}
		/>
		{#if template}
			<p class="text-sm text-muted-foreground">
				แม่แบบรุ่น {template.version} · ผู้รับและขั้นตอนจะถูกบันทึกสำหรับหนังสือฉบับนี้
			</p>
			{#each template.steps as step, index (index)}<section class="space-y-3 rounded-xl border p-3">
					<h4 class="font-medium">{index + 1}. {step.title}</h4>
					<p class="text-sm text-muted-foreground">
						{stepLabels[step.kind]} · {step.completionPolicy === 'all'
							? 'ผู้รับทุกคนดำเนินการ'
							: 'ผู้รับคนใดคนหนึ่งดำเนินการ'}{step.dueAfterDays
							? ` · ภายใน ${step.dueAfterDays} วัน`
							: ''}
					</p>
					{#if steps[index]}<Recipients
							bind:recipients={steps[index].recipients}
							{options}
							disabled={pending}
							prefix={`send-step-${index}`}
						/>{/if}
					<div class="space-y-2">
						<Label for={`step-due-${index}`}>กำหนดเสร็จขั้นตอนนี้ (ถ้ามี)</Label><DatePicker
							id={`step-due-${index}`}
							disabled={pending}
							clearable
							onValueChange={(value) => {
								steps = steps.map((item, current) =>
									current === index
										? { ...item, dueAt: value ? `${value}T23:59:59+07:00` : undefined }
										: item
								);
							}}
						/>
					</div>
				</section>{/each}
			<div class="space-y-2">
				<Label for="send-due">กำหนดเสร็จทั้งเรื่อง</Label><DatePicker
					id="send-due"
					bind:value={dueDate}
					disabled={pending}
					clearable
				/>
			</div>
		{/if}
	{/if}
	<div class="flex justify-end gap-2">
		<Button type="button" variant="outline" onclick={oncancel} disabled={pending}>ยกเลิก</Button
		><LoadingButton type="submit" loading={pending} disabled={!template || loading}
			>ส่งหนังสือ</LoadingButton
		>
	</div>
</form>
