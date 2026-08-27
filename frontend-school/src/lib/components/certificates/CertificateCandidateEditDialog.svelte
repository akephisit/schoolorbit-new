<script lang="ts">
	import type {
		CertificateCandidateDetail,
		CertificateTemplateDetail,
		RecipientType,
		UpdateCertificateCandidateRequest
	} from '$lib/api/certificates';
	import { LoadingButton } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Input } from '$lib/components/ui/input';
	import * as Select from '$lib/components/ui/select';
	import { Save } from 'lucide-svelte';
	import { untrack } from 'svelte';

	const NO_TEMPLATE_VALUE = '__no_template__';

	let {
		open,
		candidate,
		templates,
		busy = false,
		onopenchange,
		onsave
	}: {
		open: boolean;
		candidate: CertificateCandidateDetail;
		templates: CertificateTemplateDetail[];
		busy?: boolean;
		onopenchange: (open: boolean) => void;
		onsave: (payload: UpdateCertificateCandidateRequest) => Promise<void>;
	} = $props();

	type NameSource = NonNullable<CertificateCandidateDetail['selectedNameSource']>;
	type CustomField = { key: string; value: string };

	let recipientType = $state<RecipientType>(untrack(() => candidate.recipientType));
	let studentId = $state(untrack(() => candidate.studentId ?? ''));
	let staffUsername = $state(untrack(() => candidate.staffUsername ?? ''));
	let importedTitle = $state(untrack(() => candidate.importedTitle ?? ''));
	let importedFirstName = $state(untrack(() => candidate.importedFirstName));
	let importedLastName = $state(untrack(() => candidate.importedLastName));
	let activityItem = $state(untrack(() => candidate.activityItem ?? ''));
	let awardOrRole = $state(untrack(() => candidate.awardOrRole ?? ''));
	let templateId = $state(untrack(() => candidate.templateId ?? ''));
	let selectedNameSource = $state<NameSource | null>(untrack(() => candidate.selectedNameSource));
	let customFields = $state<CustomField[]>(
		untrack(() => Object.entries(candidate.customValues).map(([key, value]) => ({ key, value })))
	);

	const accountWasFound = $derived(
		candidate.matchStatus === 'matched' ||
			candidate.matchStatus === 'name_mismatch' ||
			candidate.matchStatus === 'inactive' ||
			candidate.matchedUserId !== null
	);
	const canChangeRecipientType = $derived(
		candidate.validationCodes.includes('invalid_recipient_type')
	);
	const compatibleTemplates = $derived(
		templates.filter(
			(template) => template.isActive && template.allowedRecipientTypes.includes(recipientType)
		)
	);
	const valid = $derived(importedFirstName.trim().length > 0 && importedLastName.trim().length > 0);

	function changeRecipientType(value: string) {
		if (!canChangeRecipientType) return;
		if (value !== 'student' && value !== 'staff' && value !== 'external') return;
		recipientType = value;
		if (!compatibleTemplates.some((template) => template.id === templateId)) templateId = '';
	}

	async function submit() {
		if (!valid || busy) return;
		await onsave({
			expectedUpdatedAt: candidate.updatedAt,
			recipientType,
			studentId: recipientType === 'student' ? studentId.trim() || null : null,
			staffUsername: recipientType === 'staff' ? staffUsername.trim() || null : null,
			importedTitle: importedTitle.trim() || null,
			importedFirstName: importedFirstName.trim(),
			importedLastName: importedLastName.trim(),
			activityItem: activityItem.trim() || null,
			awardOrRole: awardOrRole.trim() || null,
			templateId: templateId || null,
			selectedNameSource,
			customValues: Object.fromEntries(
				customFields.map((field) => [field.key, field.value.trim()] as const)
			)
		});
	}
</script>

