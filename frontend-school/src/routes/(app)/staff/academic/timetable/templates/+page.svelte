<script lang="ts">
	import { page } from '$app/state';
	import { replaceState } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { onDestroy, onMount, untrack } from 'svelte';
	import { toast } from 'svelte-sonner';
	import { registerAcademicContextDirtySource } from '$lib/academic-context/store';
	import { academicContextualMenuPath } from '$lib/academic-context/route-context';
	import { selectPreferredTemplateVersion } from '$lib/academic/timetable/version-selection';
	import {
		applyTimetableTemplate,
		clearTimetable,
		createTimetableTemplateFromCurrent,
		deleteTimetableTemplate,
		listTimetableTemplates,
		listTimetableVersions,
		type TimetableTemplate,
		type TimetableVersion
	} from '$lib/api/timetable';
	import { LatestRequest, isAbortError } from '$lib/async/latest-request';
	import { PageShell } from '$lib/components/app-layout';
	import {
		LoadingButton,
		PageSkeleton,
		PageState,
		RegionUpdatingState
	} from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import { Eraser, Play, Plus, Trash2 } from '@lucide/svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const academicTermId = $derived(data.academicTermId);
	let templates = $state<TimetableTemplate[]>([]);
	let versions = $state<TimetableVersion[]>([]);
	let selectedVersion = $state.raw<TimetableVersion | null>(null);
	let versionSelectValue = $state('');
	let templatesLoading = $state(true);
	let versionsLoading = $state(true);
	let templatesError = $state('');
	let versionsError = $state('');
	const templatesRequest = new LatestRequest();
	const versionsRequest = new LatestRequest();
	let activeTermId: string | null = null;
	let templatesRevision = 0;
	let versionsRevision = 0;
	let creating = $state(false);
	let applying = $state(false);
	let clearing = $state(false);
	let deletingTemplateId = $state<string | null>(null);
	let showCreateDialog = $state(false);
	let showApplyDialog = $state(false);
	let showClearDialog = $state(false);
	let applyTarget = $state<TimetableTemplate | null>(null);
	let createName = $state('');
	let createDescription = $state('');
	let clearMode = $state<'all_except_course' | 'course_only' | 'all'>('all_except_course');
	onDestroy(() => {
		templatesRequest.abort();
		versionsRequest.abort();
	});

	const canRead = $derived($can.has(PERMISSIONS.ACADEMIC_TIMETABLE_READ_SCHOOL));
	const canManage = $derived($can.has(PERMISSIONS.ACADEMIC_TIMETABLE_MANAGE_SCHOOL));
	const canEditSelected = $derived(canManage && selectedVersion?.status === 'draft');
	const hasDirtyDraft = $derived(
		showCreateDialog && Boolean(createName.trim() || createDescription.trim())
	);

	function versionLabel(version: TimetableVersion): string {
		const status =
			version.status === 'draft'
				? 'แบบร่าง'
				: version.status === 'published'
					? 'เผยแพร่แล้ว'
					: 'ยกเลิกแล้ว';
		return `${status} · เริ่ม ${version.effectiveFrom}`;
	}

	function syncVersionUrl(versionId: string): void {
		if (page.url.searchParams.get('timetableVersionId') === versionId) return;
		const nextUrl = new URL(page.url);
		nextUrl.searchParams.set('timetableVersionId', versionId);
		replaceState(
			resolve(`/staff/academic/timetable/templates?${nextUrl.searchParams.toString()}`),
			page.state
		);
	}

	function changeVersion(versionId: string): void {
		const version = versions.find((item) => item.id === versionId);
		if (!version) return;
		selectedVersion = version;
		versionSelectValue = version.id;
		syncVersionUrl(version.id);
	}

	function applyVersions(loaded: TimetableVersion[]): void {
		versions = loaded;
		selectedVersion = selectPreferredTemplateVersion(
			loaded,
			versionSelectValue || data.requestedVersionId
		);
		versionSelectValue = selectedVersion?.id ?? '';
		if (selectedVersion) syncVersionUrl(selectedVersion.id);
	}

	async function loadTemplates(): Promise<void> {
		if (!academicTermId) return;
		const termId = academicTermId;
		const { revision, signal } = templatesRequest.begin();
		templatesRevision += 1;
		templatesLoading = true;
		templatesError = '';
		try {
			const loaded = await listTimetableTemplates({ signal });
			if (templatesRequest.isCurrent(revision) && academicTermId === termId) templates = loaded;
		} catch (error) {
			if (!isAbortError(error) && templatesRequest.isCurrent(revision))
				templatesError = error instanceof Error ? error.message : 'โหลดแม่แบบตารางสอนไม่สำเร็จ';
		} finally {
			if (templatesRequest.isCurrent(revision)) templatesLoading = false;
		}
	}

	async function loadVersions(): Promise<void> {
		if (!academicTermId) return;
		const termId = academicTermId;
		const { revision, signal } = versionsRequest.begin();
		versionsRevision += 1;
		versionsLoading = true;
		versionsError = '';
		try {
			const loaded = await listTimetableVersions(termId, { signal });
			if (versionsRequest.isCurrent(revision) && academicTermId === termId) applyVersions(loaded);
		} catch (error) {
			if (!isAbortError(error) && versionsRequest.isCurrent(revision))
				versionsError = error instanceof Error ? error.message : 'โหลดรุ่นตารางสอนไม่สำเร็จ';
		} finally {
			if (versionsRequest.isCurrent(revision)) versionsLoading = false;
		}
	}

	async function handleCreate(): Promise<void> {
		if (!academicTermId || !selectedVersion || !createName.trim()) return;
		creating = true;
		try {
			const created = await createTimetableTemplateFromCurrent({
				academicTermId,
				timetableVersionId: selectedVersion.id,
				name: createName.trim(),
				description: createDescription.trim() || null,
				entryTypes: null
			});
			templatesRevision += 1;
			templatesLoading = false;
			templates = [created, ...templates.filter((template) => template.id !== created.id)];
			showCreateDialog = false;
			createName = '';
			createDescription = '';
			toast.success('สร้างแม่แบบจากตารางของภาคเรียนนี้แล้ว');
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'สร้างแม่แบบไม่สำเร็จ');
		} finally {
			creating = false;
		}
	}

	async function handleDelete(template: TimetableTemplate): Promise<void> {
		deletingTemplateId = template.id;
		try {
			await deleteTimetableTemplate(template.id);
			templatesRevision += 1;
			templatesLoading = false;
			templates = templates.filter((item) => item.id !== template.id);
			toast.success('ลบแม่แบบแล้ว');
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ลบแม่แบบไม่สำเร็จ');
		} finally {
			deletingTemplateId = null;
		}
	}

	function openApply(template: TimetableTemplate): void {
		applyTarget = template;
		showApplyDialog = true;
	}

	async function handleApply(): Promise<void> {
		if (!academicTermId || !selectedVersion || !canEditSelected || !applyTarget) return;
		applying = true;
		try {
			const result = await applyTimetableTemplate(applyTarget.id, {
				academicTermId,
				timetableVersionId: selectedVersion.id
			});
			showApplyDialog = false;
			applyTarget = null;
			toast.success(`นำแม่แบบไปใช้แล้ว ${result.applied} คาบ`);
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'นำแม่แบบไปใช้ไม่สำเร็จ');
		} finally {
			applying = false;
		}
	}

	async function handleClear(): Promise<void> {
		if (!academicTermId || !selectedVersion || !canEditSelected) return;
		const entryTypes =
			clearMode === 'all'
				? ['BREAK', 'HOMEROOM', 'ACTIVITY', 'ACADEMIC', 'COURSE']
				: clearMode === 'course_only'
					? ['COURSE']
					: ['BREAK', 'HOMEROOM', 'ACTIVITY', 'ACADEMIC'];
		clearing = true;
		try {
			const removed = await clearTimetable({
				academicTermId,
				timetableVersionId: selectedVersion.id,
				entryTypes
			});
			showClearDialog = false;
			toast.success(`ล้างออกจากตารางแล้ว ${removed.length} คาบ`);
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ล้างตารางไม่สำเร็จ');
		} finally {
			clearing = false;
		}
	}

	function formatDate(value: string): string {
		return new Date(value).toLocaleString('th-TH', { dateStyle: 'short', timeStyle: 'short' });
	}

	$effect.pre(() => {
		const termId = data.academicTermId;
		const routeTemplates = data.templates;
		const routeVersions = data.versions;
		const initialTemplatesRevision = templatesRevision;
		const initialVersionsRevision = versionsRevision;
		let current = true;
		untrack(() => {
			if (activeTermId !== termId) {
				activeTermId = termId;
				templates = [];
				versions = [];
				selectedVersion = null;
				versionSelectValue = '';
				showCreateDialog = false;
				showApplyDialog = false;
				showClearDialog = false;
				applyTarget = null;
			}
			templatesRequest.abort();
			versionsRequest.abort();
			templatesLoading = Boolean(routeTemplates);
			versionsLoading = Boolean(routeVersions);
			templatesError = '';
			versionsError = '';
		});
		if (routeTemplates) {
			void routeTemplates.then((result) => {
				if (!current) return;
				untrack(() => {
					if (templatesRevision === initialTemplatesRevision) {
						if (result.ok) templates = result.data;
						else templatesError = result.error;
						templatesLoading = false;
					}
				});
			});
		}
		if (routeVersions) {
			void routeVersions.then((result) => {
				if (!current) return;
				untrack(() => {
					if (versionsRevision === initialVersionsRevision) {
						if (result.ok) applyVersions(result.data);
						else versionsError = result.error;
						versionsLoading = false;
					}
				});
			});
		}
		return () => {
			current = false;
		};
	});

	onMount(() =>
		registerAcademicContextDirtySource('timetable-template-draft', () => hasDirtyDraft)
	);
