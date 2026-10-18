<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import {
		getCorrespondenceOptions,
		type CorrespondenceOptions,
		type ProcessStepRecipients
	} from '#lib/api/correspondence.js';
	import { stepLabels } from '#lib/correspondence/presentation.js';
	import { PageState } from '#lib/components/app-state/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import DatePicker from '#lib/components/ui/date-picker/DatePicker.svelte';
	import { mergeRecipientLookups } from '#lib/correspondence/recipients.js';
	import Choice from './Choice.svelte';
	import * as ToggleGroup from '#lib/components/ui/toggle-group/index.js';
	import Recipients from './Recipients.svelte';
	let {
		mode = $bindable('directed'),
		firstUserId = $bindable(''),
		register,
		templateId = $bindable(''),
		steps = $bindable([]),
		dueDate = $bindable(),
		disabled = false,
		onready
	}: {
		mode?: string;
		firstUserId?: string;
		register?: CorrespondenceOptions['routingSettings'][number]['register'];
		templateId?: string;
		steps?: ProcessStepRecipients[];
		dueDate?: string;
		disabled?: boolean;
		onready?: (options: CorrespondenceOptions) => void;
	} = $props();
	let options = $state<CorrespondenceOptions | null>(null),
		loading = $state(true),
		error = $state('');
	let disposed = false;
	const request = new LatestRequest();
	onDestroy(() => {
		disposed = true;
		request.abort();
	});
	async function loadOptions(context = register) {
		const ticket = request.begin();
		loading = true;
		error = '';
		try {
			const result = await getCorrespondenceOptions('', { signal: ticket.signal }, context);
			if (!disposed && request.isCurrent(ticket.revision)) {
				options = result;
				if (!result.decisionMakers.some((user) => user.id === firstUserId)) {
					firstUserId = '';
					const suggested = result.routingSettings.find(
						(item) => item.register === context
					)?.defaultFirstRecipient;
					if (
						suggested?.kind === 'user' &&
						result.decisionMakers.some((user) => user.id === suggested.userId)
					)
						firstUserId = suggested.userId ?? '';
				}
				onready?.(result);
			}
		} catch (cause) {
			if (!disposed && request.isCurrent(ticket.revision))
				error = cause instanceof Error ? cause.message : 'โหลดตัวเลือกไม่สำเร็จ';
		} finally {
			if (!disposed && request.isCurrent(ticket.revision)) loading = false;
		}
	}
	$effect.pre(() => {
		const context = register;
		untrack(() => {
			options = null;
			void loadOptions(context);
		});
	});
	const template = $derived(options?.templates.find((item) => item.id === templateId));
	function selectTemplate(value: string) {
		templateId = value;
		const selected = options?.templates.find((item) => item.id === value);
		steps =
			selected?.steps.map((step, stepIndex) => ({
				stepIndex,
				recipients: (step.recipients ?? []).map((recipient) => ({ ...recipient }))
			})) ?? [];
	}
</script>

{#if error}<PageState
		variant="error"
		title={error}
		actionLabel="ลองอีกครั้ง"
		onaction={() => loadOptions()}
	/>{/if}
{#if loading && !options}<p role="status" class="text-sm text-muted-foreground">
		กำลังโหลดผู้พิจารณาและเส้นทาง…
	</p>{:else if options}
	<div class="space-y-3">
		<span class="text-sm font-medium">วิธีส่งหนังสือ</span>
		<ToggleGroup.Root
			type="single"
			value={mode}
			{disabled}
			aria-label="วิธีส่งหนังสือ"
			onValueChange={(value) => {
				if (value) mode = value;
			}}
			class="flex flex-wrap justify-start"
		>
			<ToggleGroup.Item value="directed">เลือกผู้พิจารณาสั่งการ</ToggleGroup.Item>
			<ToggleGroup.Item value="template">ใช้แม่แบบลำดับงาน</ToggleGroup.Item>
		</ToggleGroup.Root>
	</div>
	{#if mode === 'directed'}
		<Choice
			id="send-first-decisionmaker"
			label="ผู้พิจารณาสั่งการคนแรก *"
			bind:value={firstUserId}
			searchable
			{disabled}
			options={options.decisionMakers.map((user) => ({ value: user.id, label: user.displayName }))}
		/>
		<p class="text-sm text-muted-foreground">
			เลือกบุคลากรผู้รับผิดชอบจริงหนึ่งคน บุคคลนี้จะพิจารณาแล้วเลือกผู้รับและวิธีดำเนินการต่อ
		</p>
		{#if !options.decisionMakers.length}<p class="text-sm text-warning">
				ยังไม่มีผู้พิจารณาที่เลือกได้ กรุณาติดต่อผู้ดูแลการตั้งค่าสารบรรณ
			</p>{/if}
	{:else}
		<Choice
			id="send-template"
			label="ลำดับการดำเนินงาน"
			value={templateId}
			onValueChange={selectTemplate}
			{disabled}
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
							{disabled}
							prefix={`send-step-${index}`}
							onlookups={(value) => {
								if (options) {
									options = mergeRecipientLookups(options, value);
									onready?.(options);
								}
							}}
						/>{/if}
					<div class="space-y-2">
						<Label for={`step-due-${index}`}>กำหนดเสร็จขั้นตอนนี้ (ถ้ามี)</Label><DatePicker
							id={`step-due-${index}`}
							value={steps[index]?.dueAt?.slice(0, 10)}
							{disabled}
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
		{/if}
	{/if}
	<div class="space-y-2">
		<Label for="send-due">กำหนดเสร็จทั้งเรื่อง</Label><DatePicker
			id="send-due"
			bind:value={dueDate}
			{disabled}
			clearable
		/>
	</div>
{/if}