<Dialog.Root {open} onOpenChange={onopenchange}>
	<Dialog.Content class="max-h-[92vh] overflow-y-auto sm:max-w-3xl">
		<Dialog.Header>
			<Dialog.Title>แก้ไขรายชื่อ</Dialog.Title>
			<Dialog.Description>
				ปรับข้อมูลจากไฟล์ เลือกชื่อที่จะใช้ และกำหนดแบบที่รองรับประเภทผู้รับรายการนี้
			</Dialog.Description>
		</Dialog.Header>

		<form
			class="space-y-5"
			onsubmit={(event) => {
				event.preventDefault();
				void submit();
			}}
		>
			<div class="grid gap-4 sm:grid-cols-3">
				<label class="space-y-1.5">
					<span class="text-sm font-medium">ประเภทผู้รับ</span>
					<Select.Root
						type="single"
						value={recipientType}
						disabled={!canChangeRecipientType}
						onValueChange={changeRecipientType}
					>
						<Select.Trigger class="w-full">
							{recipientType === 'student'
								? 'นักเรียน'
								: recipientType === 'staff'
									? 'บุคลากร'
									: 'บุคคลภายนอก'}
						</Select.Trigger>
						<Select.Content>
							<Select.Item value="student">นักเรียน</Select.Item>
							<Select.Item value="staff">บุคลากร</Select.Item>
							<Select.Item value="external" disabled={accountWasFound}>บุคคลภายนอก</Select.Item>
						</Select.Content>
					</Select.Root>
					{#if !canChangeRecipientType}
						<p class="text-xs text-muted-foreground">
							ประเภทผู้รับที่ตรวจสอบแล้วแก้ผ่านคำสั่งทั่วไปไม่ได้
						</p>
					{/if}
				</label>
				{#if recipientType === 'student'}
					<label class="space-y-1.5 sm:col-span-2">
						<span class="text-sm font-medium">รหัสนักเรียน</span>
						<Input bind:value={studentId} maxlength={100} />
					</label>
				{:else if recipientType === 'staff'}
					<label class="space-y-1.5 sm:col-span-2">
						<span class="text-sm font-medium">ชื่อผู้ใช้บุคลากร</span>
						<Input bind:value={staffUsername} maxlength={100} />
					</label>
				{:else}
					<div class="flex items-end text-xs text-muted-foreground sm:col-span-2">
						บุคคลภายนอกไม่ต้องมีรหัสบัญชี และจะไม่แสดงใบในพื้นที่นักเรียนหรือบุคลากร
					</div>
				{/if}
			</div>

			{#if accountWasFound}
				<div class="rounded-lg border border-blue-200 bg-blue-50 px-4 py-3 text-sm text-blue-900">
					บัญชีที่พบแล้วไม่สามารถเปลี่ยนเป็นบุคคลภายนอกได้
				</div>
			{/if}

			<div class="grid gap-4 sm:grid-cols-[140px_1fr_1fr]">
				<label class="space-y-1.5">
					<span class="text-sm font-medium">คำนำหน้า</span>
					<Input bind:value={importedTitle} maxlength={80} />
				</label>
				<label class="space-y-1.5">
					<span class="text-sm font-medium">ชื่อ <span class="text-destructive">*</span></span>
					<Input bind:value={importedFirstName} maxlength={150} required />
				</label>
				<label class="space-y-1.5">
					<span class="text-sm font-medium">นามสกุล <span class="text-destructive">*</span></span>
					<Input bind:value={importedLastName} maxlength={150} required />
				</label>
			</div>

			{#if candidate.matchStatus === 'name_mismatch'}
				<fieldset class="space-y-3 rounded-lg border border-amber-200 bg-amber-50 p-4">
					<legend class="px-1 text-sm font-semibold text-amber-900">ชื่อในไฟล์ไม่ตรงกับบัญชี</legend
					>
					<p class="text-sm text-amber-900">
						ชื่อบัญชี: {candidate.accountTitle ?? ''}{candidate.accountFirstName ?? ''}
						{candidate.accountLastName ?? ''}
					</p>
					<div class="flex flex-wrap gap-2">
						<Button
							variant={selectedNameSource === 'account' ? 'default' : 'outline'}
							size="sm"
							onclick={() => (selectedNameSource = 'account')}
						>
							ใช้ชื่อจากบัญชี
						</Button>
						<Button
							variant={selectedNameSource === 'file' ? 'default' : 'outline'}
							size="sm"
							onclick={() => (selectedNameSource = 'file')}
						>
							ใช้ชื่อจากไฟล์
						</Button>
					</div>
				</fieldset>
			{/if}

			<div class="grid gap-4 sm:grid-cols-2">
				<label class="space-y-1.5">
					<span class="text-sm font-medium">รายการกิจกรรม</span>
					<Input bind:value={activityItem} maxlength={300} />
				</label>
				<label class="space-y-1.5">
					<span class="text-sm font-medium">รางวัลหรือบทบาท</span>
					<Input bind:value={awardOrRole} maxlength={300} />
				</label>
			</div>

			<label class="block space-y-1.5">
				<span class="text-sm font-medium">แบบเกียรติบัตร</span>
				<Select.Root
					type="single"
					value={templateId || NO_TEMPLATE_VALUE}
					onValueChange={(value) => (templateId = value === NO_TEMPLATE_VALUE ? '' : value)}
				>
					<Select.Trigger class="w-full">
						{@const template = compatibleTemplates.find((item) => item.id === templateId)}
						{template
							? `${template.name}${template.isReady ? '' : ' (ยังไม่พร้อม)'}`
							: 'ยังไม่กำหนด'}
					</Select.Trigger>
					<Select.Content>
						<Select.Item value={NO_TEMPLATE_VALUE}>ยังไม่กำหนด</Select.Item>
						{#each compatibleTemplates as template (template.id)}
							<Select.Item value={template.id}
								>{template.name}{template.isReady ? '' : ' (ยังไม่พร้อม)'}</Select.Item
							>
						{/each}
					</Select.Content>
				</Select.Root>
			</label>

			{#if customFields.length > 0}
				<section class="space-y-3 rounded-lg border p-4">
					<div>
						<h3 class="text-sm font-semibold">คอลัมน์เพิ่มเติมจากไฟล์</h3>
						<p class="text-xs text-muted-foreground">ชื่อคอลัมน์เหล่านี้ใช้เป็นตัวแปรในแม่แบบ</p>
					</div>
					<div class="grid gap-3 sm:grid-cols-2">
						{#each customFields as field (field.key)}
							<label class="space-y-1.5">
								<span class="text-sm font-medium">{field.key}</span>
								<Input bind:value={field.value} maxlength={500} />
							</label>
						{/each}
					</div>
				</section>
			{/if}

			<Dialog.Footer>
				<Button variant="outline" disabled={busy} onclick={() => onopenchange(false)}>ยกเลิก</Button
				>
				<LoadingButton type="submit" loading={busy} disabled={!valid}>
					<Save class="size-4" /> บันทึกรายชื่อ
				</LoadingButton>
			</Dialog.Footer>
		</form>
	</Dialog.Content>
</Dialog.Root>
