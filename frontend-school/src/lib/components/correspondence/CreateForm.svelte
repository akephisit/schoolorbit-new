<script lang="ts">
	import { onDestroy } from 'svelte';
	import { goto } from '$app/navigation';
	import {
		createCorrespondence,
		sendCorrespondence,
		type ProcessStepRecipients,
		getCorrespondenceOptions,
		type CorrespondenceOptions,
		reviseCorrespondence,
		type CorrespondenceDetail,
		type ReviseCorrespondenceRequest,
		type SendCorrespondenceRequest,
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
	import { MutationIntent } from '#lib/correspondence/mutation-intent.js';
	import Choice from './Choice.svelte';
	import RoutePicker from './RoutePicker.svelte';
	import UnsavedGuard from './UnsavedGuard.svelte';
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
	let stage = $state(1),
		templateId = $state(''),
		steps = $state<ProcessStepRecipients[]>([]),
		dueDate = $state<string | undefined>();
	let routeOptions = $state<CorrespondenceOptions | null>(null),
		savedBasis = $state(''),
		finished = $state(false);
	type Upload = {
		file: File;
		id?: string;
		status: 'queued' | 'uploading' | 'ready' | 'failed';
		error?: string;
	};
	let uploads = $state<Upload[]>([]);
	const createIntent = new MutationIntent<CreateCorrespondenceRequest>();
	const revisionIntent = new MutationIntent<ReviseCorrespondenceRequest>();
	const sendIntent = new MutationIntent<SendCorrespondenceRequest>();
	let registrationLocked = $state(false),
		sendingUncertain = $state(false);
	let disposed = false;
	const formBasis = $derived(
		JSON.stringify([
			register,
			title,
			sender,
			documentNumber,
			documentDate,
			urgency,
			content,
			organizationUnitId,
			sourceLinks
		])
	);
	const saved = $derived(!!created && savedBasis === formBasis);
	let optionalError = $state(false);
	const routeTemplate = $derived(routeOptions?.templates.find((item) => item.id === templateId));
	const dirty = $derived(
		!finished &&
			(!saved || !!templateId) &&
			(!!title ||
				!!sender ||
				!!content ||
				!!original ||
				attachments.length > 0 ||
				sourceLinks.length > 0)
	);
	onDestroy(() => {
		disposed = true;
		ownerController.abort();
	});
	function validate(): boolean {
		if (!title.trim() || !sender.trim()) {
			error = 'กรอกเรื่องและผู้ส่งให้ครบ';
			stage = 1;
			return false;
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
			optionalError = true;
			stage = 1;
			return false;
		}
		return true;
	}
	function syncUploads() {
		const files = [...(original ? [original] : []), ...attachments];
		uploads = files.map(
			(file) => uploads.find((item) => item.file === file) ?? { file, status: 'queued' }
		);
	}
	async function upload(item: Upload) {
		if (item.id || !created) return;
		item.status = 'uploading';
		item.error = '';
		try {
			const result = await uploadFile(item.file, 'correspondence_document', created.record.id);
			if (disposed) return;
			item.id = result.id;
			item.status = 'ready';
		} catch (cause) {
			if (!disposed) {
				item.status = 'failed';
				item.error = cause instanceof Error ? cause.message : 'อัปโหลดไม่สำเร็จ';
			}
			throw cause;
		}
	}
	async function persist(): Promise<CorrespondenceDetail | null> {
		if (!validate()) return null;
		if (!created) {
			const payload = createIntent.begin({
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
			});
			registrationLocked = true;
			let result: CorrespondenceDetail;
			try {
				result = await createCorrespondence(payload);
				createIntent.accepted();
			} catch (cause) {
				createIntent.rejected(cause);
				registrationLocked = createIntent.unresolved;
				throw cause;
			}
			if (disposed) return null;
			created = result;
		}
		if (revisionIntent.unresolved) {
			const payload = revisionIntent.begin({ expectedRevision: created.record.revision, content });
			try {
				const result = await reviseCorrespondence(created.record.id, payload);
				if (disposed) return null;
				created = result;
				revisionIntent.accepted();
			} catch (cause) {
				revisionIntent.rejected(cause);
				throw cause;
			}
		}
		syncUploads();
		for (const item of uploads) {
			await upload(item);
			if (disposed) return null;
		}
		const originalFileId = original ? uploads.find((item) => item.file === original)?.id : null;
		const attachmentFileIds = uploads
			.filter((item) => attachments.includes(item.file))
			.map((item) => item.id!)
			.filter(Boolean);
		if (
			title.trim() !== created.record.title ||
			sender.trim() !== created.record.sender ||
			(documentNumber || null) !== (created.record.documentNumber ?? null) ||
			(documentDate || null) !== (created.record.documentDate ?? null) ||
			urgency !== created.record.urgency ||
			content !== created.document.content ||
			(originalFileId ?? null) !== (created.document.originalFileId ?? null) ||
			JSON.stringify(attachmentFileIds) !== JSON.stringify(created.document.attachmentFileIds)
		) {
			const body = {
				expectedRevision: created.record.revision,
				content,
				originalFileId,
				attachmentFileIds,
				annotations: [],
				metadata: {
					title: title.trim(),
					sender: sender.trim(),
					documentNumber: documentNumber || null,
					documentDate: documentDate || null,
					urgency: urgency as CreateCorrespondenceRequest['urgency']
				}
			};
			const payload = revisionIntent.begin(body);
			let result: CorrespondenceDetail;
			try {
				result = await reviseCorrespondence(created.record.id, payload);
				revisionIntent.accepted();
			} catch (cause) {
				revisionIntent.rejected(cause);
				throw cause;
			}
			if (disposed) return null;
			created = result;
		}
		savedBasis = formBasis;
		return created;
	}
	async function saveDraft() {
		if (pending) return;
		pending = true;
		error = '';
		try {
			await persist();
		} catch (cause) {
			if (!disposed) error = cause instanceof Error ? cause.message : 'บันทึกฉบับร่างไม่สำเร็จ';
		} finally {
			if (!disposed) pending = false;
		}
	}
	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (pending) return;
		if (stage === 1) {
			if (validate()) {
				stage = 2;
				error = '';
			}
			return;
		}
		if (
			!routeTemplate ||
			steps.length !== routeTemplate.steps.length ||
			steps.some((step) => !step.recipients.length)
		) {
			error = 'เลือกแม่แบบและผู้รับให้ครบทุกขั้นตอน';
			stage = 2;
			return;
		}
		if (stage === 2) {
			stage = 3;
			error = '';
			return;
		}
		pending = true;
		error = '';
		try {
			const draft = sendingUncertain ? created : await persist();
			if (!draft || disposed) return;
			const body = {
				expectedRevision: draft.record.revision,
				templateId,
				stepRecipients: steps,
				dueAt: dueDate ? `${dueDate}T23:59:59+07:00` : undefined
			};
			const payload = sendIntent.begin(body);
			let result: CorrespondenceDetail;
			try {
				result = await sendCorrespondence(draft.record.id, payload);
				sendIntent.accepted();
			} catch (cause) {
				sendIntent.rejected(cause);
				sendingUncertain = sendIntent.unresolved;
				throw cause;
			}
			if (disposed) return;
			created = result;
			finished = true;
			await goto(`/staff/correspondence/${result.record.id}`);
		} catch (cause) {
			if (!disposed)
				error = finished
					? 'ส่งหนังสือแล้ว แต่เปิดรายละเอียดไม่สำเร็จ กดเปิดหนังสือด้านบนเพื่อตรวจสอบ'
					: cause instanceof Error
						? cause.message
						: 'ส่งหนังสือไม่สำเร็จ';
		} finally {
			if (!disposed) pending = false;
		}
	}
