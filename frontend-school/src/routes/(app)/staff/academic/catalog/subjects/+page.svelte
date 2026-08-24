<script lang="ts">
	import { onMount } from 'svelte';
	import {
		createCatalogSubject,
		createSubjectVersion,
		listCatalogSubjects,
		listSubjectVersions,
		publishSubjectVersion,
		type CatalogSubject,
		type SubjectVersion
	} from '$lib/api/academic-core';
	import CatalogVersionHistory from '$lib/components/academic-core/CatalogVersionHistory.svelte';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import { Plus } from 'lucide-svelte';

	type VersionDraft = {
		name: string;
		secondaryName: string;
		exactValue: string;
		effectiveFrom: string;
		effectiveUntil: string;
		gradeLevelIds: string[];
		classification: string;
	};
	let subjects = $state<CatalogSubject[]>([]);
	let selected = $state<CatalogSubject | null>(null);
	let versions = $state<SubjectVersion[]>([]);
	let loading = $state(true);
	let errorMessage = $state('');
	let code = $state('');
	const canManage = $derived(
		$can.hasAny(
			PERMISSIONS.ACADEMIC_CATALOG_MANAGE_SCHOOL,
			PERMISSIONS.ACADEMIC_CATALOG_MANAGE_ORGANIZATION_TREE,
			PERMISSIONS.ACADEMIC_CATALOG_MANAGE_ORGANIZATION_UNIT
		)
	);

	async function loadWorkspace() {
		loading = true;
		errorMessage = '';
		try {
			subjects = await listCatalogSubjects();
			if (subjects[0]) await selectSubject(subjects[0]);
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'โหลดทะเบียนรายวิชาไม่สำเร็จ';
		} finally {
			loading = false;
		}
	}
	async function selectSubject(subject: CatalogSubject) {
		selected = subject;
		versions = await listSubjectVersions(subject.id);
	}
	async function addSubject(event: SubmitEvent) {
		event.preventDefault();
		const created = await createCatalogSubject({ code, owningOrganizationUnitId: null });
		subjects = [...subjects, created];
		code = '';
		await selectSubject(created);
	}
	async function addVersion(draft: VersionDraft) {
		if (!selected) return;
		const created = await createSubjectVersion(selected.id, {
			nameTh: draft.name,
			nameEn: draft.secondaryName || null,
			credit: draft.exactValue,
			description: null,
			effectiveFrom: draft.effectiveFrom,
			effectiveUntil: draft.effectiveUntil || null,
			gradeLevelIds: draft.gradeLevelIds,
			groupId: null,
			hoursPerSemester: null,
			periodsPerWeek: null,
			subjectType: draft.classification,
			termCode: null
		});
		versions = [...versions, created];
	}
	async function publish(id: string, rowVersion: number) {
		const updated = await publishSubjectVersion(id, { rowVersion });
		versions = versions.map((item) => (item.id === id ? updated : item));
	}
	onMount(loadWorkspace);
</script>

<PageShell
	title="ทะเบียนรายวิชา"
	description="แยกรหัสรายวิชาคงที่ออกจากรายละเอียดแต่ละรุ่น เพื่อรักษาประวัติที่เคยใช้งาน"
>
	{#if loading}<PageSkeleton
			variant="cards"
			rows={4}
		/>{:else if errorMessage && subjects.length === 0}<PageState
			variant="error"
			title="โหลดทะเบียนไม่สำเร็จ"
			description={errorMessage}
			actionLabel="ลองอีกครั้ง"
			onaction={loadWorkspace}
		/>{:else}
		<div class="grid gap-5 xl:grid-cols-[260px_minmax(0,1fr)]">
			<aside class="space-y-3 rounded-xl border bg-card p-4">
				<h2 class="font-semibold">รหัสรายวิชา</h2>
				{#each subjects as subject (subject.id)}<button
						class:border-primary={selected?.id === subject.id}
						class="w-full rounded-lg border px-3 py-2 text-left text-sm hover:bg-muted"
						onclick={() => selectSubject(subject)}
						><span class="font-medium">{subject.code}</span><span
							class="block text-xs text-muted-foreground">ข้อมูลคงที่</span
						></button
					>{/each}{#if canManage}<form class="flex gap-2 border-t pt-3" onsubmit={addSubject}>
						<div class="min-w-0 flex-1">
							<Label class="sr-only" for="new-subject-code">รหัสใหม่</Label><Input
								id="new-subject-code"
								bind:value={code}
								placeholder="รหัสใหม่"
								required
							/>
						</div>
						<Button size="icon" type="submit" aria-label="เพิ่มรหัสรายวิชา"
							><Plus class="size-4" /></Button
						>
					</form>{/if}
			</aside>
			{#if selected}<CatalogVersionHistory
					kind="subject"
					code={selected.code}
					items={versions.map((item) => ({
						id: item.id,
						versionNo: item.versionNo,
						name: item.nameTh,
						secondaryName: item.nameEn,
						exactValue: `${item.credit} หน่วยกิต`,
						effectiveFrom: item.effectiveFrom,
						effectiveUntil: item.effectiveUntil,
						status: item.status,
						rowVersion: item.rowVersion
					}))}
					{canManage}
					onCreate={addVersion}
					onPublish={publish}
				/>{:else}<div
					class="rounded-xl border border-dashed p-10 text-center text-sm text-muted-foreground"
				>
					เลือกรหัสรายวิชาเพื่อดูประวัติรุ่น
				</div>{/if}
		</div>
		{#if errorMessage}<p role="alert" class="text-sm text-destructive">{errorMessage}</p>{/if}
	{/if}
</PageShell>
