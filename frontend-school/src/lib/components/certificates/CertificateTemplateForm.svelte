<script lang="ts">
	import { untrack } from 'svelte';
	import {
		attachCertificateTemplateBackground,
		createCertificateTemplate,
		updateCertificateTemplate,
		type CertificateTemplateDetail
	} from '$lib/api/certificates';
	import { deleteFile, uploadCertificateTemplateFile, type FileMetadata } from '$lib/api/files';
	import { LoadingButton } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { AlertCircle, FileUp, Save, Trash2 } from 'lucide-svelte';

	type RecipientType = CertificateTemplateDetail['allowedRecipientTypes'][number];

	let {
		campaignId,
		template,
		onpatched,
		onpendingchange,
		oncompleted,
		oncancel
	}: {
		campaignId: string;
		template?: CertificateTemplateDetail;
		onpatched: (template: CertificateTemplateDetail) => void;
		onpendingchange: (pending: boolean) => void;
		oncompleted: () => void;
		oncancel: () => void;
	} = $props();

	const recipientOptions: Array<{ value: RecipientType; label: string; hint: string }> = [
		{ value: 'student', label: 'นักเรียน', hint: 'ทั้งนักเรียนในโรงเรียนและนักเรียนภายนอก' },
		{ value: 'staff', label: 'บุคลากร', hint: 'ครู บุคลากร หรือผู้ปฏิบัติงานของโรงเรียน' },
		{ value: 'external', label: 'บุคคลภายนอก', hint: 'วิทยากร กรรมการ หรือผู้รับจากภายนอก' }
	];

	let name = $state(untrack(() => template?.name ?? ''));
	let allowedRecipientTypes = $state<RecipientType[]>(
		untrack(() => [...(template?.allowedRecipientTypes ?? [])])
	);
	let backgroundFile = $state<File | null>(null);
	let workingTemplate = $state.raw<CertificateTemplateDetail | null>(
		untrack(() => template ?? null)
	);
	let unattachedFile = $state.raw<FileMetadata | null>(null);
	let attachError = $state<Error | null>(null);
	let validationError = $state('');
	let saving = $state(false);
	let cleaning = $state(false);
	let fileInputKey = $state(0);

	const isCreating = $derived(template === undefined);

	function toggleRecipient(type: RecipientType, checked: boolean) {
		allowedRecipientTypes = checked
			? Array.from(new Set([...allowedRecipientTypes, type]))
			: allowedRecipientTypes.filter((candidate) => candidate !== type);
	}

	function selectBackground(event: Event) {
		const input = event.currentTarget as HTMLInputElement;
		backgroundFile = input.files?.[0] ?? null;
		attachError = null;
	}

	function asError(error: unknown, fallback: string): Error {
		return error instanceof Error ? error : new Error(fallback);
	}

	function setUnattachedFile(file: FileMetadata | null) {
		unattachedFile = file;
		onpendingchange(file !== null);
	}

	async function attachInitialBackground(current: CertificateTemplateDetail) {
		if (!unattachedFile) return;
		try {
			const attached = await attachCertificateTemplateBackground(current.id, {
				fileId: unattachedFile.id,
				geometryAction: 'preserve',
				previewConfirmed: false
			});
			workingTemplate = attached;
			setUnattachedFile(null);
			backgroundFile = null;
			attachError = null;
			fileInputKey += 1;
			onpatched(attached);
			oncompleted();
		} catch (error) {
			attachError = asError(error, 'แนบ PDF พื้นหลังไม่สำเร็จ');
		}
	}

	async function retryAttach() {
		if (!workingTemplate || !unattachedFile || saving) return;
		saving = true;
		onpendingchange(true);
		await attachInitialBackground(workingTemplate);
		saving = false;
		if (!unattachedFile) onpendingchange(false);
	}

	async function deleteTemporaryUpload() {
		if (!workingTemplate || !unattachedFile || cleaning) return;
		cleaning = true;
		try {
			await deleteFile(unattachedFile.id, workingTemplate.id);
			setUnattachedFile(null);
			backgroundFile = null;
			attachError = null;
			fileInputKey += 1;
		} catch (error) {
			attachError = asError(error, 'ลบไฟล์ชั่วคราวไม่สำเร็จ');
		} finally {
			cleaning = false;
		}
	}

	async function handleSubmit(event: SubmitEvent) {
		event.preventDefault();
		if (saving || unattachedFile) return;
		validationError = '';
		attachError = null;
		const normalizedName = name.trim().replace(/\s+/g, ' ');
		if (!normalizedName) {
			validationError = 'กรุณาระบุชื่อแบบเกียรติบัตร';
			return;
		}
		if (allowedRecipientTypes.length === 0) {
			validationError = 'เลือกประเภทผู้รับอย่างน้อยหนึ่งประเภท';
			return;
		}
		if (isCreating && !workingTemplate && !backgroundFile) {
			validationError = 'กรุณาเลือก PDF พื้นหลังหนึ่งหน้า';
			return;
		}

		saving = true;
		onpendingchange(true);
		try {
			let current = workingTemplate;
			if (!current) {
				current = await createCertificateTemplate(campaignId, {
					name: normalizedName,
					allowedRecipientTypes
				});
				workingTemplate = current;
				onpatched(current);
			} else if (
				current.name !== normalizedName ||
				current.allowedRecipientTypes.join('|') !== allowedRecipientTypes.join('|')
			) {
				current = await updateCertificateTemplate(current.id, {
					expectedUpdatedAt: current.updatedAt,
					name: normalizedName,
					allowedRecipientTypes
				});
				workingTemplate = current;
				onpatched(current);
			}

			if (!isCreating) {
				onpendingchange(false);
				oncompleted();
				return;
			}
			if (!backgroundFile) {
				validationError = 'เลือก PDF พื้นหลังเพื่อทำแบบเกียรติบัตรให้พร้อมใช้งาน';
				return;
			}

			setUnattachedFile(
				await uploadCertificateTemplateFile(
					backgroundFile,
					'certificate_template_background',
					current.id
				)
			);
			await attachInitialBackground(current);
		} catch (error) {
			attachError = asError(error, 'บันทึกแบบเกียรติบัตรไม่สำเร็จ');
		} finally {
			saving = false;
			if (!unattachedFile) onpendingchange(false);
		}
	}

	function handleCancel() {
		if (unattachedFile) {
			validationError = 'ลบไฟล์ชั่วคราวหรือแนบให้สำเร็จก่อนปิดแบบฟอร์ม';
			return;
		}
		oncancel();
	}
