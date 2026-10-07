<script lang="ts">
	import { untrack } from 'svelte';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { LoadingButton } from '#lib/components/app-state/index.js';
	import DatePicker from '#lib/components/ui/date-picker/DatePicker.svelte';
	import type { CreateCalendarRequest } from '#lib/api/calendar.js';
	let {
		open = $bindable(false),
		initialDate,
		saving = false,
		error = '',
		onsubmit
	}: {
		open: boolean;
		initialDate: string;
		saving?: boolean;
		error?: string;
		onsubmit: (payload: CreateCalendarRequest) => void;
	} = $props();
	let title = $state(''),
		description = $state(''),
		location = $state('');
	let startDate = $state(untrack(() => initialDate)),
		endDate = $state(untrack(() => initialDate));
	let allDay = $state(true),
		startTime = $state(''),
		endTime = $state('');
	let validation = $state('');
	function submit() {
		if (saving) return;
		validation = '';
		if (!title.trim() || !description.trim()) {
			validation = 'กรุณาระบุชื่อและรายละเอียดกิจกรรม';
			return;
		}
		if (!startDate || !endDate || endDate < startDate) {
			validation = 'วันที่สิ้นสุดต้องไม่ก่อนวันที่เริ่มต้น';
			return;
		}
		if (!allDay && (!startTime || !endTime || (startDate === endDate && endTime <= startTime))) {
			validation = 'กรุณาระบุเวลาเริ่มและสิ้นสุดให้ถูกต้อง';
			return;
		}
		onsubmit({
			title: title.trim(),
			description: description.trim(),
			location: location.trim() || null,
			startDate,
			endDate,
			allDay,
			startTime: allDay ? null : startTime,
			endTime: allDay ? null : endTime
		});
	}
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="max-h-[90dvh] overflow-y-auto sm:max-w-xl">
		<Dialog.Header>
			<Dialog.Title>คำร้องขอเพิ่มวันกิจกรรม</Dialog.Title>
			<Dialog.Description>กิจกรรมจะขึ้นปฏิทินหลังผู้ดูแลตรวจและอนุมัติ</Dialog.Description>
		</Dialog.Header>
		<form
			class="space-y-4"
			onsubmit={(event) => {
				event.preventDefault();
				submit();
			}}
		>
			<div class="space-y-2">
				<Label for="request-title">ชื่อกิจกรรม *</Label><Input
					id="request-title"
					bind:value={title}
					required
					maxlength={200}
					disabled={saving}
				/>
			</div>
			<div class="grid gap-4 sm:grid-cols-2">
				<div class="space-y-2">
					<Label for="request-start-date">วันที่เริ่ม *</Label><DatePicker
						id="request-start-date"
						bind:value={startDate}
						required
						disabled={saving}
						ariaLabel="วันที่เริ่มกิจกรรม"
					/>
				</div>
				<div class="space-y-2">
					<Label for="request-end-date">วันที่สิ้นสุด *</Label><DatePicker
						id="request-end-date"
						bind:value={endDate}
						required
						disabled={saving}
						ariaLabel="วันที่สิ้นสุดกิจกรรม"
					/>
				</div>
			</div>
			<div class="flex items-center gap-2">
				<Checkbox id="request-all-day" bind:checked={allDay} disabled={saving} /><Label
					for="request-all-day">ทั้งวัน</Label
				>
			</div>
			{#if !allDay}
				<div class="grid grid-cols-2 gap-4">
					<div class="space-y-2">
						<Label for="request-start-time">เวลาเริ่ม *</Label><Input
							id="request-start-time"
							type="time"
							bind:value={startTime}
							required
							disabled={saving}
						/>
					</div>
					<div class="space-y-2">
						<Label for="request-end-time">เวลาสิ้นสุด *</Label><Input
							id="request-end-time"
							type="time"
							bind:value={endTime}
							required
							disabled={saving}
						/>
					</div>
				</div>
			{/if}
			<div class="space-y-2">
				<Label for="request-location">สถานที่</Label><Input
					id="request-location"
					bind:value={location}
					maxlength={200}
					disabled={saving}
				/>
			</div>
			<div class="space-y-2">
				<Label for="request-description">รายละเอียดกิจกรรม *</Label><Textarea
					id="request-description"
					bind:value={description}
					required
					maxlength={5000}
					disabled={saving}
					class="min-h-24"
				/>
			</div>
			{#if validation || error}<p role="alert" class="text-sm text-destructive">
					{validation || error}
				</p>{/if}
			<Dialog.Footer
				><Button type="button" variant="outline" disabled={saving} onclick={() => (open = false)}
					>ยกเลิก</Button
				><LoadingButton type="submit" loading={saving} loadingLabel="กำลังส่งคำร้อง..."
					>ส่งคำร้อง</LoadingButton
				></Dialog.Footer
			>
		</form>
	</Dialog.Content>
</Dialog.Root>