</script>

<PageShell
	title="สร้างหนังสือ"
	backHref="/staff/correspondence"
	description="กรอกข้อมูล เลือกเส้นทางผู้รับ และตรวจสอบก่อนส่งในหน้านี้"
>
	<UnsavedGuard {dirty} {pending} />
	<nav aria-label="ขั้นตอนสร้างหนังสือ" class="mb-6 grid grid-cols-3 gap-2">
		{#each ['ข้อมูลเอกสาร', 'เส้นทางผู้รับ', 'ตรวจสอบส่ง'] as label, index (label)}
			<Button
				type="button"
				variant={stage === index + 1 ? 'secondary' : 'outline'}
				disabled={pending || sendingUncertain || index + 1 > stage}
				aria-current={stage === index + 1 ? 'step' : undefined}
				onclick={() => {
					stage = index + 1;
				}}>{index + 1}. {label}</Button
			>
		{/each}
	</nav>
	<form
		class="w-full min-w-0 space-y-6"
		onsubmit={submit}
		oninput={() => {
			savedBasis = '';
		}}
	>
		{#if error}<PageState
				variant="error"
				title={error}
				description="ข้อมูลที่กรอกยังอยู่ กรุณาตรวจสอบแล้วลองอีกครั้ง"
			/>{/if}
		{#if created}<div
				class="rounded-lg border border-primary/20 bg-primary/5 p-3 text-sm"
				role="status"
			>
				<p>
					เลขทะเบียน {created.record.registerNumber}/{created.record.registerYear} · {saved
						? 'บันทึกฉบับร่างแล้ว ยังไม่ได้ส่ง'
						: 'มีฉบับร่างในระบบ ยังมีข้อมูลหรือไฟล์ที่ต้องบันทึก'}
				</p>
				<p class="text-muted-foreground">
					ประเภททะเบียนและหน่วยงานเจ้าของเรื่องบันทึกแล้ว แก้ไขข้อมูลเอกสารและไฟล์ต่อได้
					ผู้รับจะบันทึกเมื่อส่งหนังสือ
				</p>
				<a class="text-primary underline" href={`/staff/correspondence/${created.record.id}`}
					>{finished ? 'เปิดหนังสือที่ส่งแล้ว' : 'เปิดฉบับร่าง'}</a
				>
			</div>{/if}
		<fieldset disabled={sendingUncertain} hidden={stage !== 1} class="min-w-0 space-y-6">
			<Card.Root
				><Card.Header><Card.Title>ข้อมูลทะเบียน</Card.Title></Card.Header><Card.Content
					class="grid gap-4 sm:grid-cols-2"
				>
					<Choice
						id="create-register"
						label="ประเภทหนังสือ"
						bind:value={register}
						disabled={pending || registrationLocked}
						options={Object.entries(registerLabels).map(([value, label]) => ({ value, label }))}
					/>
					<Choice
						id="create-urgency"
						label="ความเร่งด่วน"
						bind:value={urgency}
						disabled={pending}
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
							disabled={pending}
						/>
					</div>
					<div class="space-y-2">
						<Label for="create-sender">ผู้ส่ง / หน่วยงานต้นทาง *</Label><Input
							id="create-sender"
							bind:value={sender}
							required
							disabled={pending}
						/>
					</div>
					<details class="space-y-4 sm:col-span-2">
						<summary class="cursor-pointer text-sm font-medium"
							>ข้อมูลเพิ่มเติม (เลขที่ต้นทาง วันที่ และหน่วยงาน)</summary
						>
						<div class="mt-4 grid gap-4 sm:grid-cols-2">
							<div class="space-y-2">
								<Label for="create-number">เลขที่หนังสือต้นทาง</Label><Input
									id="create-number"
									bind:value={documentNumber}
									disabled={pending}
								/>
							</div>
							<div class="space-y-2">
								<Label for="create-date">วันที่หนังสือ</Label><DatePicker
									id="create-date"
									bind:value={documentDate}
									disabled={pending}
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
										disabled={pending || registrationLocked}
									/>{:else}<LoadingButton
										type="button"
										variant="outline"
										loading={ownerLoading}
										disabled={pending || registrationLocked}
										onclick={loadOwners}>ระบุหน่วยงานเจ้าของเรื่อง</LoadingButton
									>{/if}{#if ownerError}<PageState
										variant="error"
										title={ownerError}
										actionLabel="ลองอีกครั้ง"
										onaction={loadOwners}
									/>{/if}
							</div>
						</div>
					</details>
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
							disabled={pending}
						/>
					</div>
					<div class="space-y-2">
						<Label for="create-original">เอกสารต้นฉบับ PDF หรือภาพ</Label><Input
							id="create-original"
							type="file"
							accept="application/pdf,image/png,image/jpeg,image/webp"
							disabled={pending}
							onchange={(event) => {
								original = event.currentTarget.files?.[0] ?? null;
								savedBasis = '';
								syncUploads();
							}}
						/>
					</div>
					<div class="space-y-2">
						<Label for="create-attachments">ไฟล์แนบ</Label><Input
							id="create-attachments"
							type="file"
							multiple
							disabled={pending}
							onchange={(event) => {
								attachments = [...attachments, ...Array.from(event.currentTarget.files ?? [])];
								event.currentTarget.value = '';
								savedBasis = '';
								syncUploads();
							}}
						/>
					</div>
				</Card.Content></Card.Root
			>
			<Card.Root
				><Card.Content class="space-y-3"
					><details open={optionalError}>
						<summary class="cursor-pointer font-medium">แหล่งที่มาเพิ่มเติม</summary>
						<div class="mt-4 space-y-3">
							{#each sourceLinks as link, index (index)}<div
									class="grid gap-3 sm:grid-cols-[1fr_2fr_auto]"
								>
									<div class="space-y-2">
										<Label for={`source-label-${index}`}>ชื่อแหล่งที่มา {index + 1}</Label><Input
											id={`source-label-${index}`}
											bind:value={link.label}
											disabled={pending || registrationLocked}
										/>
									</div>
									<div class="space-y-2">
										<Label for={`source-url-${index}`}>ลิงก์ {index + 1}</Label><Input
											id={`source-url-${index}`}
											type="url"
											bind:value={link.url}
											disabled={pending || registrationLocked}
										/>
									</div>
									<Button
										type="button"
										class="self-end"
										variant="ghost"
										disabled={pending || registrationLocked}
										onclick={() => {
											sourceLinks = sourceLinks.filter((_, current) => current !== index);
										}}>นำออก</Button
									>
								</div>{/each}<Button
								type="button"
								variant="outline"
								disabled={pending || registrationLocked}
								onclick={() => {
									sourceLinks = [...sourceLinks, { label: '', url: '' }];
								}}>เพิ่มแหล่งที่มา</Button
							>
						</div>
					</details></Card.Content
				></Card.Root
			>
		</fieldset>
		<section hidden={stage !== 2} class="space-y-4" aria-label="เส้นทางผู้รับ">
			{#if stage >= 2}<RoutePicker
					bind:templateId
					bind:steps
					bind:dueDate
					disabled={pending || sendingUncertain}
					onready={(value) => {
						routeOptions = value;
					}}
				/>{/if}
		</section>
		{#if stage === 3}<section
				class="space-y-4 rounded-xl border bg-card p-4"
				aria-label="ตรวจสอบก่อนส่ง"
			>
				<h2 class="font-semibold">ตรวจสอบก่อนส่ง</h2>
				<dl class="grid gap-2 text-sm">
					<dt class="text-muted-foreground">เรื่อง</dt>
					<dd class="break-words font-medium">{title}</dd>
					<dt class="text-muted-foreground">ผู้ส่ง</dt>
					<dd>{sender}</dd>
					<dt class="text-muted-foreground">เส้นทาง</dt>
					<dd>{routeTemplate?.title} · {steps.length} ขั้นตอน</dd>
				</dl>
				<ol class="space-y-2 text-sm">
					{#each routeTemplate?.steps ?? [] as step, index (index)}<li>
							{index + 1}. {step.title} · เลือกผู้รับ {steps[index]?.recipients.length ?? 0} รายการ
						</li>{/each}
				</ol>
				<p class="text-sm text-muted-foreground">
					เมื่อกดส่ง ระบบจะบันทึกผู้รับและเส้นทางสำหรับหนังสือฉบับนี้
				</p>
			</section>{/if}
		{#if sendingUncertain}<PageState
				variant="error"
				title="ยังยืนยันผลการส่งครั้งก่อนไม่ได้"
				description="ข้อมูลและผู้รับครั้งก่อนยังอยู่ กดยืนยันส่งอีกครั้งเพื่อตรวจผลด้วยคำขอเดิม ระบบจะไม่สร้างหนังสือหรือส่งซ้ำ"
			/>{/if}
		{#if uploads.length}<section class="space-y-2" aria-label="ไฟล์ที่เลือก">
				<h2 class="text-sm font-medium">ไฟล์ที่เลือก ({uploads.length})</h2>
				{#each uploads as item, index (index)}<div
						class="flex flex-wrap items-center gap-2 rounded-lg border p-3 text-sm"
					>
						<span class="min-w-0 flex-1 break-words"
							>{item.file.name} · {Math.ceil(item.file.size / 1024)} KB {item.file === original
								? '· ต้นฉบับ'
								: ''}</span
						>
						<span role="status"
							>{{
								queued: 'รออัปโหลด',
								uploading: 'กำลังอัปโหลด…',
								ready: 'อัปโหลดแล้ว',
								failed: 'อัปโหลดไม่สำเร็จ'
							}[item.status]}</span
						>
						{#if item.status === 'uploading'}<progress
								class="w-20"
								aria-label={`กำลังอัปโหลด ${item.file.name}`}
							></progress>{/if}
						{#if item.error}<p class="w-full text-destructive">{item.error}</p>
							<Button
								type="button"
								size="sm"
								variant="outline"
								disabled={pending}
								onclick={saveDraft}>ลองอัปโหลดอีกครั้ง</Button
							>{/if}
						<Button
							type="button"
							size="sm"
							variant="ghost"
							disabled={pending}
							aria-label={`นำไฟล์ ${item.file.name} ออก`}
							onclick={() => {
								if (item.file === original) original = null;
								attachments = attachments.filter((file) => file !== item.file);
								syncUploads();
								savedBasis = '';
							}}>นำออก</Button
						>
					</div>{/each}
			</section>{/if}
		<div class="sticky bottom-0 z-10 flex flex-wrap justify-end gap-2 border-t bg-background py-3">
			<Button href="/staff/correspondence" variant="ghost" disabled={pending}>ยกเลิก</Button>
			{#if stage > 1}<Button
					type="button"
					variant="outline"
					disabled={pending || sendingUncertain}
					onclick={() => {
						stage -= 1;
					}}>ย้อนกลับ</Button
				>{/if}
			<LoadingButton
				type="button"
				variant="outline"
				loading={pending}
				disabled={sendingUncertain || !title.trim() || !sender.trim()}
				onclick={saveDraft}>บันทึกฉบับร่าง</LoadingButton
			>
			<LoadingButton type="submit" loading={pending} disabled={finished}
				>{stage === 3 ? 'ยืนยันส่งหนังสือ' : 'ถัดไป'}</LoadingButton
			>
		</div>
	</form>
</PageShell>