</script>

<form class="space-y-6" onsubmit={handleSubmit}>
	{#if validationError || attachError}
		<div
			class="flex items-start gap-2 rounded-lg border border-destructive/30 bg-destructive/5 p-3 text-sm text-destructive"
		>
			<AlertCircle class="mt-0.5 size-4 shrink-0" />
			<div class="min-w-0 flex-1">
				<p>{attachError?.message ?? validationError}</p>
				{#if unattachedFile}
					<p class="mt-1 text-xs text-muted-foreground">
						ไฟล์ถูกอัปโหลดแล้วแต่ยังไม่ถูกใช้เป็นพื้นหลัง คุณลองแนบซ้ำหรือลบไฟล์นี้ได้
					</p>
					<div class="mt-3 flex flex-wrap gap-2">
						<Button
							type="button"
							size="sm"
							variant="outline"
							onclick={retryAttach}
							disabled={saving}
						>
							ลองแนบอีกครั้ง
						</Button>
						<LoadingButton
							type="button"
							size="sm"
							variant="outline"
							loading={cleaning}
							onclick={deleteTemporaryUpload}
						>
							<Trash2 class="size-4" /> ลบไฟล์ชั่วคราว
						</LoadingButton>
					</div>
				{/if}
			</div>
		</div>
	{/if}

	<div class="space-y-2">
		<Label for="certificate-template-name">ชื่อแบบเกียรติบัตร</Label>
		<Input
			id="certificate-template-name"
			bind:value={name}
			maxlength={200}
			placeholder="เช่น รางวัลการแข่งขันคำคม"
			required
		/>
		<p class="text-xs text-muted-foreground">
			ใช้ชื่อที่แยกบทบาทหรือรางวัลของแบบนี้ได้ชัดเจน เช่น “วิทยากร” หรือ “รองชนะเลิศ”
		</p>
	</div>

	<fieldset class="space-y-3">
		<legend class="text-sm font-medium">ใช้กับผู้รับประเภทใด</legend>
		<div class="grid gap-2 sm:grid-cols-3">
			{#each recipientOptions as option (option.value)}
				<label
					class="flex cursor-pointer items-start gap-3 rounded-lg border bg-card p-3 transition-colors hover:bg-muted/40"
				>
					<Checkbox
						checked={allowedRecipientTypes.includes(option.value)}
						onCheckedChange={(checked) => toggleRecipient(option.value, checked === true)}
						class="mt-0.5"
					/>
					<span>
						<span class="block text-sm font-medium">{option.label}</span>
						<span class="mt-0.5 block text-xs leading-relaxed text-muted-foreground">
							{option.hint}
						</span>
					</span>
				</label>
			{/each}
		</div>
	</fieldset>

	{#if isCreating}
		<div class="space-y-2 rounded-xl border border-dashed bg-muted/20 p-4">
			<Label for="certificate-template-background">PDF พื้นหลังเริ่มต้น</Label>
			{#key fileInputKey}
				<Input
					id="certificate-template-background"
					type="file"
					accept=".pdf"
					onchange={selectBackground}
					disabled={saving || unattachedFile !== null}
					required={!workingTemplate}
				/>
			{/key}
			<p class="flex items-start gap-2 text-xs leading-relaxed text-muted-foreground">
				<FileUp class="mt-0.5 size-4 shrink-0" />
				<span>
					ระบบรับ PDF หนึ่งหน้า อ่านขนาดและแนวกระดาษจากไฟล์อัตโนมัติ โดยไม่ต้องกรอกขนาดเอง
				</span>
			</p>
		</div>
	{/if}

	<div class="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
		<Button type="button" variant="outline" onclick={handleCancel} disabled={saving || cleaning}>
			ยกเลิก
		</Button>
		<LoadingButton type="submit" loading={saving} disabled={unattachedFile !== null}>
			<Save class="size-4" />
			{isCreating ? 'สร้างและแนบพื้นหลัง' : 'บันทึกข้อมูลแบบ'}
		</LoadingButton>
	</div>
</form>
