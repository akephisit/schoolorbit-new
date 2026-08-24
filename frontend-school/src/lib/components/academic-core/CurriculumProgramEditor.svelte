<script lang="ts">
	import type { CurriculumVersion, ProgramRequirement, StudyProgram } from '$lib/api/academic-core';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { BookCheck, CheckCircle2, Layers, Plus } from 'lucide-svelte';

	let {
		version,
		programs,
		requirementsByProgram,
		canManage = false,
		onCreateProgram,
		onAddRequirement,
		onPublishVersion
	}: {
		version: CurriculumVersion;
		programs: StudyProgram[];
		requirementsByProgram: Map<string, ProgramRequirement[]>;
		canManage?: boolean;
		onCreateProgram: (draft: {
			code: string;
			nameTh: string;
			nameEn: string;
			isDefault: boolean;
		}) => Promise<void>;
		onAddRequirement: (
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
		) => Promise<void>;
		onPublishVersion: (id: string, rowVersion: number) => Promise<void>;
	} = $props();

	let draft = $state({ code: '', nameTh: '', nameEn: '', isDefault: false });
	let busy = $state(false);
	let errorMessage = $state('');
	let requirementProgramId = $state('');
	let requirementDraft = $state({
		catalogVersionId: '',
		gradeLevelId: '',
		resourceKind: 'course' as 'course' | 'activity',
		requirementKind: 'required' as 'required' | 'elective' | 'optional',
		credit: '',
		hours: '',
		recommendedTermCode: ''
	});

	async function createProgram(event: SubmitEvent) {
		event.preventDefault();
		busy = true;
		errorMessage = '';
		try {
			await onCreateProgram(draft);
			draft = { code: '', nameTh: '', nameEn: '', isDefault: false };
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'สร้างแผนการเรียนไม่สำเร็จ';
		} finally {
			busy = false;
		}
	}

	async function publish() {
		busy = true;
		errorMessage = '';
		try {
			await onPublishVersion(version.id, version.rowVersion);
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'เผยแพร่รุ่นหลักสูตรไม่สำเร็จ';
		} finally {
			busy = false;
		}
	}

	async function addRequirement(event: SubmitEvent) {
		event.preventDefault();
		const program = programs.find((item) => item.id === requirementProgramId);
		if (!program) return;
		busy = true;
		errorMessage = '';
		try {
			await onAddRequirement(program, requirementDraft);
			requirementDraft = {
				...requirementDraft,
				catalogVersionId: '',
				credit: '',
				hours: '',
				recommendedTermCode: ''
			};
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'เพิ่มข้อกำหนดไม่สำเร็จ';
		} finally {
			busy = false;
		}
	}
</script>

