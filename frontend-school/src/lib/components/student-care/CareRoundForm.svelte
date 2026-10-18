<script lang="ts">
	import { CareMutationIdentity } from '#lib/features/student-care/mutation.js';
	const mutation = new CareMutationIdentity();

	import { LatestRequest } from '#lib/async/latest-request.js';
	import { onDestroy, untrack } from 'svelte';
	import { goto } from '$app/navigation';
	import {
		careInstruments,
		createCareRound,
		type CareInstrumentVersion
	} from '#lib/api/student-care.js';
	import { careIdentity } from '#lib/features/student-care/access.js';
	import { careLink, respondentLabels } from '#lib/features/student-care/presentation.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { DatePicker } from '#lib/components/ui/date-picker/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { LoadingButton, PageState } from '#lib/components/app-state/index.js';
	import CareSection from './CareSection.svelte';
	let {
		instruments,
		term,
		date
	}: { instruments: CareInstrumentVersion[]; term: string; date: string } = $props();
	const optionsRequest = new LatestRequest();
	let instrumentPage = $state(untrack(() => instruments));
	let optionsPending = $state(false),
		after = $state(false);
	async function optionsPage(next: boolean) {
		if (optionsPending) return;
		optionsPending = true;
		const identity = $careIdentity;
		const token = optionsRequest.begin();
		try {
			const rows = await careInstruments(
				{
					limit: 100,
					afterId: next ? instrumentPage.at(-1)?.id : undefined
				},
				{ signal: token.signal }
			);
			if (alive && identity === $careIdentity) {
				instrumentPage = rows;
				after = next;
			}
		} catch (cause) {
			if (alive && identity === $careIdentity)
				error = cause instanceof Error ? cause.message : 'โหลดรุ่นแบบประเมินไม่ได้';
		} finally {
			if (alive && identity === $careIdentity) optionsPending = false;
		}
	}
	let name = $state(''),
		purpose = $state(''),
		rosterDate = $state(untrack(() => date)),
		dueDate = $state(untrack(() => date)),
		versions = $state<string[]>([]),
		pending = $state(false),
		error = $state('');
	let alive = true;
	onDestroy(() => {
		alive = false;
		optionsRequest.abort();
	});
	async function save(event: SubmitEvent) {
		event.preventDefault();
		if (pending || !$can.has(PERMISSIONS.STUDENT_CARE_MANAGE_SCHOOL)) return;
		if (!name.trim() || !purpose.trim() || !rosterDate || !dueDate) {
			error = 'กรอกชื่อรอบ วัตถุประสงค์ และวันที่ให้ครบ';
			return;
		}
		if (!versions.length) {
			error = 'เลือกแบบประเมินอย่างน้อยหนึ่งรุ่น';
			return;
		}
		pending = true;
		error = '';
		const identity = $careIdentity;
		try {
			const round = await createCareRound(
				mutation.prepare(
					{
						requestId: crypto.randomUUID(),
						name: name.trim(),
						purpose: purpose.trim(),
						academicTermId: term,
						rosterDate,
						dueDate,
						instrumentVersionIds: versions
					},
					'createCareRound'
				)
			);
			if (alive && identity === $careIdentity) await goto(careLink(`/rounds/${round.id}`, term));
		} catch (cause) {
			if (alive && identity === $careIdentity)
				error = cause instanceof Error ? cause.message : 'สร้างรอบไม่ได้';
		} finally {
			if (alive && identity === $careIdentity) pending = false;
		}
	}
</script>

<form onsubmit={save} class="max-w-3xl space-y-5">
	<CareSection title="รายละเอียดรอบประเมิน"
		><div class="space-y-2">
			<Label for="round-name">ชื่อรอบ *</Label><Input
				id="round-name"
				bind:value={name}
				required
				maxlength={200}
			/>
		</div>
		<div class="space-y-2">
			<Label for="round-purpose">วัตถุประสงค์และการใช้ข้อมูล *</Label><Textarea
				id="round-purpose"
				bind:value={purpose}
				required
				maxlength={2000}
			/>
		</div>
		<div class="grid gap-4 sm:grid-cols-2">
			<div class="space-y-2">
				<Label for="round-roster-date">วันที่อ้างอิงรายชื่อ *</Label><DatePicker
					id="round-roster-date"
					ariaLabel="วันที่อ้างอิงรายชื่อ"
					value={rosterDate}
					onValueChange={(value) => (rosterDate = value ?? '')}
				/>
			</div>
			<div class="space-y-2">
				<Label for="round-due-date">กำหนดส่ง *</Label><DatePicker
					id="round-due-date"
					ariaLabel="กำหนดส่ง"
					value={dueDate}
					onValueChange={(value) => (dueDate = value ?? '')}
				/>
			</div>
		</div></CareSection
	><CareSection
		title="รุ่นแบบประเมินที่จะใช้"
		description="รอบนี้จะใช้เฉพาะรุ่นที่เลือกไว้ แม้ภายหลังมีการสร้างรุ่นใหม่"
	>
		{#each instrumentPage as instrument (instrument.id)}<div
				class="flex items-start gap-2 rounded-lg border p-3"
			>
				<Checkbox
					id={`round-version-${instrument.id}`}
					checked={versions.includes(instrument.id)}
					onCheckedChange={(checked) =>
						(versions = checked
							? [...versions, instrument.id]
							: versions.filter((id) => id !== instrument.id))}
				/><Label for={`round-version-${instrument.id}`} class="min-w-0 break-words"
					>{instrument.definition.name} · {instrument.definition.variant} · รุ่น {instrument.version}<span
						class="mt-1 block font-normal text-muted-foreground"
						>{respondentLabels[instrument.definition.respondent]} · {instrument.definition.mode ===
						'digital'
							? 'ออนไลน์'
							: 'บันทึกคะแนนกระดาษ'}</span
					></Label
				>
			</div>{/each}
		{#if !instrumentPage.length}<PageState
				title="ยังไม่มีแบบประเมิน"
				description="ผู้ดูแลต้องเพิ่มแบบประเมินและเกณฑ์ก่อนสร้างรอบ"
			/>{/if}</CareSection
	>
	<div class="flex flex-wrap items-center justify-between gap-2">
		<p class="text-sm text-muted-foreground">เลือกแล้ว {versions.length} รุ่น</p>
		<div class="flex gap-2">
			{#if after}<Button
					type="button"
					variant="ghost"
					disabled={optionsPending}
					onclick={() => optionsPage(false)}>หน้าแรก</Button
				>{/if}{#if instrumentPage.length === 100}<LoadingButton
					type="button"
					variant="outline"
					loading={optionsPending}
					onclick={() => optionsPage(true)}>รุ่นถัดไป</LoadingButton
				>{/if}
		</div>
	</div>
	{#if error}<PageState variant="error" title="สร้างรอบไม่ได้" description={error} />{/if}
	<div class="flex justify-end gap-2">
		<Button variant="outline" href={careLink('/rounds', term)}>ยกเลิก</Button><LoadingButton
			type="submit"
			loading={pending}
			disabled={!instruments.length}>สร้างรอบประเมิน</LoadingButton
		>
	</div>
</form>