</script>

<PageShell
	title="แม่แบบตารางสอน"
	description="เก็บรูปแบบตารางไว้ใช้ซ้ำ แล้วนำไปใช้กับภาคเรียนที่เลือกบนแถบด้านบน"
	backHref={academicContextualMenuPath(
		'/staff/academic/timetable',
		{ academicYearId: data.academicYearId, academicTermId },
		null
	)}
	backPreload="tap"
>
	{#snippet actions()}
		{#if canManage && academicTermId && selectedVersion}
			<div class="flex flex-wrap gap-2">
				<Button
					variant="outline"
					disabled={!canEditSelected}
					onclick={() => (showClearDialog = true)}><Eraser /> ล้างตาราง</Button
				>
				<Button onclick={() => (showCreateDialog = true)}><Plus /> สร้างจากภาคนี้</Button>
			</div>
		{/if}
	{/snippet}

	{#if !canRead}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ดูแม่แบบ"
			description="ต้องมีสิทธิ์อ่านชุดการเรียนระดับโรงเรียน"
		/>
	{:else if !academicTermId}
		<PageState
			variant="empty"
			title="เลือกภาคเรียนก่อน"
			description="แม่แบบไม่ผูกกับภาค แต่การสร้าง นำไปใช้ และล้างตารางต้องมีภาคเรียนเป้าหมายที่ชัดเจน"
		/>
	{:else}
		<div class="space-y-4">
			{#if versionsLoading && versions.length === 0}
				<PageSkeleton variant="cards" rows={1} />
			{:else if versionsError && versions.length === 0}
				<PageState
					variant="error"
					title="โหลดรุ่นตารางสอนไม่สำเร็จ"
					description={versionsError}
					actionLabel="ลองอีกครั้ง"
					onaction={loadVersions}
				/>
			{:else if versions.length === 0}
				<PageState
					variant="empty"
					title="ยังไม่มีรุ่นตารางสอน"
					description="สร้างรุ่นตารางสอนของภาคเรียนนี้ก่อนใช้แม่แบบ"
				/>
			{:else}
				<div
					class="relative"
					aria-busy={versionsLoading}
					data-testid="timetable-template-versions-ready"
				>
					{#if versionsLoading}<RegionUpdatingState />{/if}
					{#if versionsError}
						<div
							role="alert"
							class="mb-3 flex flex-wrap items-center gap-3 rounded-lg border border-destructive/40 p-3 text-sm"
						>
							<span>{versionsError}</span>
							<Button variant="outline" size="sm" onclick={loadVersions}>ลองอีกครั้ง</Button>
						</div>
					{/if}
					<Card.Root class="gap-0 py-0">
						<Card.Content class="grid gap-3 p-4 sm:grid-cols-[1fr_20rem] sm:items-center">
							<div>
								<p class="font-medium">รุ่นตารางเป้าหมาย</p>
								<p class="mt-1 text-xs text-muted-foreground">
									{selectedVersion?.status === 'draft'
										? 'แบบร่างนี้รับการนำแม่แบบไปใช้และล้างตารางได้'
										: 'รุ่นที่เผยแพร่แล้วใช้สร้างแม่แบบได้ แต่แก้ไขตารางไม่ได้'}
								</p>
							</div>
							<Select.Root
								type="single"
								bind:value={versionSelectValue}
								onValueChange={changeVersion}
							>
								<Select.Trigger class="w-full" aria-label="เลือกรุ่นตารางเป้าหมาย">
									{selectedVersion ? versionLabel(selectedVersion) : 'เลือกรุ่นตาราง'}
								</Select.Trigger>
								<Select.Content>
									{#each versions as version (version.id)}
										<Select.Item value={version.id}>{versionLabel(version)}</Select.Item>
									{/each}
								</Select.Content>
							</Select.Root>
						</Card.Content>
					</Card.Root>
				</div>
			{/if}
			{#if templatesLoading && templates.length === 0}
				<PageSkeleton variant="cards" rows={3} />
			{:else if templatesError && templates.length === 0}
				<PageState
					variant="error"
					title="โหลดแม่แบบไม่สำเร็จ"
					description={templatesError}
					actionLabel="ลองอีกครั้ง"
					onaction={loadTemplates}
				/>
			{:else if templates.length === 0}
				<PageState
					title="ยังไม่มีแม่แบบ"
					description="สร้างแม่แบบจากตารางของภาคเรียนที่เลือกเพื่อใช้เป็นจุดเริ่มต้นในภาคถัดไป"
					actionLabel={canManage && selectedVersion ? 'สร้างจากภาคนี้' : undefined}
					onaction={canManage && selectedVersion ? () => (showCreateDialog = true) : undefined}
				/>
			{:else}
				<div class="relative" aria-busy={templatesLoading} data-testid="timetable-templates-ready">
					{#if templatesLoading}<RegionUpdatingState />{/if}
					{#if templatesError}
						<div
							role="alert"
							class="mb-3 flex flex-wrap items-center gap-3 rounded-lg border border-destructive/40 p-3 text-sm"
						>
							<span>{templatesError}</span>
							<Button variant="outline" size="sm" onclick={loadTemplates}>ลองอีกครั้ง</Button>
						</div>
					{/if}
					<div class="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
						{#each templates as template (template.id)}
							<Card.Root>
								<Card.Header>
									<Card.Title>{template.name}</Card.Title>
									<Card.Description>{template.description ?? 'ไม่มีคำอธิบาย'}</Card.Description>
								</Card.Header>
								<Card.Content>
									<p class="text-muted-foreground text-xs">
										สร้างเมื่อ {formatDate(template.createdAt)}
									</p>
								</Card.Content>
								{#if canManage}
									<Card.Footer class="gap-2">
										<Button
											class="flex-1"
											disabled={!canEditSelected}
											onclick={() => openApply(template)}><Play /> ใช้กับภาคนี้</Button
										>
										<LoadingButton
											variant="ghost"
											size="icon"
											loading={deletingTemplateId === template.id}
											loadingLabel=""
											aria-label={`ลบ ${template.name}`}
											onclick={() => handleDelete(template)}
											><Trash2 class="text-destructive" /></LoadingButton
										>
									</Card.Footer>
								{/if}
							</Card.Root>
						{/each}
					</div>
				</div>
			{/if}
		</div>
	{/if}
</PageShell>

<Dialog.Root bind:open={showCreateDialog}>
	<Dialog.Content>
		<Dialog.Header>
			<Dialog.Title>สร้างแม่แบบจากภาคเรียนนี้</Dialog.Title>
			<Dialog.Description
				>ระบบจะบันทึกตำแหน่งตามลำดับคาบและ stable resource เพื่อปรับใช้กับภาคอื่นได้</Dialog.Description
			>
		</Dialog.Header>
		<div class="space-y-4 py-2">
			<div class="space-y-2">
				<Label for="template-name">ชื่อแม่แบบ</Label><Input
					id="template-name"
					bind:value={createName}
					placeholder="เช่น ตารางพื้นฐาน ม.ต้น"
				/>
			</div>
			<div class="space-y-2">
				<Label for="template-description">คำอธิบาย</Label><Input
					id="template-description"
					bind:value={createDescription}
				/>
			</div>
		</div>
		<Dialog.Footer>
			<Button variant="outline" onclick={() => (showCreateDialog = false)}>ยกเลิก</Button>
			<LoadingButton
				loading={creating}
				loadingLabel="กำลังสร้าง"
				disabled={!createName.trim()}
				onclick={handleCreate}>สร้างแม่แบบ</LoadingButton
			>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<Dialog.Root bind:open={showApplyDialog}>
	<Dialog.Content>
		<Dialog.Header
			><Dialog.Title>ใช้แม่แบบ “{applyTarget?.name}”</Dialog.Title><Dialog.Description
				>รายการจะถูกจับคู่กับตารางเวลา กลุ่มเรียน และทรัพยากรของภาคเรียนที่เลือก
				หากจับคู่ไม่ได้ระบบจะหยุดโดยไม่สร้าง compatibility record</Dialog.Description
			></Dialog.Header
		>
		<Dialog.Footer
			><Button variant="outline" onclick={() => (showApplyDialog = false)}>ยกเลิก</Button
			><LoadingButton loading={applying} loadingLabel="กำลังนำไปใช้" onclick={handleApply}
				>ยืนยัน</LoadingButton
			></Dialog.Footer
		>
	</Dialog.Content>
</Dialog.Root>

<Dialog.Root bind:open={showClearDialog}>
	<Dialog.Content>
		<Dialog.Header
			><Dialog.Title>ล้างตารางของภาคเรียนนี้</Dialog.Title><Dialog.Description
				>เลือกชนิดคาบที่จะปิดใช้งาน การทำงานนี้ไม่ลบชุดการเรียนหรือรายชื่อนักเรียน</Dialog.Description
			></Dialog.Header
		>
		<div class="space-y-2 py-2">
			<Label for="clear-mode">ขอบเขต</Label>
			<Select.Root type="single" bind:value={clearMode}>
				<Select.Trigger id="clear-mode" class="w-full">
					{clearMode === 'all_except_course'
						? 'คาบทั่วไปและกิจกรรม (เก็บรายวิชา)'
						: clearMode === 'course_only'
							? 'เฉพาะรายวิชา'
							: 'ทุกคาบ'}
				</Select.Trigger>
				<Select.Content>
					<Select.Item value="all_except_course">คาบทั่วไปและกิจกรรม (เก็บรายวิชา)</Select.Item>
					<Select.Item value="course_only">เฉพาะรายวิชา</Select.Item>
					<Select.Item value="all">ทุกคาบ</Select.Item>
				</Select.Content>
			</Select.Root>
		</div>
		<Dialog.Footer
			><Button variant="outline" onclick={() => (showClearDialog = false)}>ยกเลิก</Button
			><LoadingButton
				variant="destructive"
				loading={clearing}
				loadingLabel="กำลังล้าง"
				onclick={handleClear}>ล้างตามขอบเขต</LoadingButton
			></Dialog.Footer
		>
	</Dialog.Content>
</Dialog.Root>
