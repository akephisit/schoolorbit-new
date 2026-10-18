<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import {
		getCorrespondenceRoutingSettings,
		getCorrespondenceOptions,
		saveCorrespondenceRoutingSettings,
		type CorrespondenceRoutingSettings,
		type CorrespondenceOptions,
		type SaveCorrespondenceRoutingSettingsRequest
	} from '#lib/api/correspondence.js';
	import type { RouteLoadResult } from '#lib/navigation/route-load.js';
	import { CorrespondenceRegion } from '#lib/correspondence/region.svelte.js';
	import { MutationIntent } from '#lib/correspondence/mutation-intent.js';
	import { registerLabels } from '#lib/correspondence/presentation.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageSkeleton, PageState, LoadingButton } from '#lib/components/app-state/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import * as ToggleGroup from '#lib/components/ui/toggle-group/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import Choice from './Choice.svelte';
	import Recipients from './Recipients.svelte';
	import UnsavedGuard from './UnsavedGuard.svelte';
	import RegisterNumberingFields from './RegisterNumberingFields.svelte';
	import TextStampTemplates from './TextStampTemplates.svelte';
	import { numberingError, stampTemplateErrors } from '#lib/correspondence/text-stamps.js';
	import {
		focusCorrespondenceField,
		type CorrespondenceFieldErrors
	} from '#lib/correspondence/validation.js';
	let fieldErrors = $state<CorrespondenceFieldErrors>({});
	let {
		source,
		optionsSource
	}: {
		source: Promise<RouteLoadResult<CorrespondenceRoutingSettings[]>>;
		optionsSource: Promise<RouteLoadResult<CorrespondenceOptions | null>>;
	} = $props();
	const settingsRegion = new CorrespondenceRegion<CorrespondenceRoutingSettings[]>();
	const optionsRegion = new CorrespondenceRegion<CorrespondenceOptions | null>();
	let drafts = $state<CorrespondenceRoutingSettings[]>([]),
		initialized = false;
	let register = $state<CorrespondenceRoutingSettings['register']>('incoming');
	let pending = $state(false),
		uncertain = $state(false),
		error = $state(''),
		message = $state('');
	let disposed = false;
	const intent = new MutationIntent<SaveCorrespondenceRoutingSettingsRequest>();
	$effect.pre(() => {
		const value = source;
		untrack(() => settingsRegion.consume(value));
	});
	$effect.pre(() => {
		const value = optionsSource;
		untrack(() => optionsRegion.consume(value));
	});
	$effect.pre(() => {
		const rows = settingsRegion.value;
		if (rows && !initialized)
			untrack(() => {
				initialized = true;
				drafts = structuredClone($state.snapshot(rows));
			});
	});
	onDestroy(() => {
		disposed = true;
		settingsRegion.dispose();
		optionsRegion.dispose();
	});
	const row = $derived(drafts.find((item) => item.register === register));
	const options = $derived(optionsRegion.value);
	const dirty = $derived(
		initialized && JSON.stringify(drafts) !== JSON.stringify(settingsRegion.value)
	);
	function update(changes: Partial<CorrespondenceRoutingSettings>) {
		drafts = drafts.map((item) => (item.register === register ? { ...item, ...changes } : item));
		message = '';
	}
	async function save(event: SubmitEvent) {
		event.preventDefault();
		if (!row || pending) return;
		error = '';
		message = '';
		fieldErrors = {};
		const numbering = row.numbering ?? { format: '{number}/{year}', minimumDigits: 1 };
		const invalidNumbering = numberingError(numbering);
		if (invalidNumbering)
			fieldErrors[
				Number.isInteger(numbering.minimumDigits) &&
				numbering.minimumDigits >= 1 &&
				numbering.minimumDigits <= 8
					? 'numbering-format'
					: 'numbering-minimum-digits'
			] = invalidNumbering;
		for (const template of row.textStampTemplates ?? []) {
			for (const [field, message] of Object.entries(stampTemplateErrors(template)))
				fieldErrors[`stamp-template-${template.id}-${field}`] = message;
		}
		if (row.defaultRequireReview && !row.defaultReviewer)
			fieldErrors['routing-default-reviewer'] = 'เลือกผู้ตรวจทานเริ่มต้น';
		const first = Object.entries(fieldErrors)[0];
		if (first) {
			error = first[1] ?? 'ตรวจสอบข้อมูลที่จำเป็น';
			void focusCorrespondenceField(first[0]);
			return;
		}
		pending = true;
		try {
			const result = await saveCorrespondenceRoutingSettings(
				intent.begin({
					expectedRevision: row.revision,
					register: row.register,
					numbering: row.numbering,
					textStampTemplates: row.textStampTemplates,
					eligibleDecisionMakers: row.eligibleDecisionMakers,
					defaultFirstRecipient: row.defaultFirstRecipient,
					defaultPurpose: row.defaultPurpose,
					defaultRequireReview: row.defaultRequireReview,
					defaultReviewer: row.defaultRequireReview ? row.defaultReviewer : null,
					defaultAllowOnwardAssignment: row.defaultAllowOnwardAssignment
				})
			);
			intent.accepted();
			if (disposed) return;
			uncertain = false;
			settingsRegion.patch(
				(settingsRegion.value ?? []).map((item) =>
					item.register === result.register ? result : item
				)
			);
			drafts = drafts.map((item) => (item.register === result.register ? result : item));
			message = 'บันทึกการตั้งค่าแล้ว';
		} catch (cause) {
			intent.rejected(cause);
			if (!disposed) {
				uncertain = intent.unresolved;
				error = cause instanceof Error ? cause.message : 'บันทึกการตั้งค่าไม่สำเร็จ';
			}
		} finally {
			if (!disposed) pending = false;
		}
	}