<section class="overflow-hidden rounded-xl border bg-card">
	<header class="flex flex-wrap items-center justify-between gap-3 border-b bg-muted/30 px-5 py-4">
		<div class="flex items-center gap-3">
			<div class="rounded-lg bg-primary/10 p-2 text-primary"><BookCheck class="size-5" /></div>
			<div>
				<div class="flex items-center gap-2">
					<h2 class="font-semibold">{version.versionName}</h2>
					<Badge variant={version.status === 'published' ? 'default' : 'secondary'}
						>{version.status}</Badge
					>
				</div>
				<p class="text-xs text-muted-foreground">ปีเริ่มใช้ {version.startAcademicYearId}</p>
			</div>
		</div>
		{#if canManage && version.status === 'draft'}<Button
				size="sm"
				disabled={busy || programs.length === 0}
				onclick={publish}><CheckCircle2 class="size-4" /> ตรวจสรุปและเผยแพร่</Button
			>{/if}
	</header>

	<div class="grid gap-5 p-5 lg:grid-cols-[minmax(0,1fr)_300px]">
		<div class="space-y-3">
			{#each programs as program (program.id)}
				<article class="rounded-lg border">
					<header class="flex items-center justify-between border-b px-4 py-3">
						<div>
							<h3 class="font-medium">{program.nameTh}</h3>
							<p class="text-xs text-muted-foreground">
								{program.code}{program.nameEn ? ` · ${program.nameEn}` : ''}
							</p>
						</div>
						{#if program.isDefault}<Badge>แผนเริ่มต้น</Badge>{/if}
					</header>
					<div class="divide-y">
						{#each requirementsByProgram.get(program.id) ?? [] as requirement (requirement.id)}
							<div class="grid gap-2 px-4 py-3 text-sm sm:grid-cols-[1fr_auto]">
								<div class="flex items-center gap-2">
									<Layers class="size-4 text-muted-foreground" /><span
										>{requirement.resourceKind} · {requirement.catalogVersionId}</span
									>
								</div>
								<span class="font-medium tabular-nums"
									>{requirement.credit ?? requirement.hours ?? '—'}</span
								>
							</div>
						{:else}<p class="px-4 py-5 text-sm text-muted-foreground">
								ยังไม่มีข้อกำหนดในแผนนี้
							</p>{/each}
					</div>
				</article>
			{:else}<div
					class="rounded-lg border border-dashed p-8 text-center text-sm text-muted-foreground"
				>
					เพิ่มแผนการเรียนอย่างน้อยหนึ่งแผนก่อนเผยแพร่
				</div>{/each}
		</div>

		{#if canManage && version.status === 'draft'}
			<div class="space-y-4">
				<form class="space-y-3 rounded-lg border bg-muted/20 p-4" onsubmit={createProgram}>
					<h3 class="font-medium">เพิ่มแผนการเรียน</h3>
					<div class="space-y-1.5">
						<Label for={`program-code-${version.id}`}>รหัสแผน</Label><Input
							id={`program-code-${version.id}`}
							bind:value={draft.code}
							required
						/>
					</div>
					<div class="space-y-1.5">
						<Label for={`program-name-${version.id}`}>ชื่อแผน</Label><Input
							id={`program-name-${version.id}`}
							bind:value={draft.nameTh}
							required
						/>
					</div>
					<div class="space-y-1.5">
						<Label for={`program-name-en-${version.id}`}>ชื่อภาษาอังกฤษ</Label><Input
							id={`program-name-en-${version.id}`}
							bind:value={draft.nameEn}
						/>
					</div>
					<label class="flex items-center gap-2 text-sm"
						><input type="checkbox" bind:checked={draft.isDefault} /> ใช้เป็นแผนเริ่มต้น</label
					>
					<Button type="submit" variant="outline" class="w-full" disabled={busy}
						><Plus class="size-4" /> เพิ่มแผน</Button
					>
				</form>
				<form class="space-y-3 rounded-lg border bg-muted/20 p-4" onsubmit={addRequirement}>
					<h3 class="font-medium">เพิ่มข้อกำหนด</h3>
					<label class="space-y-1.5 text-sm"
						><span class="font-medium">แผนการเรียน</span><select
							class="h-10 w-full rounded-md border bg-background px-3"
							bind:value={requirementProgramId}
							required
							><option value="">เลือกแผน</option>{#each programs as program (program.id)}<option
									value={program.id}>{program.nameTh}</option
								>{/each}</select
						></label
					>
					<div class="grid grid-cols-2 gap-3">
						<label class="space-y-1.5 text-sm"
							><span class="font-medium">ชนิดทรัพยากร</span><select
								class="h-10 w-full rounded-md border bg-background px-3"
								bind:value={requirementDraft.resourceKind}
								><option value="course">รายวิชา</option><option value="activity">กิจกรรม</option
								></select
							></label
						><label class="space-y-1.5 text-sm"
							><span class="font-medium">ข้อกำหนด</span><select
								class="h-10 w-full rounded-md border bg-background px-3"
								bind:value={requirementDraft.requirementKind}
								><option value="required">บังคับ</option><option value="elective">เลือก</option
								><option value="optional">เพิ่มเติม</option></select
							></label
						>
					</div>
					<div class="space-y-1.5">
						<Label for={`requirement-version-${version.id}`}>รหัสรุ่นรายวิชา/กิจกรรม</Label><Input
							id={`requirement-version-${version.id}`}
							bind:value={requirementDraft.catalogVersionId}
							required
						/>
					</div>
					<div class="space-y-1.5">
						<Label for={`requirement-grade-${version.id}`}>รหัสระดับชั้น</Label><Input
							id={`requirement-grade-${version.id}`}
							bind:value={requirementDraft.gradeLevelId}
							required
						/>
					</div>
					<div class="grid grid-cols-2 gap-3">
						<div class="space-y-1.5">
							<Label for={`requirement-credit-${version.id}`}>หน่วยกิต</Label><Input
								id={`requirement-credit-${version.id}`}
								bind:value={requirementDraft.credit}
							/>
						</div>
						<div class="space-y-1.5">
							<Label for={`requirement-hours-${version.id}`}>ชั่วโมง</Label><Input
								id={`requirement-hours-${version.id}`}
								bind:value={requirementDraft.hours}
							/>
						</div>
					</div>
					<div class="space-y-1.5">
						<Label for={`requirement-term-${version.id}`}>รหัสภาคเรียนแนะนำ</Label><Input
							id={`requirement-term-${version.id}`}
							bind:value={requirementDraft.recommendedTermCode}
						/>
					</div>
					<Button
						type="submit"
						variant="outline"
						class="w-full"
						disabled={busy || programs.length === 0}><Plus class="size-4" /> เพิ่มข้อกำหนด</Button
					>
				</form>
			</div>
		{/if}
	</div>
	{#if errorMessage}<p role="alert" class="border-t px-5 py-3 text-sm text-destructive">
			{errorMessage}
		</p>{/if}
</section>
