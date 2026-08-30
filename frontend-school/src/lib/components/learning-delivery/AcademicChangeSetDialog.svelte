<script lang="ts">
	import {
		createAcademicTermChangeSet,
		type AcademicTermChangeSet
	} from '$lib/api/learning-delivery';
	import { LoadingButton } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import { DatePicker } from '$lib/components/ui/date-picker';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Label } from '$lib/components/ui/label';
	import { Textarea } from '$lib/components/ui/textarea';
	import { CalendarClock, TriangleAlert } from 'lucide-svelte';

	type ChangePurpose = 'operational_change' | 'timetable_revision';

	let {
		academicTermId,
		onCreated,
		purpose = 'operational_change'
	}: {
		academicTermId: string;
		onCreated: (changeSet: AcademicTermChangeSet) => void;
		purpose?: ChangePurpose;
	} = $props();

	let open = $state(false);
	let effectiveFrom = $state('');
	let reason = $state('');
	let saving = $state(false);
	let errorMessage = $state('');

	async function createDraft(event: SubmitEvent) {
		event.preventDefault();
		if (!effectiveFrom.trim() || !reason.trim()) return;
		saving = true;
		errorMessage = '';
		try {
			const changeSet = await createAcademicTermChangeSet({
				academicTermId,
				effectiveFrom,
				reason: reason.trim(),
				idempotencyKey: crypto.randomUUID()
			});
			onCreated(changeSet);
			effectiveFrom = '';
			reason = '';
			open = false;
		} catch (error) {
			errorMessage =
				error instanceof Error
					? error.message
					: purpose === 'timetable_revision'
						? 'สร้างรุ่นตารางสอนใหม่ไม่สำเร็จ'
						: 'สร้างแบบร่างการเปลี่ยนแปลงกลางภาคไม่สำเร็จ';
		} finally {
			saving = false;
		}
	}
</script>

{#if purpose === 'timetable_revision'}
	<Button variant="outline" onclick={() => (open = true)}>
		<CalendarClock class="size-4" /> สร้างรุ่นตารางสอนใหม่
	</Button>
{:else}
	<Button
		variant="outline"
		class="border-amber-500/40 text-amber-800"
		onclick={() => (open = true)}
	>
		<CalendarClock class="size-4" /> เพิ่ม/ปรับ/หยุดกลางภาค
	</Button>
{/if}

<Dialog.Root bind:open>
	<Dialog.Content class="sm:max-w-xl">
		<Dialog.Header>
			<Dialog.Title>
				{purpose === 'timetable_revision'
					? 'สร้างรุ่นตารางสอนใหม่'
					: 'เริ่มการเปลี่ยนแปลงกลางภาค'}
			</Dialog.Title>
			<Dialog.Description>
				{purpose === 'timetable_revision'
					? 'คัดลอกรุ่นที่ใช้อยู่ตามวันที่เริ่มใช้ แล้วปรับตารางโดยไม่กระทบรุ่นเดิม'
					: 'ใช้เมื่อภาคเรียนเริ่มสอนแล้วและต้องเพิ่ม ปรับคาบ หรือหยุดรายการเปิดสอน'}
			</Dialog.Description>
		</Dialog.Header>

		<div class="flex gap-3 rounded-xl border border-amber-500/30 bg-amber-500/8 p-3 text-sm">
			<TriangleAlert class="mt-0.5 size-4 shrink-0 text-amber-700" />
			<p class="leading-relaxed">
				{#if purpose === 'timetable_revision'}
					ระบบจะสร้างรุ่นแบบร่างจากตารางที่มีผลในวันนั้น รุ่นเดิมยังใช้งานต่อจนถึงวันก่อนเริ่มใช้รุ่นใหม่
				{:else}
					การทำงานนี้มีผลเฉพาะภาคเรียนนี้และ <strong>ไม่เปลี่ยนหลักสูตร</strong>
					ระบบจะสร้างรุ่นตารางสอนแบบร่างสำหรับวันที่เริ่มใช้โดยอัตโนมัติ
				{/if}
			</p>
		</div>

		<form class="space-y-4" onsubmit={createDraft}>
			<div class="space-y-2">
				<Label for="academic-change-effective-date">
					{purpose === 'timetable_revision' ? 'วันที่เริ่มใช้รุ่นใหม่' : 'วันที่เริ่มมีผล'}
				</Label>
				<DatePicker
					id="academic-change-effective-date"
					bind:value={effectiveFrom}
					placeholder="เลือกวันที่เริ่มใช้จริง"
					ariaLabel="เลือกวันที่เริ่มใช้จริง"
					required
				/>
				<p class="text-xs text-muted-foreground">
					{purpose === 'timetable_revision'
						? 'รุ่นใหม่จะเริ่มมีผลในวันนี้ และรุ่นก่อนหน้าจะสิ้นสุดโดยอัตโนมัติเมื่อเผยแพร่'
						: 'รายการและตารางชุดใหม่เริ่มใช้ตั้งแต่วันนี้ ส่วนข้อมูลก่อนหน้านี้ยังคงเดิม'}
				</p>
			</div>
			<div class="space-y-2">
				<Label for="academic-change-reason">เหตุผลการเปลี่ยนแปลง</Label>
				<Textarea
					id="academic-change-reason"
					bind:value={reason}
					rows={3}
					placeholder={purpose === 'timetable_revision'
						? 'เช่น ปรับตารางหลังเปลี่ยนจำนวนคาบและห้องเรียน'
						: 'เช่น เปิดรายวิชาเสริมตั้งแต่สัปดาห์ที่ 8 ตามมติฝ่ายวิชาการ'}
					required
				/>
			</div>
			{#if errorMessage}<p role="alert" class="text-sm text-destructive">{errorMessage}</p>{/if}
			<Dialog.Footer>
				<Button type="button" variant="outline" onclick={() => (open = false)}>ยกเลิก</Button>
				<LoadingButton
					type="submit"
					loading={saving}
					loadingLabel="กำลังสร้างแบบร่าง"
					disabled={!effectiveFrom.trim() || !reason.trim()}
				>
					สร้างแบบร่าง
				</LoadingButton>
			</Dialog.Footer>
		</form>
	</Dialog.Content>
</Dialog.Root>
