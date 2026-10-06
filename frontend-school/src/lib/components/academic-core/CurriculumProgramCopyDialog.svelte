<script lang="ts">
	import {
		getCurriculumOverview,
		listCurriculumLevels,
		listStudyPrograms,
		copyStudyProgram,
		type CurriculumEdition,
		type CurriculumLevel,
		type StudyProgram
	} from '#lib/api/academic-core.js';
	import { LoadingButton } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	let { level, onCopied }: { level: CurriculumLevel; onCopied: () => void } = $props();
	let open = $state(false);
	let loading = $state(false);
	let saving = $state(false);
	let error = $state('');
	let editions = $state.raw<CurriculumEdition[]>([]);
	let levels = $state.raw<CurriculumLevel[]>([]);
	let programs = $state.raw<StudyProgram[]>([]);
	let sourceEditionId = $state('');
	let sourceLevelId = $state('');
	let sourceProgramId = $state('');
	let name = $state('');
	let requestNo = 0;
	let sourceProgram = $derived(programs.find((p) => p.id === sourceProgramId));
	async function show() {
		open = true;
		error = '';
		loading = true;
		const current = ++requestNo;
		try {
			const result = await getCurriculumOverview();
			if (current === requestNo)
				editions = result.items
					.map((item) => item.edition)
					.filter((edition) => edition.status === 'published' && edition.isActive);
		} catch (e) {
			if (current === requestNo) error = e instanceof Error ? e.message : 'โหลดฉบับต้นทางไม่สำเร็จ';
		} finally {
			if (current === requestNo) loading = false;
		}
	}
	async function chooseEdition(id: string) {
		sourceLevelId = '';
		sourceProgramId = '';
		levels = [];
		programs = [];
		name = '';
		if (!id) return;
		loading = true;
		error = '';
		const current = ++requestNo;
		try {
			const result = await listCurriculumLevels(id);
			if (current === requestNo)
				levels = result.map((view) => view.level).filter((level) => level.isActive);
		} catch (e) {
			if (current === requestNo)
				error = e instanceof Error ? e.message : 'โหลดระดับต้นทางไม่สำเร็จ';
		} finally {
			if (current === requestNo) loading = false;
		}
	}
	async function chooseLevel(id: string) {
		sourceProgramId = '';
		programs = [];
		name = '';
		if (!id) return;
		loading = true;
		error = '';
		const current = ++requestNo;
		try {
			const result = await listStudyPrograms(id);
			if (current === requestNo)
				programs = result.filter((program) => program.status === 'published');
		} catch (e) {
			if (current === requestNo) error = e instanceof Error ? e.message : 'โหลดแผนต้นทางไม่สำเร็จ';
		} finally {
			if (current === requestNo) loading = false;
		}
	}
	async function save(event: SubmitEvent) {
		event.preventDefault();
		if (!sourceProgram) return;
		saving = true;
		error = '';
		try {
			await copyStudyProgram(level.id, {
				sourceProgramId: sourceProgram.id,
				sourceRowVersion: sourceProgram.rowVersion,
				destinationRowVersion: level.rowVersion,
				nameTh: name.trim() || null
			});
			open = false;
			onCopied();
		} catch (e) {
			error = e instanceof Error ? e.message : 'คัดลอกแผนไม่สำเร็จ';
		} finally {
			saving = false;
		}
	}
</script>

<Button variant="outline" onclick={show}>คัดลอกแผนจากฉบับเดิม</Button>
<Dialog.Root bind:open
	><Dialog.Content class="sm:max-w-xl"
		><Dialog.Header
			><Dialog.Title>คัดลอกแผนจากฉบับเดิม</Dialog.Title><Dialog.Description
				>เลือกแผนที่เผยแพร่แล้วเพื่อคัดลอกมาเป็นร่างใน {level.editionName} · {level.nameTh} ข้อมูลต้นฉบับและห้องเดิมจะคงเดิม</Dialog.Description
			></Dialog.Header
		>
		<form class="space-y-4 py-2" onsubmit={save}>
			<div class="space-y-2">
				<Label>ฉบับต้นทาง *</Label><Select.Root
					type="single"
					bind:value={sourceEditionId}
					onValueChange={chooseEdition}
					><Select.Trigger class="w-full"
						>{editions.find((e) => e.id === sourceEditionId)?.name ??
							'เลือกฉบับหลักสูตร'}</Select.Trigger
					><Select.Content
						>{#each editions as edition (edition.id)}<Select.Item value={edition.id}
								>{edition.name}</Select.Item
							>{/each}</Select.Content
					></Select.Root
				>
			</div>
			<div class="space-y-2">
				<Label>ระดับการศึกษาต้นทาง *</Label><Select.Root
					type="single"
					bind:value={sourceLevelId}
					onValueChange={chooseLevel}
					><Select.Trigger class="w-full" disabled={!sourceEditionId || loading}
						>{levels.find((l) => l.id === sourceLevelId)?.nameTh ??
							'เลือกระดับการศึกษา'}</Select.Trigger
					><Select.Content
						>{#each levels as source (source.id)}<Select.Item value={source.id}
								>{source.nameTh}</Select.Item
							>{/each}</Select.Content
					></Select.Root
				>
			</div>
			<div class="space-y-2">
				<Label>แผนต้นทาง *</Label><Select.Root type="single" bind:value={sourceProgramId}
					><Select.Trigger class="w-full" disabled={!sourceLevelId || loading}
						>{sourceProgram?.nameTh ?? 'เลือกแผนการเรียน'}</Select.Trigger
					><Select.Content
						>{#each programs as program (program.id)}<Select.Item value={program.id}
								>{program.nameTh}</Select.Item
							>{/each}</Select.Content
					></Select.Root
				>
			</div>
			<div class="space-y-2">
				<Label for="copied-program-name">ชื่อแผนในฉบับใหม่ (ถ้าต้องการเปลี่ยน)</Label><Input
					id="copied-program-name"
					bind:value={name}
					placeholder={sourceProgram?.nameTh ?? 'ใช้ชื่อเดิม'}
				/>
			</div>
			{#if loading}<p class="text-sm text-muted-foreground" aria-live="polite">
					กำลังโหลดตัวเลือก…
				</p>{/if}{#if error}<p role="alert" class="text-sm text-destructive">{error}</p>{/if}
			<Dialog.Footer
				><Button
					type="button"
					variant="outline"
					disabled={saving}
					onclick={() => {
						requestNo++;
						open = false;
					}}>ยกเลิก</Button
				><LoadingButton
					type="submit"
					loading={saving}
					loadingLabel="กำลังคัดลอก"
					disabled={!sourceProgram || loading}>คัดลอกเป็นแผนร่าง</LoadingButton
				></Dialog.Footer
			>
		</form>
	</Dialog.Content></Dialog.Root
>
