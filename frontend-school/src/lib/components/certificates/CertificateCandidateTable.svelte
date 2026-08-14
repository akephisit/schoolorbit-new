<script lang="ts">
	import type { CertificateCandidateDetail } from '$lib/api/certificates';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Table from '$lib/components/ui/table';
	import { Pencil, Trash2, UserCheck, UserRoundCheck } from 'lucide-svelte';

	let {
		candidates,
		selectedIds,
		externalConfirmationIssues = [],
		canManage = false,
		canSubmit = false,
		canDelete = false,
		onselectionchange,
		onedit,
		onchoosename,
		onconfirmexternal,
		onconfirmduplicate,
		ondelete
	}: {
		candidates: CertificateCandidateDetail[];
		selectedIds: string[];
		externalConfirmationIssues?: Array<{
			candidateId: string;
			code: 'account_state_changed';
			message: string;
		}>;
		canManage?: boolean;
		canSubmit?: boolean;
		canDelete?: boolean;
		onselectionchange: (ids: string[]) => void;
		onedit: (candidate: CertificateCandidateDetail) => void;
		onchoosename: (candidate: CertificateCandidateDetail, source: 'file' | 'account') => void;
		onconfirmexternal: (candidate: CertificateCandidateDetail) => void;
		onconfirmduplicate: (candidate: CertificateCandidateDetail) => void;
		ondelete: (candidate: CertificateCandidateDetail) => void;
	} = $props();

	const selectableIds = $derived(
		candidates
			.filter(
				(candidate) =>
					(canManage && candidate.capabilities.canUpdate) ||
					(canSubmit && candidate.validationStatus === 'ready')
			)
			.map((candidate) => candidate.id)
	);
	const allSelected = $derived(
		selectableIds.length > 0 && selectableIds.every((id) => selectedIds.includes(id))
	);
	const partlySelected = $derived(
		!allSelected && selectableIds.some((id) => selectedIds.includes(id))
	);
	const containsProtectedAccount = $derived(
		candidates.some(
			(candidate) =>
				candidate.matchedUserId !== null ||
				candidate.matchStatus === 'matched' ||
				candidate.matchStatus === 'inactive'
		)
	);

	const validationLabels: Record<CertificateCandidateDetail['validationStatus'], string> = {
		ready: 'พร้อมออก',
		needs_review: 'ต้องตรวจสอบ',
		invalid: 'ข้อมูลไม่ถูกต้อง'
	};

	const validationClasses: Record<CertificateCandidateDetail['validationStatus'], string> = {
		ready: 'border-emerald-200 bg-emerald-50 text-emerald-800',
		needs_review: 'border-amber-200 bg-amber-50 text-amber-800',
		invalid: 'border-red-200 bg-red-50 text-red-800'
	};

	const recipientLabels: Record<CertificateCandidateDetail['recipientType'], string> = {
		student: 'นักเรียน',
		staff: 'บุคลากร',
		external: 'บุคคลภายนอก'
	};

	const matchLabels: Record<CertificateCandidateDetail['matchStatus'], string> = {
		matched: 'พบบัญชีและชื่อตรง',
		name_mismatch: 'พบบัญชีแต่ชื่อต่างกัน',
		not_found: 'ไม่พบบัญชี',
		inactive: 'พบบัญชีที่ปิดใช้งาน',
		external_confirmed: 'ยืนยันเป็นบุคคลภายนอก',
		not_applicable: 'ไม่ต้องเชื่อมบัญชี'
	};

	const issueLabels: Record<CertificateCandidateDetail['validationCodes'][number], string> = {
		invalid_recipient_type: 'ประเภทผู้รับไม่ถูกต้อง',
		missing_student_id: 'ไม่มีรหัสนักเรียน',
		missing_staff_username: 'ไม่มีชื่อผู้ใช้บุคลากร',
		unexpected_internal_lookup: 'ข้อมูลเชื่อมบัญชีไม่สอดคล้อง',
		missing_first_name: 'ไม่มีชื่อ',
		missing_last_name: 'ไม่มีนามสกุล',
		name_too_long: 'ชื่อยาวเกินกำหนด',
		value_too_long: 'ข้อมูลยาวเกินกำหนด',
		forbidden_sensitive_value: 'พบข้อมูลอ่อนไหวที่ห้ามใช้',
		account_not_found: 'ไม่พบบัญชีในโรงเรียน',
		account_inactive: 'บัญชีปิดใช้งาน',
		name_source_required: 'ต้องเลือกชื่อจากบัญชีหรือไฟล์',
		template_required: 'ยังไม่กำหนดแบบ',
		template_not_found: 'ไม่พบแบบที่ระบุ',
		template_incompatible: 'แบบไม่รองรับประเภทผู้รับ',
		template_not_ready: 'แบบยังไม่พร้อมใช้งาน',
		duplicate_candidate: 'อาจเป็นรายชื่อซ้ำ'
	};

	function displayName(candidate: CertificateCandidateDetail): string {
		if (candidate.selectedNameSource === 'account' && candidate.accountFirstName) {
			return `${candidate.accountTitle ?? ''}${candidate.accountFirstName} ${candidate.accountLastName ?? ''}`.trim();
		}
		return `${candidate.importedTitle ?? ''}${candidate.importedFirstName} ${candidate.importedLastName}`.trim();
	}

	function accountReference(candidate: CertificateCandidateDetail): string {
		if (candidate.recipientType === 'student') return candidate.studentId ?? '-';
		if (candidate.recipientType === 'staff') return candidate.staffUsername ?? '-';
		return '-';
	}

	function toggleAll(checked: boolean) {
		if (checked) {
			onselectionchange(Array.from(new Set([...selectedIds, ...selectableIds])));
			return;
		}
		onselectionchange(selectedIds.filter((id) => !selectableIds.includes(id)));
	}

	function toggleOne(candidateId: string, checked: boolean) {
		if (checked) {
			onselectionchange(Array.from(new Set([...selectedIds, candidateId])));
			return;
		}
		onselectionchange(selectedIds.filter((id) => id !== candidateId));
	}
