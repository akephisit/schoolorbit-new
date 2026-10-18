<script lang="ts">
	import { onDestroy } from 'svelte';
	import { goto } from '$app/navigation';
	import {
		createCorrespondence,
		getCorrespondenceOptions,
		type CorrespondenceOptions,
		reviseCorrespondence,
		type CorrespondenceDetail,
		type CreateCorrespondenceRequest
	} from '#lib/api/correspondence.js';
	import { uploadFile } from '#lib/api/files.js';
	import { registerLabels } from '#lib/correspondence/presentation.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { LoadingButton, PageState } from '#lib/components/app-state/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import DatePicker from '#lib/components/ui/date-picker/DatePicker.svelte';
	import Choice from './Choice.svelte';
	let register = $state('incoming'),
		title = $state(''),
		sender = $state(''),
		documentNumber = $state(''),
		documentDate = $state<string | undefined>(),
		urgency = $state('normal'),
		content = $state('');
	let ownerOptions = $state<CorrespondenceOptions | null>(null),
		organizationUnitId = $state(''),
		ownerLoading = $state(false),
		ownerError = $state('');
	const ownerController = new AbortController();
	async function loadOwners() {
		ownerLoading = true;
		ownerError = '';
		try {
			const result = await getCorrespondenceOptions('', { signal: ownerController.signal });
			if (!disposed) ownerOptions = result;
		} catch (cause) {
			if (!disposed) ownerError = cause instanceof Error ? cause.message : 'โหลดหน่วยงานไม่สำเร็จ';
		} finally {
			if (!disposed) ownerLoading = false;
		}
	}
	let sourceLinks = $state<{ label: string; url: string }[]>([]);
	let original = $state<File | null>(null),
		attachments = $state<File[]>([]);
	let pending = $state(false),
		error = $state('');
	let created = $state<CorrespondenceDetail | null>(null);
	let originalFileId: string | undefined;
	const attachmentFileIds: string[] = [];
	const requestId = crypto.randomUUID();
	let disposed = false;
	onDestroy(() => {
		disposed = true;
		ownerController.abort();
	});
	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (pending) return;
		if (!title.trim() || !sender.trim()) {
			error = 'กรอกเรื่องและผู้ส่งให้ครบ';
			return;
		}
		if (
			sourceLinks.some((link) => {
				try {
					return !['http:', 'https:'].includes(new URL(link.url).protocol) || !link.label.trim();
				} catch {
					return true;
				}
			})
		) {
			error = 'ระบุชื่อและลิงก์แหล่งที่มาแบบ https:// หรือ http://';
			return;
		}
		pending = true;
		error = '';
		try {
			if (!created) {
				const payload: CreateCorrespondenceRequest = {
					requestId,
					register: register as CreateCorrespondenceRequest['register'],
					title: title.trim(),
					sender: sender.trim(),
					documentNumber: documentNumber || undefined,
					documentDate,
					urgency: urgency as CreateCorrespondenceRequest['urgency'],
					content,
					sourceLinks,
					organizationUnitId: organizationUnitId || undefined,
					annotations: [],
					attachmentFileIds: []
				};
				const result = await createCorrespondence(payload);
				if (disposed) return;
				created = result;
			}
			if (original && !originalFileId) {
				const file = await uploadFile(original, 'correspondence_document', created.record.id);
				if (disposed) return;
				originalFileId = file.id;
			}
			for (let index = attachmentFileIds.length; index < attachments.length; index += 1) {
				const file = await uploadFile(
					attachments[index],
					'correspondence_document',
					created.record.id
				);
				if (disposed) return;
				attachmentFileIds.push(file.id);
			}
			if (originalFileId || attachmentFileIds.length) {
				const result = await reviseCorrespondence(created.record.id, {
					requestId: crypto.randomUUID(),
					expectedRevision: created.record.revision,
					content,
					originalFileId,
					attachmentFileIds,
					annotations: []
				});
				if (disposed) return;
				created = result;
			}
			await goto(`/staff/correspondence/${created.record.id}`);
		} catch (cause) {
			if (!disposed) error = cause instanceof Error ? cause.message : 'สร้างหนังสือไม่สำเร็จ';
		} finally {
			if (!disposed) pending = false;
		}
	}
</script>

<PageShell
	title="สร้างหนังสือ"
	backHref="/staff/correspondence"
	description="ลงทะเบียนฉบับร่าง แล้วเลือกผู้รับเพื่อส่งดำเนินการ"
