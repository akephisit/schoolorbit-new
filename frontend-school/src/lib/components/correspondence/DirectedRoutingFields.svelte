<script lang="ts">
	import { untrack } from 'svelte';
	import type {
		CorrespondenceOptions,
		DirectedRoutingRequest,
		ProcessRecipient,
		CorrespondenceDetail
	} from '#lib/api/correspondence.js';
	import {
		recipientLabel,
		recipientKey,
		mergeRecipientLookups
	} from '#lib/correspondence/recipients.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as ToggleGroup from '#lib/components/ui/toggle-group/index.js';
	import DatePicker from '#lib/components/ui/date-picker/DatePicker.svelte';
	import Recipients from './Recipients.svelte';
	import Choice from './Choice.svelte';
	let {
		options,
		routing = $bindable(),
		register,
		inheritedReviewerNames = [],
		disabled = false
	}: {
		options: CorrespondenceOptions;
		routing: DirectedRoutingRequest;
		register: CorrespondenceDetail['record']['register'];
		inheritedReviewerNames?: string[];
		disabled?: boolean;
	} = $props();
	const defaults = untrack(() =>
		options.routingSettings.find((item) => item.register === register)
	);
	let known = $state(untrack(() => options));
	let purpose = $state(untrack(() => defaults?.defaultPurpose ?? 'execute'));
	let required = $state(true);
	let allowOnwardAssignment = $state(
		untrack(() => defaults?.defaultAllowOnwardAssignment ?? false)
	);
	function add(targets: ProcessRecipient[]) {
		const keys = new Set(routing.recipients.map((item) => recipientKey(item.recipient)));
		const additions = targets
			.filter((item) => !keys.has(recipientKey(item)))
			.map((recipient) => ({
				recipient,
				purpose: required ? purpose : ('acknowledge' as const),
				required,
				allowOnwardAssignment: required && allowOnwardAssignment
			}));
		routing = { ...routing, recipients: [...routing.recipients, ...additions] };
	}
</script>

<section class="space-y-4" aria-label="ผู้รับคำสั่ง">
	<h4 class="font-medium">เลือกผู้รับและงานที่มอบหมาย</h4>
	<div class="flex items-start gap-2">
		<Checkbox id="directed-required" bind:checked={required} {disabled} /><Label
			for="directed-required">ผู้รับหลัก ต้องยืนยันก่อนจบเรื่อง</Label
		>
	</div>
	{#if required}
		<ToggleGroup.Root
			type="single"
			value={purpose}
			onValueChange={(value) => {
				if (value === 'acknowledge' || value === 'execute') purpose = value;
			}}
			{disabled}
			aria-label="วัตถุประสงค์การมอบหมาย"
			class="justify-start flex-wrap"
		>
			<ToggleGroup.Item value="acknowledge">เพื่อทราบ</ToggleGroup.Item><ToggleGroup.Item
				value="execute">ให้ดำเนินการ</ToggleGroup.Item
			>
		</ToggleGroup.Root>
		<div class="flex items-start gap-2">
			<Checkbox id="directed-onward" bind:checked={allowOnwardAssignment} {disabled} /><Label
				for="directed-onward">อนุญาตให้มอบหมายต่อ</Label
			>
		</div>
	{:else}<p class="text-sm text-muted-foreground">
			สำเนาเพื่อทราบ รับทราบได้อย่างเดียว สั่งการต่อไม่ได้ และไม่ทำให้เรื่องรอ
		</p>{/if}
	<Recipients
		onlookups={(value) => {
			known = mergeRecipientLookups(known, value);
		}}
		onadd={add}
		options={known}
		directed
		{disabled}
		prefix="directed-target"
	/>
	<ol class="space-y-3" aria-label="รายการมอบหมาย">
		{#each routing.recipients as item, index (index)}
			<li class="space-y-2 rounded-lg border p-3 text-sm">
				<div class="flex items-start justify-between gap-2">
					<p class="min-w-0 break-words font-medium">{recipientLabel(item.recipient, known)}</p>
					<Button
						type="button"
						variant="ghost"
						size="sm"
						{disabled}
						aria-label={`นำรายการมอบหมาย ${index + 1} ออก`}
						onclick={() => {
							routing = {
								...routing,
								recipients: routing.recipients.filter((_, current) => current !== index)
							};
						}}>นำออก</Button
					>
				</div>
				<p>
					{item.required
						? item.purpose === 'execute'
							? 'ให้ดำเนินการ'
							: 'เพื่อทราบ'
						: 'สำเนาเพื่อทราบ'} · {item.allowOnwardAssignment
						? 'มอบหมายต่อได้'
						: 'มอบหมายต่อไม่ได้'}
				</p>
				<Label for={`directed-due-${index}`}>กำหนดเสร็จรายการ {index + 1} (ถ้ามี)</Label>
				<DatePicker
					id={`directed-due-${index}`}
					value={item.dueAt?.slice(0, 10)}
					{disabled}
					clearable
					onValueChange={(value) => {
						routing = {
							...routing,
							recipients: routing.recipients.map((row, current) =>
								current === index
									? { ...row, dueAt: value ? `${value}T23:59:59+07:00` : undefined }
									: row
							)
						};
					}}
				/>
			</li>
		{/each}
	</ol>
	<div class="space-y-3 border-t pt-4">
		{#if inheritedReviewerNames.length}<p
				class="rounded-lg border border-warning/30 bg-warning/5 p-3 text-sm"
			>
				เงื่อนไขตรวจทานที่ได้รับมอบหมายยังคงใช้: {inheritedReviewerNames.join(', ')} การส่งต่อครั้งนี้ไม่ยกเลิกการตรวจทานเดิม
			</p>{/if}
		<div class="flex items-start gap-2">
			<Checkbox
				id="directed-review"
				checked={routing.requireReview}
				{disabled}
				onCheckedChange={(value) => {
					routing = {
						...routing,
						requireReview: value === true,
						reviewer: value === true ? routing.reviewer : null
					};
				}}
			/><Label for="directed-review">ให้ผู้ตรวจทานตรวจผลหลังผู้รับที่จำเป็นดำเนินการครบ</Label>
		</div>
		{#if routing.requireReview}<Choice
				id="directed-reviewer"
				label="ผู้ตรวจทาน *"
				value={routing.reviewer?.userId ?? ''}
				onValueChange={(value) => {
					routing = { ...routing, reviewer: value ? { kind: 'user', userId: value } : null };
				}}
				searchable
				{disabled}
				options={known.reviewers.map((user) => ({ value: user.id, label: user.displayName }))}
			/>
		{:else}<p class="text-sm text-muted-foreground">
				เมื่อผู้รับที่จำเป็นรับทราบหรือดำเนินการครบ
				เรื่องจะจบตามเงื่อนไขเดิมโดยไม่เพิ่มการส่งกลับตรวจทาน
			</p>{/if}
	</div>
</section>
