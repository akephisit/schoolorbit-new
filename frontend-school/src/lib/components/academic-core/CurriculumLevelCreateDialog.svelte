<script lang="ts">
	import {
		createCurriculumLevel,
		getCurriculumCreateOptions,
		type CurriculumLevel,
		type CurriculumCreateOptions
	} from '#lib/api/academic-core.js';
	import { LoadingButton } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import GradeLevelMultiSelect from './GradeLevelMultiSelect.svelte';
	let {
		editionId,
		draftId,
		onCreated
	}: { editionId: string; draftId: string; onCreated: (level: CurriculumLevel) => void } = $props();
	let open = $state(false);
	let saving = $state(false);
	let loading = $state(false);
	let error = $state('');
	let name = $state('');
	let grades = $state<string[]>([]);
	let options = $state.raw<CurriculumCreateOptions | null>(null);
	async function show() {
		open = true;
		error = '';
		if (options) return;
		loading = true;
		try {
			options = await getCurriculumCreateOptions();
		} catch (e) {
			error = e instanceof Error ? e.message : 'โหลดระดับชั้นไม่สำเร็จ';
		} finally {
			loading = false;
		}
	}
	async function save(event: SubmitEvent) {
		event.preventDefault();
		if (!name.trim() || !grades.length) return;
		saving = true;
		error = '';
		try {
			const result = await createCurriculumLevel(editionId, {
				draftId,
				nameTh: name.trim(),
				gradeLevelIds: grades,
				description: null
			});
			onCreated(result);
			open = false;
			name = '';
			grades = [];
		} catch (e) {
			error = e instanceof Error ? e.message : 'เพิ่มระดับการศึกษาไม่สำเร็จ';
		} finally {
			saving = false;
		}
	}
</script>

<Button onclick={show}>เพิ่มระดับการศึกษา</Button>
<Dialog.Root bind:open
	><Dialog.Content class="sm:max-w-xl">
		<Dialog.Header
			><Dialog.Title>เพิ่มระดับการศึกษา</Dialog.Title><Dialog.Description
				>กำหนดชื่อระดับและชั้นที่ครอบคลุมภายในฉบับหลักสูตรนี้ เช่น มัธยมศึกษาตอนต้น ครอบคลุม ม.1–ม.3</Dialog.Description
			></Dialog.Header
		>
		<form class="space-y-4 py-2" onsubmit={save}>
			<div class="space-y-2">
				<Label for="level-name">ชื่อระดับการศึกษา *</Label><Input
					id="level-name"
					bind:value={name}
					placeholder="ระดับมัธยมศึกษาตอนต้น"
					required
				/>
			</div>
			{#if loading}<p class="text-sm text-muted-foreground" aria-live="polite">
					กำลังโหลดระดับชั้น…
				</p>{:else if options}<div class="space-y-2">
					<Label>ระดับชั้นที่ครอบคลุม *</Label><GradeLevelMultiSelect
						bind:value={grades}
						options={options.gradeLevels}
						ariaLabel="เลือกระดับชั้นที่ครอบคลุม"
					/>
				</div>{/if}
			{#if error}<p role="alert" class="text-sm text-destructive">{error}</p>{/if}
			<Dialog.Footer
				><Button variant="outline" type="button" disabled={saving} onclick={() => (open = false)}
					>ยกเลิก</Button
				><LoadingButton
					type="submit"
					loading={saving}
					loadingLabel="กำลังเพิ่ม"
					disabled={!name.trim() || !grades.length || loading}>เพิ่มระดับการศึกษา</LoadingButton
				></Dialog.Footer
			>
		</form>
	</Dialog.Content></Dialog.Root
>