</script>

<PageShell
	title="ตั้งค่าสารบรรณ"
	backHref="/staff/correspondence"
	description="รูปแบบเลขทะเบียน แม่แบบข้อความ และค่าเริ่มต้นแยกตามประเภทหนังสือ"
>
	<UnsavedGuard {dirty} {pending} />
	<section
		class="space-y-4"
		aria-busy={settingsRegion.loading || optionsRegion.loading}
		data-testid="correspondence-routing-settings"
	>
		{#if settingsRegion.error}<PageState
				variant="error"
				title={settingsRegion.error}
				actionLabel="ลองอีกครั้ง"
				onaction={() =>
					settingsRegion.retry((signal) => getCorrespondenceRoutingSettings({ signal }))}
			/>{/if}
		{#if optionsRegion.error}<PageState
				variant="error"
				title={optionsRegion.error}
				actionLabel="โหลดตัวเลือกอีกครั้ง"
				onaction={() => optionsRegion.retry((signal) => getCorrespondenceOptions('', { signal }))}
			/>{/if}
		{#if (settingsRegion.loading && !settingsRegion.value) || (optionsRegion.loading && !options)}<PageSkeleton
			/>{:else if row && options}
			<ToggleGroup.Root
				type="single"
				value={register}
				disabled={pending || uncertain}
				onValueChange={(value) => {
					if (value === 'incoming' || value === 'outgoing' || value === 'internal') {
						register = value;
						error = '';
						message = '';
					}
				}}
				aria-label="ประเภททะเบียนที่ตั้งค่า"
				class="justify-start flex-wrap"
			>
				{#each Object.entries(registerLabels) as [value, label] (value)}<ToggleGroup.Item {value}
						>{label}</ToggleGroup.Item
					>{/each}
			</ToggleGroup.Root>
			<form class="space-y-4" onsubmit={save}>
				{#if error}<PageState
						variant="error"
						title={error}
						description="ค่าที่แก้ไขยังอยู่ ตรวจสอบข้อมูลล่าสุดก่อนบันทึกอีกครั้ง"
						actionLabel="ตรวจสอบข้อมูลล่าสุด"
						onaction={() =>
							settingsRegion.retry((signal) => getCorrespondenceRoutingSettings({ signal }))}
					/>{/if}
				{#if message}<p role="status" class="text-sm text-success">{message}</p>{/if}
				{#if uncertain}<p class="text-sm text-warning">
						ยังยืนยันผลไม่ได้ กดบันทึกอีกครั้งเพื่อตรวจผลด้วยคำขอเดิม
					</p>{/if}
				{#if settingsRegion.value?.find((item) => item.register === register)?.revision !== row.revision}<div
						class="space-y-2 rounded-lg border border-warning/30 bg-warning/5 p-3 text-sm"
					>
						<p>การตั้งค่ามีฉบับใหม่ ตรวจสอบแล้วใช้ฉบับล่าสุดเป็นฐานเพื่อเก็บค่าที่คุณแก้ไข</p>
						<LoadingButton
							type="button"
							variant="outline"
							disabled={pending || uncertain}
							onclick={() =>
								update({
									revision:
										settingsRegion.value?.find((item) => item.register === register)?.revision ??
										row.revision
								})}>ใช้ฉบับล่าสุดและเก็บค่าที่แก้ไข</LoadingButton
						>
					</div>{/if}
				<fieldset disabled={pending || uncertain} class="min-w-0 space-y-4">
					<Card.Root
						><Card.Content class="space-y-5"
							><RegisterNumberingFields
								value={row.numbering ?? { format: '{number}/{year}', minimumDigits: 1 }}
								onchange={(numbering) => update({ numbering })}
								errors={fieldErrors}
								disabled={pending || uncertain}
							/>
							<div class="border-t pt-5">
								<TextStampTemplates
									bind:templates={
										() => row.textStampTemplates ?? [],
										(textStampTemplates) => update({ textStampTemplates })
									}
									errors={fieldErrors}
									disabled={pending || uncertain}
								/>
							</div></Card.Content
						></Card.Root
					>
					<Card.Root
						><Card.Header
							><Card.Title>ผู้มีสิทธิพิจารณา {registerLabels[register]}</Card.Title></Card.Header
						><Card.Content class="space-y-4">
							<p class="text-sm text-muted-foreground">
								หากไม่จำกัดรายชื่อ
								บุคลากรที่ใช้งานอยู่และมีสิทธิดำเนินการหนังสือที่ได้รับมอบหมายจะเลือกได้ทั้งหมด
								การเพิ่มรายชื่อจำกัดการเลือกภายในกลุ่มนั้น
							</p>
							<Recipients
								onlookups={(value) => optionsRegion.patch(value)}
								{options}
								directed
								prefix="routing-eligible"
								disabled={pending || uncertain}
								bind:recipients={
									() => row.eligibleDecisionMakers,
									(value) => update({ eligibleDecisionMakers: value })
								}
							/>
							<Choice
								id="routing-first"
								label="ผู้พิจารณาสั่งการคนแรกที่แนะนำ"
								value={row.defaultFirstRecipient?.userId ?? '__none__'}
								onValueChange={(value) =>
									update({
										defaultFirstRecipient:
											value !== '__none__' ? { kind: 'user', userId: value } : null
									})}
								searchable
								options={[
									{ value: '__none__', label: 'ให้ผู้ลงทะเบียนเลือกทุกครั้ง' },
									...options.reviewers.map((user) => ({ value: user.id, label: user.displayName }))
								]}
								disabled={pending || uncertain}
							/>
						</Card.Content></Card.Root
					>
					<Card.Root
						><Card.Header><Card.Title>ค่าเริ่มต้นเมื่อมอบหมายต่อ</Card.Title></Card.Header
						><Card.Content class="space-y-4">
							<ToggleGroup.Root
								type="single"
								value={row.defaultPurpose}
								onValueChange={(value) => {
									if (value === 'acknowledge' || value === 'execute')
										update({ defaultPurpose: value });
								}}
								aria-label="วัตถุประสงค์เริ่มต้น"
								class="justify-start"
								><ToggleGroup.Item value="acknowledge">เพื่อทราบ</ToggleGroup.Item><ToggleGroup.Item
									value="execute">ให้ดำเนินการ</ToggleGroup.Item
								></ToggleGroup.Root
							>
							<div class="flex items-start gap-2">
								<Checkbox
									id="routing-default-onward"
									checked={row.defaultAllowOnwardAssignment}
									onCheckedChange={(value) =>
										update({ defaultAllowOnwardAssignment: value === true })}
								/><Label for="routing-default-onward">ผู้รับมอบหมายต่อได้เป็นค่าเริ่มต้น</Label>
							</div>
							<div class="flex items-start gap-2">
								<Checkbox
									id="routing-default-review"
									checked={row.defaultRequireReview}
									onCheckedChange={(value) => update({ defaultRequireReview: value === true })}
								/><Label for="routing-default-review">เสนอให้ตรวจทานผลเป็นค่าเริ่มต้น</Label>
							</div>
							{#if row.defaultRequireReview}<Choice
									id="routing-default-reviewer"
									error={fieldErrors['routing-default-reviewer']}
									label="ผู้ตรวจทานเริ่มต้น *"
									value={row.defaultReviewer?.userId ?? ''}
									onValueChange={(value) =>
										update({ defaultReviewer: value ? { kind: 'user', userId: value } : null })}
									searchable
									options={options.reviewers.map((user) => ({
										value: user.id,
										label: user.displayName
									}))}
								/>{/if}
							<p class="text-sm text-muted-foreground">
								เป็นค่าที่เสนอให้ผู้สั่งการตรวจสอบและเปลี่ยนก่อนส่ง
								เงื่อนไขตรวจทานที่รับมาจากผู้มอบหมายก่อนหน้ายังคงใช้เสมอ
							</p>
						</Card.Content></Card.Root
					>
				</fieldset>
				<div class="flex justify-end">
					<LoadingButton type="submit" loading={pending}>บันทึกการตั้งค่า</LoadingButton>
				</div>
			</form>
		{/if}
	</section>
</PageShell>
