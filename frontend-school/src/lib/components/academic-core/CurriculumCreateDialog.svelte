<script lang="ts">
	import {
		createCurriculum,
		updateCurriculum,
		type CurriculumEdition
	} from '#lib/api/academic-core.js';
	import { LoadingButton } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	let {
		edition = null,
		onSaved
	}: { edition?: CurriculumEdition | null; onSaved: (edition: CurriculumEdition) => void } =
		$props();
	let open = $state(false);
	let saving = $state(false);
	let error = $state('');
	let year = $state<number | undefined>(2569);
	let name = $state('');
	let description = $state('');
	function show() {
		year = edition?.revisionYear ?? new Date().getFullYear() + 543;
		name = edition?.name ?? '';
		description = edition?.description ?? '';
		error = '';
		open = true;
	}
	async function save(event: SubmitEvent) {
		event.preventDefault();
		if (year === undefined || !Number.isInteger(year) || year < 2400 || year > 2999) {
			error = 'ระบุปีปรับปรุงเป็นพุทธศักราชระหว่าง 2400–2999';
			return;
		}
		saving = true;
		error = '';
		try {
			const body = {
				name: name.trim() || `ฉบับปรับปรุง พุทธศักราช ${year}`,
				revisionYear: year,
				description: description.trim() || null
			};
			const result = edition
				? await updateCurriculum(edition.id, { ...body, rowVersion: edition.rowVersion })
				: await createCurriculum(body);
			onSaved(result);
			open = false;
		} catch (e) {
			error = e instanceof Error ? e.message : 'บันทึกฉบับหลักสูตรไม่สำเร็จ';
		} finally {
			saving = false;
		}
	}
</script>

<Button variant={edition ? 'outline' : 'default'} onclick={show}
	>{edition ? 'แก้ไขฉบับหลักสูตร' : 'เพิ่มฉบับหลักสูตร'}</Button
>
<Dialog.Root bind:open
	><Dialog.Content class="sm:max-w-xl">
		<Dialog.Header
			><Dialog.Title>{edition ? 'แก้ไขฉบับหลักสูตร' : 'เพิ่มฉบับหลักสูตร'}</Dialog.Title
			><Dialog.Description
				>ระบุปีปรับปรุงเพื่อใช้เลือกฉบับหลักสูตร ปีนี้ไม่จำกัดปีการศึกษาที่นำไปใช้</Dialog.Description
			></Dialog.Header
		>
		<form class="space-y-4 py-2" onsubmit={save}>
			<div class="space-y-2">
				<Label for="edition-year">ปีปรับปรุง (พุทธศักราช) *</Label><Input
					id="edition-year"
					type="number"
					min={2400}
					max={2999}
					step={1}
					bind:value={year}
					required
				/>
			</div>
			<div class="space-y-2">
				<Label for="edition-name">ชื่อฉบับเพิ่มเติม (ถ้ามี)</Label><Input
					id="edition-name"
					bind:value={name}
					placeholder={`ฉบับปรับปรุง พุทธศักราช ${year ?? '2569'}`}
				/>
			</div>
			<div class="space-y-2">
				<Label for="edition-description">รายละเอียด (ถ้ามี)</Label><Textarea
					id="edition-description"
					bind:value={description}
				/>
			</div>
			{#if error}<p role="alert" class="text-sm text-destructive">{error}</p>{/if}
			<Dialog.Footer
				><Button variant="outline" type="button" disabled={saving} onclick={() => (open = false)}
					>ยกเลิก</Button
				><LoadingButton type="submit" loading={saving} loadingLabel="กำลังบันทึก"
					>{edition ? 'บันทึก' : 'สร้างฉบับร่าง'}</LoadingButton
				></Dialog.Footer
			>
		</form>
	</Dialog.Content></Dialog.Root
>
