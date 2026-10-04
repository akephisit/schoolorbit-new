<script lang="ts">
	import { untrack } from 'svelte';
	import type { LearningDeliveryRefreshScope } from '#lib/academic/learning-delivery-page.js';
	import {
		deleteDeliveryVersion,
		getDeliveryVersion,
		getAcademicTermChangeSet,
		previewAcademicTermChangeSet,
		publishAcademicTermChangeSet,
		type AcademicTermChangeSet,
		type AcademicTermChangeSetPreview,
		type AcademicChangeFindingCode,
		type DeleteDeliveryVersionRequest
	} from '#lib/api/learning-delivery.js';
	import { ApiClientError } from '#lib/api/client.js';
	import {
		LoadingButton,
		PageState,
		RegionUpdatingState
	} from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { DatePicker } from '#lib/components/ui/date-picker/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import {
		RefreshCw,
		Send,
		Trash2,
		ExternalLink,
		CircleAlert,
		TriangleAlert,
		CheckCircle2
	} from '@lucide/svelte';

	let {
		changeSet,
		canManage,
		onChanged,
		onDeleted
	}: {
		changeSet: AcademicTermChangeSet;
		canManage: boolean;
		onChanged: (
			changeSet: AcademicTermChangeSet,
			scope?: LearningDeliveryRefreshScope
		) => void | Promise<void>;
		onDeleted: (sourceVersionId: string | null) => Promise<void>;
	} = $props();
	let preview = $state.raw<AcademicTermChangeSetPreview | null>(null);
	let loadingPreview = $state(false);
	let publishing = $state(false);
	let deleting = $state(false);
	let preparingDelete = $state(false);
	let publishOpen = $state(false);
	let deleteOpen = $state(false);
	let selectedDate = $state('');
	let publicationKey = $state('');
	let errorMessage = $state('');
	let deleteRequest = $state.raw<DeleteDeliveryVersionRequest | null>(null);
	let acknowledgedWarnings = $state<AcademicChangeFindingCode[]>([]);
	let blocking = $derived(preview?.findings.filter((f) => f.severity === 'blocking') ?? []);
	let warnings = $derived(preview?.findings.filter((f) => f.severity === 'warning') ?? []);
	let canPublish = $derived(
		Boolean(
			preview &&
			!preview.preliminary &&
			preview.effectiveFrom === selectedDate &&
			blocking.length === 0 &&
			warnings.every((f) => acknowledgedWarnings.includes(f.code))
		)
	);
	let requestRevision = 0;

	let knownRevision: number | undefined;
	$effect(() => {
		const revision = changeSet.rowVersion;
		untrack(() => {
			if (knownRevision !== undefined && knownRevision !== revision) clearPreview();
			knownRevision = revision;
		});
	});

	function clearPreview() {
		requestRevision++;
		loadingPreview = false;
		preview = null;
		acknowledgedWarnings = [];
		errorMessage = '';
		publicationKey = '';
	}
	function dateChanged(value: string | undefined) {
		selectedDate = value ?? '';
		clearPreview();
	}
	function openPublication() {
		clearPreview();
		const today = new Intl.DateTimeFormat('en-CA', {
			timeZone: 'Asia/Bangkok',
			year: 'numeric',
			month: '2-digit',
			day: '2-digit'
		}).format(new Date());
		selectedDate = changeSet.referenceDate > today ? changeSet.referenceDate : today;
		publishOpen = true;
	}
	async function recover(message: string) {
		clearPreview();
		errorMessage = message;
		try {
			await onChanged(await getAcademicTermChangeSet(changeSet.id));
		} catch (error) {
			errorMessage = error instanceof Error ? `${message} (${error.message})` : message;
		}
	}
	async function refreshPreview() {
		const revision = ++requestRevision;
		loadingPreview = true;
		preview = null;
		acknowledgedWarnings = [];
		errorMessage = '';
		publicationKey = '';
		try {
			const current = await getAcademicTermChangeSet(changeSet.id);
			if (revision !== requestRevision) return;
			if (current.rowVersion !== changeSet.rowVersion) {
				await onChanged(current);
				errorMessage = 'ข้อมูลเปลี่ยน กรุณาตรวจความพร้อมอีกครั้ง';
				return;
			}
			const loaded = await previewAcademicTermChangeSet(
				changeSet.id,
				{},
				publishOpen ? selectedDate : undefined
			);
			if (revision !== requestRevision) return;
			if (loaded.changeSetRowVersion !== current.rowVersion) {
				await recover('ข้อมูลเปลี่ยนระหว่างตรวจ กรุณาตรวจใหม่');
				return;
			}
			preview = loaded;
		} catch (error) {
			if (revision !== requestRevision) return;
			if (error instanceof ApiClientError && error.status === 409) await recover(error.message);
			else errorMessage = error instanceof Error ? error.message : 'ตรวจความพร้อมไม่สำเร็จ';
		} finally {
			if (revision === requestRevision) loadingPreview = false;
		}
	}
	async function publish() {
		if (!canManage || !canPublish || !preview) return;
		publishing = true;
		errorMessage = '';
		publicationKey ||= crypto.randomUUID();
		try {
			const updated = await publishAcademicTermChangeSet(changeSet.id, {
				effectiveFrom: selectedDate,
				rowVersion: preview.changeSetRowVersion,
				targetDeliveryVersionRowVersion: preview.targetDeliveryVersionRowVersion,
				previewHash: preview.previewHash,
				acknowledgedWarningCodes: [...new Set(warnings.map((f) => f.code))],
				idempotencyKey: publicationKey
			});
			await onChanged(updated, 'homerooms');
			publishOpen = false;
			clearPreview();
		} catch (error) {
			if (error instanceof ApiClientError && error.status === 409) await recover(error.message);
			else errorMessage = error instanceof Error ? error.message : 'เผยแพร่รุ่นเปิดสอนไม่สำเร็จ';
		} finally {
			publishing = false;
		}
	}
	async function prepareDeletion() {
		preparingDelete = true;
		errorMessage = '';
		deleteRequest = null;
		try {
			const [current, version] = await Promise.all([
				getAcademicTermChangeSet(changeSet.id),
				getDeliveryVersion(changeSet.targetDeliveryVersionId)
			]);
			if (current.rowVersion !== changeSet.rowVersion) {
				await onChanged(current);
				errorMessage = 'ข้อมูลเปลี่ยน กรุณาตรวจรายการล่าสุดก่อนลบ';
				return;
			}
			deleteRequest = {
				rowVersion: version.rowVersion,
				changeSetRowVersion: current.rowVersion,
				expectedItemCount: current.items.length,
				expectedOfferingCount: version.snapshot.offerings.length,
				expectedGroupCount: version.snapshot.offerings.reduce((n, o) => n + o.groups.length, 0)
			};
			deleteOpen = true;
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'โหลดข้อมูลก่อนลบไม่สำเร็จ';
		} finally {
			preparingDelete = false;
		}
	}
	async function deleteDraft() {
		if (!canManage || !deleteRequest) return;
		deleting = true;
		errorMessage = '';
		try {
			const result = await deleteDeliveryVersion(changeSet.targetDeliveryVersionId, deleteRequest);
			deleteOpen = false;
			await onDeleted(result.sourceVersionId ?? null);
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'ลบแบบร่างไม่สำเร็จ';
			if (error instanceof ApiClientError && error.status === 409) deleteRequest = null;
		} finally {
			deleting = false;
		}
	}
