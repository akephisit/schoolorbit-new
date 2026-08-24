<script lang="ts">
	import { onMount } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import {
		createCurriculum,
		createCurriculumVersion,
		createStudyProgram,
		getStudyProgram,
		listCurricula,
		listCurriculumVersions,
		listProgramRequirements,
		listStudyPrograms,
		publishCurriculumVersion,
		replaceProgramRequirements,
		type Curriculum,
		type CurriculumVersion,
		type ProgramRequirement,
		type StudyProgram
	} from '$lib/api/academic-core';
	import CurriculumProgramEditor from '$lib/components/academic-core/CurriculumProgramEditor.svelte';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import { GitBranchPlus, Plus } from 'lucide-svelte';

	let curricula = $state<Curriculum[]>([]);
	let selectedCurriculum = $state<Curriculum | null>(null);
	let versions = $state<CurriculumVersion[]>([]);
	let selectedVersion = $state<CurriculumVersion | null>(null);
	let programs = $state<StudyProgram[]>([]);
	const requirementsByProgram = new SvelteMap<string, ProgramRequirement[]>();
	let loading = $state(true);
	let errorMessage = $state('');
	let curriculumDraft = $state({ code: '', nameTh: '', gradeLevelIds: '' });
	let versionDraft = $state({ versionName: '', startAcademicYearId: '', endAcademicYearId: '' });
	const canManage = $derived(
		$can.hasAny(
			PERMISSIONS.ACADEMIC_CURRICULUM_MANAGE_SCHOOL,
			PERMISSIONS.ACADEMIC_CURRICULUM_MANAGE_ORGANIZATION_TREE,
			PERMISSIONS.ACADEMIC_CURRICULUM_MANAGE_ORGANIZATION_UNIT
		)
	);

	async function loadWorkspace() {
		loading = true;
		errorMessage = '';
		try {
			curricula = await listCurricula();
			if (curricula[0]) await selectCurriculum(curricula[0]);
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'โหลดหลักสูตรไม่สำเร็จ';
		} finally {
			loading = false;
		}
	}
	async function selectCurriculum(curriculum: Curriculum) {
		selectedCurriculum = curriculum;
		versions = await listCurriculumVersions(curriculum.id);
		if (versions[0]) await selectVersion(versions[0]);
		else {
			selectedVersion = null;
			programs = [];
			requirementsByProgram.clear();
		}
	}
	async function selectVersion(version: CurriculumVersion) {
		selectedVersion = version;
		programs = await listStudyPrograms(version.id);
		const next = new SvelteMap<string, ProgramRequirement[]>();
		for (const program of programs) next.set(program.id, await listProgramRequirements(program.id));
		requirementsByProgram.clear();
		for (const [programId, requirements] of next)
			requirementsByProgram.set(programId, requirements);
	}
	async function addCurriculum(event: SubmitEvent) {
		event.preventDefault();
		const created = await createCurriculum({
			code: curriculumDraft.code,
			nameTh: curriculumDraft.nameTh,
			nameEn: null,
			description: null,
			gradeLevelIds: curriculumDraft.gradeLevelIds
				.split(',')
				.map((id) => id.trim())
				.filter(Boolean),
			owningOrganizationUnitId: null
		});
		curricula = [...curricula, created];
		curriculumDraft = { code: '', nameTh: '', gradeLevelIds: '' };
		await selectCurriculum(created);
	}
	async function addVersion(event: SubmitEvent) {
		event.preventDefault();
		if (!selectedCurriculum) return;
		const created = await createCurriculumVersion(selectedCurriculum.id, {
			versionName: versionDraft.versionName,
			startAcademicYearId: versionDraft.startAcademicYearId,
			endAcademicYearId: versionDraft.endAcademicYearId || null,
			description: null
		});
		versions = [created, ...versions];
		versionDraft = { versionName: '', startAcademicYearId: '', endAcademicYearId: '' };
		await selectVersion(created);
	}
	async function addProgram(draft: {
		code: string;
		nameTh: string;
		nameEn: string;
		isDefault: boolean;
	}) {
		if (!selectedVersion) return;
		const created = await createStudyProgram(selectedVersion.id, {
			...draft,
			nameEn: draft.nameEn || null,
			owningOrganizationUnitId: null
		});
		programs = [...programs, created];
		requirementsByProgram.set(created.id, []);
	}
	async function addRequirement(
		program: StudyProgram,
		draft: {
			catalogVersionId: string;
			gradeLevelId: string;
			resourceKind: 'course' | 'activity';
			requirementKind: 'required' | 'elective' | 'optional';
			credit: string;
			hours: string;
			recommendedTermCode: string;
		}
	) {
		const existing = requirementsByProgram.get(program.id) ?? [];
		const requirements = existing.map((item) => ({
			catalogVersionId: item.catalogVersionId,
			gradeLevelId: item.gradeLevelId,
			resourceKind: item.resourceKind,
			requirementKind: item.requirementKind,
			credit: item.credit ?? null,
			hours: item.hours ?? null,
			recommendedTermCode: item.recommendedTermCode ?? null,
			displayOrder: item.displayOrder
		}));
		requirements.push({
			catalogVersionId: draft.catalogVersionId,
			gradeLevelId: draft.gradeLevelId,
			resourceKind: draft.resourceKind,
			requirementKind: draft.requirementKind,
			credit: draft.credit || null,
			hours: draft.hours || null,
			recommendedTermCode: draft.recommendedTermCode || null,
			displayOrder: requirements.length + 1
		});
		const updated = await replaceProgramRequirements(program.id, {
			rowVersion: program.rowVersion,
			requirements
		});
		requirementsByProgram.set(program.id, updated);
		const refreshedProgram = await getStudyProgram(program.id);
		programs = programs.map((item) => (item.id === program.id ? refreshedProgram : item));
	}
	async function publishVersion(id: string, rowVersion: number) {
		const updated = await publishCurriculumVersion(id, { rowVersion });
		versions = versions.map((version) => (version.id === id ? updated : version));
		selectedVersion = updated;
	}
	onMount(loadWorkspace);
