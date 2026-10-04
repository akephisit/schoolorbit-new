<script lang="ts">
	import { onMount } from 'svelte';
	import type { LearningDeliveryRefreshScope } from '#lib/academic/learning-delivery-page.js';
	import {
		applyTeacherHandoff,
		getAcademicTermChangeSet,
		previewTeacherHandoff,
		type AcademicTermChangeSet,
		type ApplyTeacherHandoffResponse,
		type DeliveryManagementOptions,
		type TeacherHandoffMode,
		type TeacherHandoffPreview
	} from '#lib/api/learning-delivery.js';
	import { listTimetableVersions, type TimetableVersion } from '#lib/api/timetable.js';
	import { getDeliveryVersion, type DeliveryVersion } from '#lib/api/learning-delivery.js';
	import { ApiClientError } from '#lib/api/client.js';
	import { isAbortError } from '#lib/async/latest-request.js';
	import { LoadingButton } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import {
		AlertTriangle,
		ArrowRight,
		CheckCircle2,
		ExternalLink,
		RefreshCw,
		UsersRound,
		X
	} from '@lucide/svelte';
	import DeliveryOptionCombobox from './DeliveryOptionCombobox.svelte';

	type ChangeItem = AcademicTermChangeSet['items'][number];
	type StopTeacherItem = Extract<ChangeItem, { actionKind: 'stop_group_teacher' }>;

	let {
		changeSet,
		teacherChangeItem,
		managementOptions,
		onChanged,
		onApplied,
		onClose
	}: {
		changeSet: AcademicTermChangeSet;
		teacherChangeItem: StopTeacherItem;
		managementOptions: DeliveryManagementOptions;
		onChanged: (
			changeSet: AcademicTermChangeSet,
			refreshScope?: LearningDeliveryRefreshScope
		) => void | Promise<void>;
		onApplied: (result: ApplyTeacherHandoffResponse) => void | Promise<void>;
		onClose: () => void;
	} = $props();

	let mode = $state<TeacherHandoffMode>('assign_one');
	let selectedInstructorIds = $state<string[]>([]);
	let selectedEntryIds = $state<string[]>([]);
	let selectionInitialized = $state(false);
	let targetTimetableVersionRowVersion = $state(0);
	let timetableVersionId = $state('');
	let drafts = $state<TimetableVersion[]>([]);
	let deliverySource = $state.raw<DeliveryVersion | null>(null);
	let preview = $state.raw<TeacherHandoffPreview | null>(null);
	let loading = $state(false);
	let applying = $state(false);
	let appliedCount = $state(0);
	let errorMessage = $state('');
	let previewController: AbortController | null = null;
	let previewRevision = 0;

	let projectedTeacherIds = $derived(
		deliverySource?.snapshot.offerings
			.flatMap((offering) => offering.groups)
			.find((group) => group.id === teacherChangeItem.learningGroupId)
			?.teachers.map((teacher) => teacher.teacherId) ?? []
	);
	let replacementOptions = $derived(
		managementOptions.teachers
			.filter((teacher) => projectedTeacherIds.includes(teacher.id))
			.map((teacher) => ({
				id: teacher.id,
				label: teacher.name,
				description: teacher.title
			}))
	);
	let canPreviewChoice = $derived(
		mode === 'manual' ||
			(mode === 'assign_one' && selectedInstructorIds.length === 1) ||
			(mode === 'assign_coteachers' && selectedInstructorIds.length > 0)
	);
	let selectedEntries = $derived(
		(preview?.affectedEntries ?? []).filter((entry) => selectedEntryIds.includes(entry.entryId))
	);

	function modeLabel(value: TeacherHandoffMode): string {
		return value === 'assign_one'
			? 'ให้ครูคนเดียวสอนทุกคาบที่เลือก'
			: value === 'assign_coteachers'
				? 'ให้ครูหลายคนสอนร่วมกัน'
				: 'จัดครูเองในหน้าตารางสอน';
	}

	function dayLabel(value: string): string {
		const labels: Record<string, string> = {
			monday: 'จันทร์',
			tuesday: 'อังคาร',
			wednesday: 'พุธ',
			thursday: 'พฤหัสบดี',
			friday: 'ศุกร์',
			saturday: 'เสาร์',
			sunday: 'อาทิตย์'
		};
		return labels[value.toLowerCase()] ?? value;
	}

	function setMode(value: string) {
		mode = value as TeacherHandoffMode;
		selectedInstructorIds = [];
		preview = null;
		appliedCount = 0;
		errorMessage = '';
		void refreshPreview();
	}

	function selectOne(value: string) {
		selectedInstructorIds = value ? [value] : [];
		void refreshPreview();
	}

	function toggleInstructor(teacherId: string, checked: boolean) {
		selectedInstructorIds = checked
			? [...selectedInstructorIds.filter((id) => id !== teacherId), teacherId].sort()
			: selectedInstructorIds.filter((id) => id !== teacherId);
		void refreshPreview();
	}

	function toggleEntry(entryId: string, checked: boolean) {
		const next = checked
			? [...selectedEntryIds.filter((id) => id !== entryId), entryId].sort()
			: selectedEntryIds.filter((id) => id !== entryId);
		if (next.length === 0) {
			errorMessage = 'ต้องเลือกอย่างน้อยหนึ่งคาบ หรือใช้โหมดจัดเองในหน้าตารางสอน';
			return;
		}
		selectedEntryIds = next;
		errorMessage = '';
		void refreshPreview();
	}

	async function loadTargetRevision(signal: AbortSignal): Promise<void> {
		const [listed, source] = await Promise.all([
			listTimetableVersions(changeSet.academicTermId, { signal }),
			getDeliveryVersion(changeSet.targetDeliveryVersionId, { signal })
		]);
		drafts = listed.filter(
			(version) => version.status === 'draft' && version.deliveryVersionId === source.id
		);
		deliverySource = source;
		if (!timetableVersionId && drafts.length === 1) timetableVersionId = drafts[0].id;
		const selected = drafts.find((version) => version.id === timetableVersionId);
		targetTimetableVersionRowVersion = selected?.rowVersion ?? 0;
	}

	async function recoverStale(): Promise<void> {
		const latest = await getAcademicTermChangeSet(changeSet.id);
		await onChanged(latest);
		const controller = new AbortController();
		await loadTargetRevision(controller.signal);
	}

	async function refreshPreview(): Promise<void> {
		previewController?.abort();
		if (!canPreviewChoice) {
			preview = null;
			loading = false;
			return;
		}
		const controller = new AbortController();
		previewController = controller;
		const revision = ++previewRevision;
		loading = true;
		errorMessage = '';
		try {
			if (targetTimetableVersionRowVersion <= 0) await loadTargetRevision(controller.signal);
			if (!timetableVersionId || targetTimetableVersionRowVersion <= 0)
				throw new Error('กรุณาเลือกร่างตารางที่อ้างอิงรุ่นเปิดสอนนี้ก่อน');
			const result = await previewTeacherHandoff(
				changeSet.id,
				{
					changeSetRowVersion: changeSet.rowVersion,
					timetableVersionId,
					targetTimetableVersionRowVersion,
					teacherChangeItemId: teacherChangeItem.id,
					entryIds: selectionInitialized ? selectedEntryIds : [],
					mode,
					instructorIds: mode === 'manual' ? [] : selectedInstructorIds
				},
				{ signal: controller.signal }
			);
			if (revision !== previewRevision) return;
			preview = result;
			if (!selectionInitialized) {
				selectedEntryIds = result.affectedEntries.map((entry) => entry.entryId);
				selectionInitialized = true;
			}
		} catch (error) {
			if (isAbortError(error) || revision !== previewRevision) return;
			if (error instanceof ApiClientError && error.status === 409) {
				try {
					await recoverStale();
					errorMessage = 'ข้อมูลเปลี่ยนระหว่างตรวจ ระบบโหลดรุ่นล่าสุดแล้ว กรุณาตรวจอีกครั้ง';
				} catch (reloadError) {
					errorMessage =
						reloadError instanceof Error ? reloadError.message : 'โหลดข้อมูลรุ่นล่าสุดไม่สำเร็จ';
				}
				return;
			}
			errorMessage = error instanceof Error ? error.message : 'ตรวจคาบที่ได้รับผลกระทบไม่สำเร็จ';
		} finally {
			if (revision === previewRevision) loading = false;
		}
	}

	async function applyPreview(): Promise<void> {
		if (
			mode === 'manual' ||
			!preview?.canApply ||
			!preview.previewHash ||
			preview.conflicts.length > 0
		)
			return;
		applying = true;
		errorMessage = '';
		try {
			const result = await applyTeacherHandoff(changeSet.id, {
				changeSetRowVersion: preview.changeSetRowVersion,
				timetableVersionId: preview.targetTimetableVersionId,
				targetTimetableVersionRowVersion: preview.targetTimetableVersionRowVersion,
				teacherChangeItemId: teacherChangeItem.id,
				entries: preview.proposedEntries.map((entry) => ({
					entryId: entry.entryId,
					rowVersion: entry.rowVersion
				})),
				mode,
				instructorIds: selectedInstructorIds,
				previewHash: preview.previewHash,
				idempotencyKey: crypto.randomUUID()
			});
			appliedCount = result.updatedEntries.length;
			await onApplied(result);
		} catch (error) {
			if (error instanceof ApiClientError && error.status === 409) {
				try {
					await recoverStale();
					await refreshPreview();
					errorMessage = 'ข้อมูลตารางเปลี่ยนหลังตรวจ ระบบสร้างตัวอย่างล่าสุดให้แล้ว';
				} catch (reloadError) {
					errorMessage =
						reloadError instanceof Error ? reloadError.message : 'โหลดข้อมูลล่าสุดไม่สำเร็จ';
				}
				return;
			}
			errorMessage = error instanceof Error ? error.message : 'ส่งต่อคาบไม่สำเร็จ';
		} finally {
			applying = false;
		}
	}

	onMount(() => {
		const initialization = new AbortController();
		void loadTargetRevision(initialization.signal)
			.then(() => refreshPreview())
			.catch((error: unknown) => {
				if (!isAbortError(error))
					errorMessage = error instanceof Error ? error.message : 'โหลดแบบร่างตารางไม่สำเร็จ';
			});
		return () => {
			initialization.abort();
			previewController?.abort();
		};
	});