>
	<form class="max-w-4xl space-y-6" onsubmit={submit}>
		{#if error}<PageState
				variant="error"
				title={error}
				description="ข้อมูลที่กรอกยังอยู่ กรุณาตรวจสอบแล้วลองอีกครั้ง"
			/>{/if}
		{#if created}<p class="text-sm">
				บันทึกฉบับร่างแล้ว กำลังแนบไฟล์ · <a
					class="text-primary underline"
					href={`/staff/correspondence/${created.record.id}`}>เปิดฉบับร่าง</a
				>
			</p>{/if}
		<Card.Root
			><Card.Header><Card.Title>ข้อมูลทะเบียน</Card.Title></Card.Header><Card.Content
				class="grid gap-4 sm:grid-cols-2"
			>
				<Choice
					id="create-register"
					label="ประเภทหนังสือ"
					bind:value={register}
					disabled={pending || !!created}
					options={Object.entries(registerLabels).map(([value, label]) => ({ value, label }))}
				/>
				<Choice
					id="create-urgency"
					label="ความเร่งด่วน"
					bind:value={urgency}
					disabled={pending || !!created}
					options={[
						{ value: 'normal', label: 'ปกติ' },
						{ value: 'urgent', label: 'ด่วน' },
						{ value: 'very_urgent', label: 'ด่วนมาก' }
					]}
				/>
				<div class="space-y-2 sm:col-span-2">
					<Label for="create-title">เรื่อง *</Label><Input
						id="create-title"
						bind:value={title}
						required
						maxlength={500}
						disabled={pending || !!created}
					/>
				</div>
				<div class="space-y-2">
					<Label for="create-sender">ผู้ส่ง / หน่วยงานต้นทาง *</Label><Input
						id="create-sender"
						bind:value={sender}
						required
						disabled={pending || !!created}
					/>
				</div>
				<div class="space-y-2">
					<Label for="create-number">เลขที่หนังสือต้นทาง</Label><Input
						id="create-number"
						bind:value={documentNumber}
						disabled={pending || !!created}
					/>
				</div>
				<div class="space-y-2">
					<Label for="create-date">วันที่หนังสือ</Label><DatePicker
						id="create-date"
						bind:value={documentDate}
						disabled={pending || !!created}
						clearable
					/>
				</div>
				<div class="space-y-2 sm:col-span-2">
					{#if ownerOptions}<Choice
							id="create-owner"
							label="หน่วยงานเจ้าของเรื่อง (ถ้ามี)"
							bind:value={organizationUnitId}
							options={ownerOptions.organizations.map((unit) => ({
								value: unit.id,
								label: unit.name
							}))}
							searchable
							disabled={pending || !!created}
						/>{:else}<LoadingButton
							type="button"
							variant="outline"
							loading={ownerLoading}
							disabled={pending || !!created}
							onclick={loadOwners}>ระบุหน่วยงานเจ้าของเรื่อง</LoadingButton
						>{/if}{#if ownerError}<PageState
							variant="error"
							title={ownerError}
							actionLabel="ลองอีกครั้ง"
							onaction={loadOwners}
						/>{/if}
				</div>
			</Card.Content></Card.Root
		>
		<Card.Root
			><Card.Header><Card.Title>เนื้อหาและเอกสาร</Card.Title></Card.Header><Card.Content
				class="space-y-4"
			>
				<div class="space-y-2">
					<Label for="create-content">เนื้อหา / ข้อความประกอบ</Label><Textarea
						id="create-content"
						bind:value={content}
						rows={8}
						disabled={pending || !!created}
					/>
				</div>
				<div class="space-y-2">
					<Label for="create-original">เอกสารต้นฉบับ PDF หรือภาพ</Label><Input
						id="create-original"
						type="file"
						accept="application/pdf,image/png,image/jpeg,image/webp"
						disabled={pending || !!created}
						onchange={(event) => {
							original = event.currentTarget.files?.[0] ?? null;
						}}
					/>
				</div>
				<div class="space-y-2">
					<Label for="create-attachments">ไฟล์แนบ</Label><Input
						id="create-attachments"
						type="file"
						multiple
						disabled={pending || !!created}
						onchange={(event) => {
							attachments = Array.from(event.currentTarget.files ?? []);
						}}
					/>
				</div>
			</Card.Content></Card.Root
		>
		<Card.Root
			><Card.Header><Card.Title>แหล่งที่มา</Card.Title></Card.Header><Card.Content class="space-y-3"
				>{#each sourceLinks as link, index (index)}<div
						class="grid gap-3 sm:grid-cols-[1fr_2fr_auto]"
					>
						<div class="space-y-2">
							<Label for={`source-label-${index}`}>ชื่อแหล่งที่มา {index + 1}</Label><Input
								id={`source-label-${index}`}
								bind:value={link.label}
								disabled={pending || !!created}
							/>
						</div>
						<div class="space-y-2">
							<Label for={`source-url-${index}`}>ลิงก์ {index + 1}</Label><Input
								id={`source-url-${index}`}
								type="url"
								bind:value={link.url}
								disabled={pending || !!created}
							/>
						</div>
						<Button
							type="button"
							class="self-end"
							variant="ghost"
							disabled={pending || !!created}
							onclick={() => {
								sourceLinks = sourceLinks.filter((_, current) => current !== index);
							}}>นำออก</Button
						>
					</div>{/each}<Button
					type="button"
					variant="outline"
					disabled={pending || !!created}
					onclick={() => {
						sourceLinks = [...sourceLinks, { label: '', url: '' }];
					}}>เพิ่มแหล่งที่มา</Button
				></Card.Content
			></Card.Root
		>
		<div class="flex justify-end gap-3">
			<Button href="/staff/correspondence" variant="outline">ยกเลิก</Button><LoadingButton
				type="submit"
				loading={pending}>สร้างฉบับร่าง</LoadingButton
			>
		</div>
	</form>
</PageShell>
