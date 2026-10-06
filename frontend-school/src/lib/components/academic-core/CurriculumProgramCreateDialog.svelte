<script lang="ts">
	import { createStudyProgram, type StudyProgram } from '#lib/api/academic-core.js';
	import { LoadingButton } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	let {
		levelId,
		isFirst = false,
		onCreated
	}: { levelId: string; isFirst?: boolean; onCreated: (program: StudyProgram) => void } = $props();
	let open = $state(false);
	let saving = $state(false);
	let name = $state('');
	let isDefault = $state(false);
	let error = $state('');
	function show() {
		name = '';
		isDefault = isFirst;
		error = '';
		open = true;
	}
	async function save(event: SubmitEvent) {
		event.preventDefault();
		if (!name.trim()) return;
		saving = true;
		error = '';
		try {
			const program = await createStudyProgram(levelId, { nameTh: name.trim(), isDefault });
			open = false;
			onCreated(program);
		} catch (e) {
			error = e instanceof Error ? e.message : 'เพิ่มแผนไม่สำเร็จ';
		} finally {
			saving = false;
		}
	}
</script>

<Button onclick={show}>เพิ่มแผนการเรียน</Button>
<Dialog.Root bind:open
	><Dialog.Content class="sm:max-w-xl"
		><Dialog.Header
			><Dialog.Title>เพิ่มแผนการเรียน</Dialog.Title><Dialog.Description
				>สร้างแผนในระดับการศึกษานี้ แล้วกำหนดรายวิชาและกิจกรรมตามชั้นกับภาคเรียน</Dialog.Description
			></Dialog.Header
		>
		<form class="space-y-4 py-2" onsubmit={save}>
			<div class="space-y-2">
				<Label for="new-program-name">ชื่อแผนการเรียน *</Label><Input
					id="new-program-name"
					bind:value={name}
					placeholder="เช่น วิทยาศาสตร์-คณิตศาสตร์"
					required
					maxlength={200}
				/>
			</div>
			<label class="flex items-center gap-2 text-sm"
				><Checkbox bind:checked={isDefault} />ใช้เป็นแผนเริ่มต้นของระดับนี้</label
			>{#if error}<p role="alert" class="text-sm text-destructive">{error}</p>{/if}<Dialog.Footer
				><Button type="button" variant="outline" disabled={saving} onclick={() => (open = false)}
					>ยกเลิก</Button
				><LoadingButton type="submit" loading={saving} loadingLabel="กำลังสร้าง"
					>สร้างแผนการเรียน</LoadingButton
				></Dialog.Footer
			>
		</form></Dialog.Content
	></Dialog.Root
>