</script>

<section class="overflow-hidden rounded-xl border border-sky-500/30 bg-background">
	<header class="flex items-start justify-between gap-3 border-b bg-sky-500/[0.045] p-4">
		<div class="flex min-w-0 items-start gap-3">
			<div class="rounded-lg bg-sky-500/12 p-2 text-sky-700">
				<UsersRound class="size-4" />
			</div>
			<div class="min-w-0">
				<h3 class="font-medium">ส่งต่อคาบของ {teacherChangeItem.teacherLabel}</h3>
				<p class="text-xs leading-5 text-muted-foreground">
					{teacherChangeItem.learningGroupLabel} · เลือกครูใหม่และคาบที่จะเปลี่ยนในรุ่นตารางแบบร่าง
				</p>
			</div>
		</div>
		<Button
			type="button"
			size="icon"
			variant="ghost"
			onclick={onClose}
			aria-label="ปิดการส่งต่อคาบ"
		>
			<X class="size-4" />
		</Button>
	</header>

	<div class="space-y-4 p-4">
		<div class="space-y-2">
			<Label for="handoff-table">แบบร่างตารางที่จะปรับครู</Label>
			<Select.Root
				type="single"
				bind:value={timetableVersionId}
				onValueChange={() => {
					targetTimetableVersionRowVersion = 0;
					selectedEntryIds = [];
					selectionInitialized = false;
					preview = null;
					void refreshPreview();
				}}
			>
				<Select.Trigger id="handoff-table"
					>{timetableVersionId
						? `แบบร่าง ${drafts.find((draft) => draft.id === timetableVersionId)?.createdAt.slice(0, 10) ?? ''}`
						: 'เลือกแบบร่างตาราง'}</Select.Trigger
				>
				<Select.Content
					>{#each drafts as draft (draft.id)}<Select.Item value={draft.id}
							>แบบร่าง {new Intl.DateTimeFormat('th-TH', {
								dateStyle: 'short',
								timeStyle: 'short'
							}).format(new Date(draft.createdAt))}</Select.Item
						>{/each}</Select.Content
				>
			</Select.Root>
			{#if drafts.length === 0}<p class="text-sm text-muted-foreground">
					กดแก้ไขในหน้าตารางสอน แล้วอัปเดตข้อมูลเป็นรุ่นเปิดสอนนี้ก่อนส่งต่อคาบ
				</p>{/if}
		</div>
		<div class="grid gap-4 lg:grid-cols-2">
			<div class="space-y-2">
				<Label>วิธีจัดครูให้คาบที่เลือก</Label>
				<Select.Root type="single" value={mode} onValueChange={setMode}>
					<Select.Trigger class="w-full">{modeLabel(mode)}</Select.Trigger>
					<Select.Content>
						<Select.Item value="assign_one">ครูคนเดียวทุกคาบที่เลือก</Select.Item>
						<Select.Item value="assign_coteachers">ครูหลายคนสอนร่วมกัน</Select.Item>
						<Select.Item value="manual">จัดเองในหน้าตารางสอน</Select.Item>
					</Select.Content>
				</Select.Root>
			</div>
			{#if mode === 'assign_one'}
				<div class="space-y-2">
					<Label>ครูที่จะรับคาบ</Label>
					<DeliveryOptionCombobox
						bind:value={() => selectedInstructorIds[0] ?? '', selectOne}
						options={replacementOptions}
						placeholder="เลือกจากทีมสอนหลังเริ่มใช้"
						searchPlaceholder="ค้นหาครู..."
					/>
				</div>
			{:else if mode === 'assign_coteachers'}
				<div class="space-y-2">
					<Label>ครูที่จะสอนร่วมกัน</Label>
					<div class="max-h-40 space-y-1 overflow-y-auto rounded-lg border p-2">
						{#if replacementOptions.length === 0}
							<p class="px-2 py-3 text-xs text-muted-foreground">
								เผยแพร่ครูใหม่ในรุ่นเปิดสอนก่อน แล้วจึงเลือกส่งต่อคาบ
							</p>
						{:else}
							{#each replacementOptions as teacher (teacher.id)}
								<label
									class="flex cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 hover:bg-muted/50"
								>
									<Checkbox
										checked={selectedInstructorIds.includes(teacher.id)}
										onCheckedChange={(checked) => toggleInstructor(teacher.id, checked === true)}
									/>
									<span class="min-w-0 truncate text-sm">{teacher.label}</span>
								</label>
							{/each}
						{/if}
					</div>
				</div>
			{/if}
		</div>

		{#if mode === 'manual'}
			<div class="rounded-xl border border-amber-500/30 bg-amber-500/7 p-4 text-sm">
				<p class="font-medium text-amber-900">ระบบจะไม่เปลี่ยนครูในคาบให้อัตโนมัติ</p>
				<p class="mt-1 text-xs leading-5 text-muted-foreground">
					การเผยแพร่จะยังถูกบล็อกจนกว่าคาบที่ได้รับผลกระทบทุกคาบจะมีครูที่ใช้งานได้
				</p>
				{#if preview}
					<Button
						href={preview.timetableRoute}
						variant="outline"
						size="sm"
						class="mt-3 bg-background"
					>
						เปิดหน้าตารางสอนเพื่อจัดเอง <ExternalLink class="size-3.5" />
					</Button>
				{/if}
			</div>
		{/if}

		{#if loading}
			<div class="h-28 animate-pulse rounded-xl bg-muted"></div>
		{:else if preview}
			<div class="flex flex-wrap items-center justify-between gap-2">
				<div>
					<p class="text-sm font-medium">คาบที่ได้รับผลกระทบ</p>
					<p class="text-xs text-muted-foreground">
						เลือกแล้ว {selectedEntries.length} จาก {preview.affectedEntries.length} คาบ
					</p>
				</div>
				<Button type="button" size="sm" variant="ghost" onclick={() => refreshPreview()}>
					<RefreshCw class="size-3.5" /> ตรวจใหม่
				</Button>
			</div>

			{#if preview.affectedEntries.length === 0}
				<div
					class="rounded-xl border border-emerald-500/30 bg-emerald-500/7 p-4 text-sm text-emerald-800"
				>
					ไม่เหลือคาบที่อ้างอิงครูคนนี้ในรุ่นตารางแบบร่าง
				</div>
			{:else}
				<div class="overflow-hidden rounded-xl border">
					{#each preview.affectedEntries as entry (entry.entryId)}
						{@const proposed = preview.proposedEntries.find(
							(item) => item.entryId === entry.entryId
						)}
						<div
							class="grid gap-3 border-b p-3 last:border-b-0 md:grid-cols-[auto_1.1fr_1fr_auto_1fr] md:items-center"
						>
							<Checkbox
								checked={selectedEntryIds.includes(entry.entryId)}
								disabled={mode === 'manual'}
								onCheckedChange={(checked) => toggleEntry(entry.entryId, checked === true)}
								aria-label={`เลือก${dayLabel(entry.dayOfWeek)} ${entry.periodLabel}`}
							/>
							<div class="min-w-0">
								<p class="truncate text-sm font-medium">
									{dayLabel(entry.dayOfWeek)} · {entry.periodLabel}
								</p>
								<p class="truncate text-xs text-muted-foreground">
									{entry.offeringLabel} · {entry.roomLabel ?? 'ไม่ระบุห้อง'}
								</p>
							</div>
							<p class="truncate text-xs">
								{entry.beforeInstructors.map((teacher) => teacher.displayName).join(', ')}
							</p>
							<ArrowRight class="hidden size-4 text-sky-700 md:block" />
							<p class="truncate text-xs font-medium text-sky-800">
								{proposed
									? proposed.afterInstructors.map((teacher) => teacher.displayName).join(', ')
									: mode === 'manual'
										? 'จัดเองในตารางสอน'
										: 'ไม่เปลี่ยนคาบนี้'}
							</p>
						</div>
					{/each}
				</div>
			{/if}

			{#if preview.conflicts.length > 0}
				<div class="space-y-2 rounded-xl border border-destructive/30 bg-destructive/5 p-3">
					<div class="flex items-center gap-2 text-sm font-medium text-destructive">
						<AlertTriangle class="size-4" /> พบจุดที่ต้องแก้ก่อนส่งต่อ
					</div>
					{#each preview.conflicts as conflict, index (`${conflict.kind}:${conflict.entryIds.join(':')}:${index}`)}
						<div
							class="flex flex-wrap items-center justify-between gap-2 rounded-lg bg-background px-3 py-2"
						>
							<p class="text-xs">{conflict.message}</p>
							<Button href={conflict.timetableRoute} size="sm" variant="ghost">
								แก้ในตารางสอน <ExternalLink class="size-3" />
							</Button>
						</div>
					{/each}
				</div>
			{/if}
		{/if}

		{#if appliedCount > 0}
			<div
				class="flex items-center gap-2 rounded-xl border border-emerald-500/30 bg-emerald-500/7 px-3 py-2 text-sm text-emerald-800"
			>
				<CheckCircle2 class="size-4" /> ส่งต่อครูให้ {appliedCount} คาบแล้ว
			</div>
		{/if}
		{#if errorMessage}
			<div class="flex flex-wrap items-center justify-between gap-2" role="alert">
				<p class="text-sm text-destructive">{errorMessage}</p>
				<Button type="button" size="sm" variant="ghost" onclick={() => refreshPreview()}>
					<RefreshCw class="size-3.5" /> ตรวจใหม่
				</Button>
			</div>
		{/if}

		<div class="flex flex-wrap justify-end gap-2">
			<Button type="button" variant="outline" onclick={onClose}>ปิด</Button>
			{#if mode !== 'manual'}
				<LoadingButton
					type="button"
					loading={applying}
					loadingLabel="กำลังส่งต่อ"
					disabled={!preview?.canApply || preview.conflicts.length > 0 || appliedCount > 0}
					onclick={applyPreview}
				>
					ส่งต่อครูให้คาบที่เลือก
				</LoadingButton>
			{/if}
		</div>
	</div>
</section>