</script>

{#if containsProtectedAccount}
	<div
		class="mb-3 flex items-start gap-2 rounded-lg border border-blue-200 bg-blue-50 px-4 py-3 text-sm text-blue-900"
	>
		<UserRoundCheck class="mt-0.5 size-4 shrink-0" />
		<span>บัญชีที่พบแล้วไม่สามารถเปลี่ยนเป็นบุคคลภายนอกได้</span>
	</div>
{/if}

<div class="overflow-x-auto rounded-xl border bg-card shadow-sm">
	<Table.Root class="min-w-[1360px]">
		<Table.Header>
			<Table.Row class="bg-muted/40 hover:bg-muted/40">
				<Table.Head class="w-12 text-center">
					<input
						type="checkbox"
						class="size-4 rounded border-input accent-primary"
						aria-label="เลือกทุกรายการในตาราง"
						checked={allSelected}
						indeterminate={partlySelected}
						disabled={selectableIds.length === 0}
						onchange={(event) => toggleAll(event.currentTarget.checked)}
					/>
				</Table.Head>
				<Table.Head class="w-36">สถานะ</Table.Head>
				<Table.Head class="w-52">ผู้รับ</Table.Head>
				<Table.Head class="w-32">ประเภท / รหัส</Table.Head>
				<Table.Head class="w-48">การเชื่อมบัญชี</Table.Head>
				<Table.Head class="w-52">กิจกรรม / รางวัล</Table.Head>
				<Table.Head class="w-44">แบบเกียรติบัตร</Table.Head>
				<Table.Head class="w-64">รายการที่ต้องแก้</Table.Head>
				<Table.Head class="w-72 text-right">คำสั่ง</Table.Head>
			</Table.Row>
		</Table.Header>
		<Table.Body>
			{#each candidates as candidate (candidate.id)}
				{@const externalIssue = externalConfirmationIssues.find(
					(issue) => issue.candidateId === candidate.id
				)}
				<Table.Row class={selectedIds.includes(candidate.id) ? 'bg-primary/5' : undefined}>
					<Table.Cell class="text-center align-top">
						<input
							type="checkbox"
							class="mt-1 size-4 rounded border-input accent-primary"
							aria-label={`เลือกรายการ ${candidate.importedFirstName} ${candidate.importedLastName}`}
							checked={selectedIds.includes(candidate.id)}
							disabled={!(canManage && candidate.capabilities.canUpdate) &&
								!(canSubmit && candidate.validationStatus === 'ready')}
							onchange={(event) => toggleOne(candidate.id, event.currentTarget.checked)}
						/>
					</Table.Cell>
					<Table.Cell class="align-top">
						<Badge variant="outline" class={validationClasses[candidate.validationStatus]}>
							{validationLabels[candidate.validationStatus]}
						</Badge>
					</Table.Cell>
					<Table.Cell class="align-top">
						<p class="font-medium">{displayName(candidate)}</p>
						{#if candidate.selectedNameSource}
							<p class="mt-1 text-xs text-muted-foreground">
								ใช้ชื่อจาก{candidate.selectedNameSource === 'account' ? 'บัญชี' : 'ไฟล์'}
							</p>
						{/if}
					</Table.Cell>
					<Table.Cell class="align-top">
						<p>{recipientLabels[candidate.recipientType]}</p>
						<p class="mt-1 font-mono text-xs text-muted-foreground">
							{accountReference(candidate)}
						</p>
					</Table.Cell>
					<Table.Cell class="align-top text-sm">
						{matchLabels[candidate.matchStatus]}
					</Table.Cell>
					<Table.Cell class="align-top">
						<p>{candidate.activityItem ?? '-'}</p>
						{#if candidate.awardOrRole}
							<p class="mt-1 text-xs text-muted-foreground">{candidate.awardOrRole}</p>
						{/if}
					</Table.Cell>
					<Table.Cell class="align-top">
						{candidate.templateName ?? 'ยังไม่กำหนด'}
					</Table.Cell>
					<Table.Cell class="align-top">
						{#if externalIssue}
							<p
								class="mb-2 rounded-md border border-amber-200 bg-amber-50 px-2 py-1.5 text-xs text-amber-900"
							>
								{externalIssue.message}
							</p>
						{/if}
						{#if candidate.validationCodes.length === 0}
							<span class="text-emerald-700">ผ่านการตรวจสอบ</span>
						{:else}
							<ul class="space-y-1 text-xs text-muted-foreground">
								{#each candidate.validationCodes as code (code)}
									<li>• {issueLabels[code]}</li>
								{/each}
							</ul>
						{/if}
					</Table.Cell>
					<Table.Cell class="align-top">
						<div class="flex flex-wrap justify-end gap-1.5">
							{#if candidate.capabilities.canChooseName && candidate.matchStatus === 'name_mismatch'}
								<Button
									size="sm"
									variant="outline"
									onclick={() => onchoosename(candidate, 'account')}
								>
									ใช้ชื่อจากบัญชี
								</Button>
								<Button size="sm" variant="outline" onclick={() => onchoosename(candidate, 'file')}>
									ใช้ชื่อจากไฟล์
								</Button>
							{/if}
							{#if candidate.capabilities.canConfirmExternal && candidate.matchStatus !== 'matched' && candidate.matchStatus !== 'inactive' && candidate.matchedUserId === null}
								<Button
									size="sm"
									variant="outline"
									aria-label={`ยืนยัน ${candidate.importedFirstName} ${candidate.importedLastName} เป็นบุคคลภายนอก`}
									onclick={() => onconfirmexternal(candidate)}
								>
									<UserCheck class="size-4" /> ยืนยันภายนอก
								</Button>
							{/if}
							{#if candidate.capabilities.canConfirmDuplicate}
								<Button size="sm" variant="outline" onclick={() => onconfirmduplicate(candidate)}>
									ยืนยันรายชื่อซ้ำ
								</Button>
							{/if}
							{#if canManage && candidate.capabilities.canUpdate}
								<Button
									size="icon-sm"
									variant="ghost"
									aria-label={`แก้ไข ${displayName(candidate)}`}
									onclick={() => onedit(candidate)}
								>
									<Pencil class="size-4" />
								</Button>
							{/if}
							{#if canDelete && candidate.capabilities.canDelete}
								<Button
									size="icon-sm"
									variant="ghost"
									class="text-destructive"
									aria-label={`ลบ ${displayName(candidate)}`}
									onclick={() => ondelete(candidate)}
								>
									<Trash2 class="size-4" />
								</Button>
							{/if}
						</div>
					</Table.Cell>
				</Table.Row>
			{/each}
		</Table.Body>
	</Table.Root>
</div>

{#if candidates.length === 0}
	<div class="rounded-xl border border-dashed px-6 py-12 text-center">
		<p class="font-medium">ไม่พบรายชื่อในเงื่อนไขนี้</p>
		<p class="mt-1 text-sm text-muted-foreground">
			ลองเปลี่ยนตัวกรอง หรือเพิ่มรายชื่อผู้รับเข้าสู่กิจกรรม
		</p>
	</div>
{/if}