</script>

<PageShell
	title="หลักสูตรและแผนการเรียน"
	description="จัดโครงสร้างหลักสูตรเป็นรุ่น แผนการเรียน และข้อกำหนดที่อ้างรุ่นวิชา/กิจกรรมแบบชัดเจน"
>
	{#if loading}<PageSkeleton
			variant="cards"
			rows={5}
		/>{:else if errorMessage && curricula.length === 0}<PageState
			variant="error"
			title="โหลดหลักสูตรไม่สำเร็จ"
			description={errorMessage}
			actionLabel="ลองอีกครั้ง"
			onaction={loadWorkspace}
		/>{:else}
		<div class="grid gap-5 xl:grid-cols-[280px_minmax(0,1fr)]">
			<aside class="space-y-4 rounded-xl border bg-card p-4">
				<div>
					<h2 class="font-semibold">หลักสูตร</h2>
					<p class="text-xs text-muted-foreground">ตัวตนคงที่</p>
				</div>
				{#each curricula as curriculum (curriculum.id)}<button
						class:border-primary={selectedCurriculum?.id === curriculum.id}
						class="w-full rounded-lg border px-3 py-2 text-left"
						onclick={() => selectCurriculum(curriculum)}
						><span class="text-sm font-medium">{curriculum.nameTh}</span><span
							class="block text-xs text-muted-foreground">{curriculum.code}</span
						></button
					>{/each}{#if canManage}<form class="space-y-2 border-t pt-3" onsubmit={addCurriculum}>
						<Label class="sr-only" for="curriculum-code">รหัสหลักสูตร</Label><Input
							id="curriculum-code"
							bind:value={curriculumDraft.code}
							placeholder="รหัสหลักสูตร"
							required
						/><Label class="sr-only" for="curriculum-name">ชื่อหลักสูตร</Label><Input
							id="curriculum-name"
							bind:value={curriculumDraft.nameTh}
							placeholder="ชื่อหลักสูตร"
							required
						/><Label class="sr-only" for="curriculum-levels">รหัสระดับชั้น</Label><Input
							id="curriculum-levels"
							bind:value={curriculumDraft.gradeLevelIds}
							placeholder="รหัสระดับชั้น, ..."
							required
						/><Button class="w-full" type="submit"><Plus class="size-4" /> เพิ่มหลักสูตร</Button>
					</form>{/if}
			</aside>
			<div class="space-y-5">
				{#if selectedCurriculum}<section class="rounded-xl border bg-card p-4">
						<div class="flex flex-wrap items-center justify-between gap-3">
							<div>
								<h2 class="font-semibold">รุ่นของ {selectedCurriculum.nameTh}</h2>
								<p class="text-xs text-muted-foreground">เลือกรุ่นเพื่อดูแผนและข้อกำหนด</p>
							</div>
							<div class="flex flex-wrap gap-2">
								{#each versions as version (version.id)}<Button
										size="sm"
										variant={selectedVersion?.id === version.id ? 'default' : 'outline'}
										onclick={() => selectVersion(version)}
										>{version.versionName} · {version.status}</Button
									>{/each}
							</div>
						</div>
						{#if canManage}<form
								class="mt-4 grid gap-2 border-t pt-4 md:grid-cols-[1fr_1fr_1fr_auto]"
								onsubmit={addVersion}
							>
								<Input
									aria-label="ชื่อรุ่นหลักสูตร"
									bind:value={versionDraft.versionName}
									placeholder="ชื่อรุ่น"
									required
								/><Input
									aria-label="ปีเริ่มใช้"
									bind:value={versionDraft.startAcademicYearId}
									placeholder="รหัสปีเริ่ม"
									required
								/><Input
									aria-label="ปีสิ้นสุด"
									bind:value={versionDraft.endAcademicYearId}
									placeholder="รหัสปีสิ้นสุด (ถ้ามี)"
								/><Button type="submit" variant="outline"
									><GitBranchPlus class="size-4" /> สร้างรุ่น</Button
								>
							</form>{/if}
					</section>
					{#if selectedVersion}<CurriculumProgramEditor
							version={selectedVersion}
							{programs}
							{requirementsByProgram}
							{canManage}
							onCreateProgram={addProgram}
							onAddRequirement={addRequirement}
							onPublishVersion={publishVersion}
						/>{:else}<div
							class="rounded-xl border border-dashed p-10 text-center text-sm text-muted-foreground"
						>
							สร้างหรือเลือกรุ่นหลักสูตร
						</div>{/if}{/if}
			</div>
		</div>
		{#if errorMessage}<p role="alert" class="text-sm text-destructive">{errorMessage}</p>{/if}
	{/if}
</PageShell>