</script>

{#snippet result()}
	{#if loadingPreview}
		<div class="relative min-h-24"><RegionUpdatingState label="กำลังตรวจความพร้อม" /></div>
	{:else if preview}
		{#if preview.preliminary}<p class="text-sm text-muted-foreground">
				ผลตรวจเบื้องต้น ต้องเลือกวันเริ่มใช้และตรวจอีกครั้งตอนเผยแพร่
			</p>{/if}
		<div class="space-y-3" aria-label="ผลตรวจความพร้อม">
			{#each blocking as finding, index (`${finding.code}:${finding.learningOfferingId ?? ''}:${finding.learningGroupId ?? ''}:${finding.resourceId ?? ''}:${index}`)}
				<div class="rounded-xl border border-destructive/25 bg-destructive/5 p-3">
					<p class="flex items-start gap-2 font-medium text-destructive">
						<CircleAlert class="size-4 shrink-0" />{finding.title}
					</p>
					<p class="mt-1 text-sm text-muted-foreground">{finding.guidance}</p>
					{#if finding.route}<Button href={finding.route} variant="link" size="sm"
							>ไปแก้ไข <ExternalLink class="size-3" /></Button
						>{/if}
				</div>
			{:else}<p class="flex items-center gap-2 text-sm">
					<CheckCircle2 class="size-4 text-primary" />ไม่มีจุดบล็อกการเผยแพร่
				</p>{/each}
			{#each warnings as finding, index (`${finding.code}:${finding.learningOfferingId ?? ''}:${finding.learningGroupId ?? ''}:${finding.resourceId ?? ''}:${index}`)}
				<label class="flex items-start gap-2 rounded-xl border p-3">
					<Checkbox
						checked={acknowledgedWarnings.includes(finding.code)}
						onCheckedChange={(checked) =>
							(acknowledgedWarnings = checked
								? [...new Set([...acknowledgedWarnings, finding.code])]
								: acknowledgedWarnings.filter((c) => c !== finding.code))}
						aria-label={`รับทราบ ${finding.title}`}
					/>
					<span
						><span class="flex items-center gap-1 font-medium"
							><TriangleAlert class="size-4" />{finding.title}</span
						><span class="block text-sm text-muted-foreground">{finding.guidance}</span></span
					>
				</label>
			{/each}
		</div>
		{#if !preview.preliminary}<div class="space-y-2" aria-label="การเปลี่ยนแปลงตามวันเริ่มใช้">
				<p class="font-medium">ข้อมูลที่จะเผยแพร่ตามวันที่เลือก</p>
				{#each preview.changes as change, index (`${change.resourceId}:${change.field}:${index}`)}<p
						class="text-sm"
					>
						{change.label} · {change.field}: {change.before ?? 'ยังไม่มี'} → {change.after ??
							'ไม่ใช้ในรุ่นใหม่'}
					</p>{/each}
			</div>{/if}
	{:else}<PageState
			variant="empty"
			title="ยังไม่ได้ตรวจความพร้อม"
			description="ตรวจข้อมูลรายวิชา กลุ่ม และครูก่อนเผยแพร่"
		/>{/if}
{/snippet}

<section class="space-y-4" aria-label="ตรวจและเผยแพร่รุ่นเปิดสอน">
	{#if changeSet.status === 'draft'}
		<div class="flex flex-wrap items-center justify-between gap-3">
			<div>
				<h3 class="font-medium">ตรวจผลกระทบและความพร้อม</h3>
				<p class="text-xs text-muted-foreground">ผลเบื้องต้นก่อนเลือกวันเริ่มใช้</p>
			</div>
			<LoadingButton
				variant="outline"
				size="sm"
				loading={loadingPreview}
				loadingLabel="กำลังตรวจ"
				disabled={publishing || deleting}
				onclick={refreshPreview}><RefreshCw class="size-4" />ตรวจความพร้อม</LoadingButton
			>
		</div>
		{#if !publishOpen}{@render result()}{/if}
	{:else if changeSet.status === 'published'}<p class="text-sm">
			เผยแพร่แล้ว เริ่มใช้ {changeSet.effectiveFrom} · ตารางเดิมยังคงข้อมูลของรุ่นที่อ้างอิง
		</p>{/if}
	{#if canManage && changeSet.status !== 'published'}
		<div class="flex flex-wrap gap-2">
			{#if changeSet.status === 'draft'}<Button
					onclick={openPublication}
					disabled={loadingPreview || preparingDelete || deleting}
					><Send class="size-4" />เผยแพร่รุ่นเปิดสอน</Button
				>{/if}
			<LoadingButton
				variant="destructive"
				loading={preparingDelete}
				loadingLabel="กำลังตรวจข้อมูล"
				disabled={loadingPreview || publishing || deleting}
				onclick={prepareDeletion}
				><Trash2 class="size-4" />{changeSet.status === 'draft'
					? 'ลบแบบร่าง'
					: 'ลบรุ่นที่ยกเลิก'}</LoadingButton
			>
		</div>
	{/if}
	{#if errorMessage && !publishOpen && !deleteOpen}<p role="alert" class="text-sm text-destructive">
			{errorMessage}
		</p>{/if}
</section>

<Dialog.Root bind:open={publishOpen}>
	<Dialog.Content
		showCloseButton={!publishing}
		escapeKeydownBehavior={publishing ? 'ignore' : 'close'}
		interactOutsideBehavior={publishing ? 'ignore' : 'close'}
		class="max-h-[85dvh] overflow-y-auto sm:max-w-2xl"
	>
		<Dialog.Header
			><Dialog.Title>เผยแพร่รุ่นเปิดสอน</Dialog.Title><Dialog.Description
				>เลือกวันเริ่มใช้ ตรวจข้อมูลตามวันนั้น และรับทราบคำเตือนก่อนยืนยัน</Dialog.Description
			></Dialog.Header
		>
		<div class="space-y-2">
			<Label for="delivery-publication-date">วันที่เริ่มใช้</Label><DatePicker
				id="delivery-publication-date"
				value={selectedDate}
				onValueChange={dateChanged}
				ariaLabel="วันที่เริ่มใช้รุ่นเปิดสอน"
				disabled={publishing}
			/>
		</div>
		<LoadingButton
			variant="outline"
			loading={loadingPreview}
			loadingLabel="กำลังตรวจ"
			disabled={!selectedDate || publishing}
			onclick={refreshPreview}>ตรวจความพร้อมตามวันที่เลือก</LoadingButton
		>
		{@render result()}
		{#if errorMessage}<p role="alert" class="text-sm text-destructive">{errorMessage}</p>{/if}
		<Dialog.Footer
			><Button
				variant="outline"
				disabled={publishing}
				onclick={() => {
					publishOpen = false;
					clearPreview();
				}}>กลับไปแก้ไข</Button
			><LoadingButton
				loading={publishing}
				loadingLabel="กำลังเผยแพร่"
				disabled={!canPublish || loadingPreview}
				onclick={publish}>ยืนยันเผยแพร่</LoadingButton
			></Dialog.Footer
		>
	</Dialog.Content>
</Dialog.Root>
<Dialog.Root bind:open={deleteOpen}>
	<Dialog.Content
		showCloseButton={!deleting}
		escapeKeydownBehavior={deleting ? 'ignore' : 'close'}
		interactOutsideBehavior={deleting ? 'ignore' : 'close'}
		><Dialog.Header
			><Dialog.Title>ลบแบบร่างถาวร</Dialog.Title><Dialog.Description
				>“{changeSet.reason}” มี {deleteRequest?.expectedItemCount ?? changeSet.items.length} รายการเปลี่ยนแปลง
				· {deleteRequest?.expectedOfferingCount ?? 0} รายการเปิดสอน · {deleteRequest?.expectedGroupCount ??
					0} กลุ่ม รุ่นนี้จะถูกลบออกจากฐานข้อมูลและกู้คืนผ่านหน้านี้ไม่ได้</Dialog.Description
			></Dialog.Header
		>
		<p class="text-sm text-muted-foreground">
			ลบข้อมูลลูกที่ร่างสร้างและยังไม่มีผู้อื่นใช้งาน รุ่นต้นทางที่เผยแพร่แล้วจะยังอยู่
			หากมีข้อมูลอ้างอิงระบบจะปฏิเสธการลบทั้งหมด
		</p>
		{#if errorMessage}<p role="alert" class="text-sm text-destructive">{errorMessage}</p>{/if}
		<Dialog.Footer
			><Button variant="outline" disabled={deleting} onclick={() => (deleteOpen = false)}
				>กลับ</Button
			><LoadingButton
				variant="destructive"
				loading={deleting}
				loadingLabel="กำลังลบ"
				disabled={!deleteRequest}
				onclick={deleteDraft}>ยืนยันลบถาวร</LoadingButton
			></Dialog.Footer
		>
	</Dialog.Content>
</Dialog.Root>
