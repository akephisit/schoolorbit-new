<script lang="ts">
	import type {
		CertificateTemplateDetail,
		CreateManualExternalCandidateRequest
	} from '$lib/api/certificates';
	import { LoadingButton } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Input } from '$lib/components/ui/input';
	import { UserPlus } from 'lucide-svelte';

	let {
		open,
		templates,
		busy = false,
		onopenchange,
		oncreate
	}: {
		open: boolean;
		templates: CertificateTemplateDetail[];
		busy?: boolean;
		onopenchange: (open: boolean) => void;
		oncreate: (payload: CreateManualExternalCandidateRequest) => Promise<void>;
	} = $props();

	let title = $state('');
	let firstName = $state('');
	let lastName = $state('');
	let activityItem = $state('');
	let awardOrRole = $state('');
	let templateId = $state('');

	const compatibleTemplates = $derived(
		templates.filter(
			(template) => template.isActive && template.allowedRecipientTypes.includes('external')
		)
	);
	const valid = $derived(firstName.trim().length > 0 && lastName.trim().length > 0);

	async function submit() {
		if (!valid || busy) return;
		await oncreate({
			title: title.trim() || null,
			firstName: firstName.trim(),
			lastName: lastName.trim(),
			activityItem: activityItem.trim() || null,
			awardOrRole: awardOrRole.trim() || null,
			templateId: templateId || null,
			customValues: {}
		});
	}
</script>

<Dialog.Root {open} onOpenChange={onopenchange}>
	<Dialog.Content class="max-h-[90vh] overflow-y-auto sm:max-w-2xl">
		<Dialog.Header>
			<Dialog.Title>เพิ่มบุคคลภายนอก</Dialog.Title>
			<Dialog.Description>
				ใช้สำหรับผู้รับที่ไม่มีบัญชีในโรงเรียน รวมถึงนักเรียนจากโรงเรียนอื่นที่เข้าร่วมการแข่งขัน
			</Dialog.Description>
		</Dialog.Header>

		<form
			class="space-y-4"
			onsubmit={(event) => {
				event.preventDefault();
				void submit();
			}}
		>
			<div class="grid gap-4 sm:grid-cols-[140px_1fr_1fr]">
				<label class="space-y-1.5">
					<span class="text-sm font-medium">คำนำหน้า</span>
					<Input bind:value={title} maxlength={80} placeholder="คุณ" />
				</label>
				<label class="space-y-1.5">
					<span class="text-sm font-medium">ชื่อ <span class="text-destructive">*</span></span>
					<Input bind:value={firstName} maxlength={150} required />
				</label>
				<label class="space-y-1.5">
					<span class="text-sm font-medium">นามสกุล <span class="text-destructive">*</span></span>
					<Input bind:value={lastName} maxlength={150} required />
				</label>
			</div>

			<div class="grid gap-4 sm:grid-cols-2">
				<label class="space-y-1.5">
					<span class="text-sm font-medium">รายการกิจกรรม</span>
					<Input bind:value={activityItem} maxlength={300} placeholder="เช่น การแข่งขันคำคม" />
				</label>
				<label class="space-y-1.5">
					<span class="text-sm font-medium">รางวัลหรือบทบาท</span>
					<Input
						bind:value={awardOrRole}
						maxlength={300}
						placeholder="เช่น รองชนะเลิศอันดับที่ 1"
					/>
				</label>
			</div>

			<label class="block space-y-1.5">
				<span class="text-sm font-medium">แบบเกียรติบัตร</span>
				<select
					bind:value={templateId}
					class="border-input bg-background h-9 w-full rounded-md border px-3 text-sm shadow-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
				>
					<option value="">ยังไม่กำหนด</option>
					{#each compatibleTemplates as template (template.id)}
						<option value={template.id}
							>{template.name}{template.isReady ? '' : ' (ยังไม่พร้อม)'}</option
						>
					{/each}
				</select>
				<p class="text-xs text-muted-foreground">
					แสดงเฉพาะแบบที่รองรับบุคคลภายนอก จึงใช้แบบรางวัลการแข่งขันได้เมื่อผู้ออกแบบอนุญาต
				</p>
			</label>

			<Dialog.Footer>
				<Button variant="outline" disabled={busy} onclick={() => onopenchange(false)}>ยกเลิก</Button
				>
				<LoadingButton type="submit" loading={busy} disabled={!valid}>
					<UserPlus class="size-4" /> เพิ่มผู้รับ
				</LoadingButton>
			</Dialog.Footer>
		</form>
	</Dialog.Content>
</Dialog.Root>
