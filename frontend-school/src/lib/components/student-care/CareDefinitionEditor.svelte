<script lang="ts">
	import {
		LibraryBig,
		FileCheck,
		ListChecks,
		MessageSquareText,
		Plus,
		Save,
		Upload,
		Download
	} from '@lucide/svelte';
	import { CareMutationIdentity } from '#lib/features/student-care/mutation.js';
	const mutation = new CareMutationIdentity();

	import { untrack, onDestroy } from 'svelte';
	import { goto } from '$app/navigation';
	import {
		createCareInstrument,
		type CareInstrumentVersion,
		type CareInstrumentDefinition
	} from '#lib/api/student-care.js';
	import { careIdentity } from '#lib/features/student-care/access.js';
	import {
		careImportError,
		emptyCareDefinition,
		definitionErrors,
		parseCareDefinition
	} from '#lib/features/student-care/drafts.js';
	import {
		careLink,
		careDefinitionIssue,
		familyLabels,
		respondentLabels
	} from '#lib/features/student-care/presentation.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { LoadingButton, PageState } from '#lib/components/app-state/index.js';
	import CareSection from './CareSection.svelte';
	import CareSelect from './CareSelect.svelte';
	let { initial, term }: { initial?: CareInstrumentVersion; term?: string | null } = $props();
	let definition = $state<CareInstrumentDefinition>(
		untrack(() => structuredClone(initial?.definition ?? emptyCareDefinition()))
	);
	let pending = $state(false),
		error = $state(''),
		attempted = $state(false),
		showReferences = $state(false),
		json = $state(''),
		showTransfer = $state(false);
	const errors = $derived(attempted ? definitionErrors(definition) : []);
	let alive = true;
	onDestroy(() => {
		alive = false;
	});
	function addScale() {
		const key = `scale_${definition.scales.length + 1}`;
		definition.scales.push({
			key,
			label: '',
			sourceScaleKeys: [],
			bands: [{ key: 'band_1', label: '', minimum: 0, maximum: 0, urgent: false }]
		});
	}
	function addQuestion() {
		definition.questions.push({
			key: `q_${definition.questions.length + 1}`,
			text: '',
			scaleKey: definition.scales.find((s) => !s.sourceScaleKeys.length)?.key ?? '',
			reverse: false,
			options: [
				{ key: 'a', label: '', points: 0, urgent: false },
				{ key: 'b', label: '', points: 1, urgent: false }
			]
		});
	}
	function importDefinition() {
		try {
			definition = parseCareDefinition(json);
			error = '';
			attempted = false;
		} catch (cause) {
			error = careImportError(cause);
		}
	}
	function exportDefinition() {
		json = JSON.stringify(definition, null, 2);
		showTransfer = true;
	}
	async function save(event: SubmitEvent) {
		event.preventDefault();
		attempted = true;
		if (errors.length) showReferences = true;
		if (pending || errors.length || !$can.has(PERMISSIONS.STUDENT_CARE_CONFIG_MANAGE_SCHOOL))
			return;
		pending = true;
		error = '';
		const identity = $careIdentity;
		try {
			const result = await createCareInstrument(
				mutation.prepare(
					{
						requestId: crypto.randomUUID(),
						instrumentId: initial?.instrumentId ?? null,
						expectedVersion: initial?.version ?? null,
						definition
					},
					'createCareInstrument'
				)
			);
			if (alive && identity === $careIdentity) await goto(careLink(`/library/${result.id}`, term));
		} catch (cause) {
			if (alive && identity === $careIdentity)
				error = cause instanceof Error ? cause.message : 'บันทึกไม่ได้';
		} finally {
			if (alive && identity === $careIdentity) pending = false;
		}
	}
</script>

<form onsubmit={save} class="max-w-4xl space-y-5">
	<nav aria-label="ส่วนของการตั้งค่าแบบประเมิน" class="flex flex-wrap gap-1">
		<Button href="#care-editor-basic" variant="ghost" size="sm">1. ข้อมูลแบบประเมิน</Button><Button
			href="#care-editor-rights"
			variant="ghost"
			size="sm">2. คำชี้แจงและแหล่งที่มา</Button
		><Button href="#care-editor-scales" variant="ghost" size="sm">3. คะแนนและเกณฑ์</Button
		>{#if definition.mode === 'digital' || definition.questions.length}<Button
				href="#care-editor-questions"
				variant="ghost"
				size="sm">4. คำถามและตัวเลือก</Button
			>{/if}
	</nav>
	{#if errors.length}<div
			role="alert"
			class="rounded-xl border border-destructive/30 bg-destructive/5 p-4"
		>
			<p class="font-medium text-destructive">ตรวจสอบการตั้งค่า</p>
			<p class="mt-1 text-sm">
				ยังไม่บันทึกข้อมูล กรุณาแก้ไขรายการต่อไปนี้ รหัสอ้างอิงในแต่ละส่วนเปิดไว้ให้ตรวจสอบแล้ว
			</p>
			<ul class="mt-2 list-inside list-disc text-sm">
				{#each errors as message, i (i)}{@const issue = careDefinitionIssue(message)}
					<li>
						<a href={issue.href} class="underline underline-offset-4">{issue.label}</a>: {issue.message}
					</li>{/each}
			</ul>
		</div>{/if}
	<CareSection
		title={initial ? '1. สร้างรุ่นใหม่จากแบบเดิม' : '1. ข้อมูลแบบประเมิน'}
		icon={LibraryBig}
		id="care-editor-basic"
		description="แต่ละรุ่นเก็บถาวร รอบเดิมจะใช้รุ่นที่เลือกไว้"
	>
		<div class="grid gap-4 sm:grid-cols-2">
			<div class="space-y-2">
				<Label for="care-name">ชื่อแบบประเมิน *</Label><Input
					id="care-name"
					bind:value={definition.name}
					required
					maxlength={200}
				/>
			</div>
			<div class="space-y-2">
				<Label for="care-variant">ชื่อรูปแบบ / กลุ่มอายุ *</Label><Input
					id="care-variant"
					bind:value={definition.variant}
					required
					maxlength={120}
				/>
			</div>
			<CareSelect
				id="care-family"
				label="กลุ่มแบบประเมิน"
				value={definition.family}
				options={Object.entries(familyLabels).map(([value, label]) => ({ value, label }))}
				onValueChange={(value) => {
					if (value === 'sdq' || value === 'eq' || value === 'depression' || value === 'other')
						definition.family = value;
				}}
			/>
			<CareSelect
				id="care-respondent"
				label="ผู้ตอบ"
				value={definition.respondent}
				options={Object.entries(respondentLabels).map(([value, label]) => ({ value, label }))}
				onValueChange={(value) => {
					if (value === 'self_report' || value === 'teacher' || value === 'guardian')
						definition.respondent = value;
				}}
			/>
			<div class="space-y-2">
				<Label for="care-language">ภาษา *</Label><Input
					id="care-language"
					bind:value={definition.language}
					required
					maxlength={30}
				/>
			</div>
			<div class="grid grid-cols-2 gap-3">
				<div class="space-y-2">
					<Label for="care-age-min">อายุต่ำสุด</Label><Input
						id="care-age-min"
						type="number"
						min={0}
						max={120}
						bind:value={definition.minimumAge}
					/>
				</div>
				<div class="space-y-2">
					<Label for="care-age-max">อายุสูงสุด</Label><Input
						id="care-age-max"
						type="number"
						min={0}
						max={120}
						bind:value={definition.maximumAge}
					/>
				</div>
			</div>
		</div>
		<CareSelect
			id="care-mode"
			label="วิธีประเมิน"
			value={definition.mode}
			options={[
				{ value: 'manual_result', label: 'บันทึกคะแนนจากแบบกระดาษ' },
				{ value: 'digital', label: 'ตอบแบบประเมินออนไลน์' }
			]}
			onValueChange={(value) => {
				if (value === 'manual_result' || value === 'digital') definition.mode = value;
			}}
		/>
	</CareSection><CareSection
		title="2. คำชี้แจง แหล่งที่มา และสิทธิ์ใช้งาน"
		icon={FileCheck}
		id="care-editor-rights"
		description="ระบุคำชี้แจงที่ผู้ตอบจะเห็น และอ้างอิงเอกสารที่โรงเรียนได้รับอนุญาตให้ใช้"
	>
		<div class="space-y-2">
			<Label for="care-instructions">คำชี้แจง</Label><Textarea
				id="care-instructions"
				bind:value={definition.instructions}
				maxlength={10000}
			/>
		</div>
		<div class="space-y-2">
			<Label for="care-source">แหล่งที่มา / เอกสารอ้างอิง *</Label><Textarea
				id="care-source"
				bind:value={definition.source}
				required
				maxlength={2000}
			/>
		</div>
		<div class="space-y-2">
			<Label for="care-license">เงื่อนไขการใช้งาน / สิทธิ์เผยแพร่ *</Label><Textarea
				id="care-license"
				bind:value={definition.licenseNote}
				required
				maxlength={2000}
			/>
		</div>
		{#if definition.mode === 'digital'}<div
				class="flex items-start gap-2 rounded-lg border border-warning/30 bg-warning/5 p-3"
			>
				<Checkbox id="care-rights" bind:checked={definition.digitalRightsConfirmed} />
				<div>
					<Label for="care-rights">ยืนยันว่ามีสิทธิ์เผยแพร่คำถามนี้ทางดิจิทัล</Label>
					<p class="mt-1 text-sm text-muted-foreground">
						SDQ ต้องได้รับอนุญาตจากเจ้าของลิขสิทธิ์ก่อนทำแบบออนไลน์
						หากยังไม่มีสิทธิ์ให้ใช้การบันทึกคะแนนจากกระดาษ
					</p>
				</div>
			</div>{/if}
	</CareSection>
	<CareSection
		title="3. คะแนนและเกณฑ์แปลผล"
		icon={ListChecks}
		id="care-editor-scales"
		description="ใช้เกณฑ์จากแหล่งอ้างอิงที่ได้รับอนุญาตเท่านั้น ช่วงคะแนนต้องต่อเนื่องและครอบคลุมคะแนนทั้งหมด"
	>
		{#each definition.scales as scale, i (i)}
			{@const availableSources = definition.scales
				.map((source, sourceIndex) => ({ source, sourceIndex }))
				.filter(
					({ source, sourceIndex }) =>
						sourceIndex !== i && source.key !== scale.key && !source.sourceScaleKeys.length
				)}
			{@const invalidReferences = scale.sourceScaleKeys.filter(
				(key) => !availableSources.some(({ source }) => source.key === key)
			)}
			<fieldset class="space-y-3 rounded-xl border p-3">
				<legend class="px-1 text-sm font-semibold">มาตรวัด {i + 1}</legend>
				<div class="grid gap-3 sm:grid-cols-2">
					<div class="space-y-2">
						<Label for={`scale-label-${i}`}>ชื่อมาตรวัด {i + 1}</Label><Input
							id={`scale-label-${i}`}
							bind:value={scale.label}
						/>
					</div>
					<details class="space-y-2 rounded-lg border border-dashed p-3" open={showReferences}>
						<summary class="cursor-pointer text-sm text-muted-foreground"
							>รหัสอ้างอิงมาตรวัด</summary
						>
						<div class="mt-3 space-y-2">
							<Label for={`scale-key-${i}`}>รหัสมาตรวัด {i + 1}</Label><Input
								id={`scale-key-${i}`}
								bind:value={scale.key}
							/>
							<p class="text-xs text-muted-foreground">
								ใช้เชื่อมโยงการตั้งค่า รหัสต้องไม่ซ้ำภายในกลุ่มเดียวกัน
							</p>
							<div class="mt-3 space-y-2">
								<Label for={`scale-sources-${i}`}>รหัสมาตรวัดที่จะรวม {i + 1}</Label><Input
									id={`scale-sources-${i}`}
									value={scale.sourceScaleKeys.join(', ')}
									oninput={(event) =>
										(scale.sourceScaleKeys = event.currentTarget.value
											.split(',')
											.map((key) => key.trim())
											.filter(Boolean))}
								/>
								<p class="text-xs text-muted-foreground">
									คั่นรหัสด้วยจุลภาค ใช้แก้การเชื่อมโยงเมื่อมีการเปลี่ยนรหัสมาตรวัด
								</p>
							</div>
						</div>
					</details>
				</div>
				<fieldset class="space-y-2 rounded-lg bg-muted/30 p-3">
					<legend class="px-1 text-sm font-medium">รวมคะแนนจากมาตรวัดพื้นฐาน</legend>
					<p class="text-xs text-muted-foreground">
						เว้นว่างหากมาตรวัดนี้รับคะแนนจากคำถามโดยตรง เลือกได้เฉพาะมาตรวัดพื้นฐาน
						ไม่สามารถเลือกตัวเองหรือมาตรวัดรวม
					</p>
					<div class="flex flex-wrap gap-x-4 gap-y-2">
						{#each availableSources as { source, sourceIndex } (sourceIndex)}<div
								class="flex items-center gap-2"
							>
								<Checkbox
									id={`scale-source-${i}-${sourceIndex}`}
									checked={scale.sourceScaleKeys.includes(source.key)}
									onCheckedChange={(checked) =>
										(scale.sourceScaleKeys = checked
											? [...new Set([...scale.sourceScaleKeys, source.key])]
											: scale.sourceScaleKeys.filter((key) => key !== source.key))}
								/><Label for={`scale-source-${i}-${sourceIndex}`}
									>{source.label || `มาตรวัด ${sourceIndex + 1}`}</Label
								>
							</div>{/each}
					</div>
					{#if !availableSources.length}<p class="text-sm text-muted-foreground">
							ยังไม่มีมาตรวัดพื้นฐานอื่นให้เลือก เพิ่มมาตรวัดพื้นฐานก่อนหากต้องการคำนวณคะแนนรวม
						</p>{/if}
					{#if invalidReferences.length}<div
							role="group"
							aria-label="รหัสมาตรวัดที่ต้องแก้ไข"
							class="space-y-2 rounded-lg border border-warning/30 bg-warning/5 p-3"
						>
							<p class="text-sm font-medium">ตรวจสอบมาตรวัดที่เชื่อมโยง</p>
							<p class="text-xs text-muted-foreground">
								รหัสเดิมยังเก็บอยู่ เลือกนำออกหรือแก้รหัสในส่วนรหัสอ้างอิงมาตรวัดก่อนบันทึก
							</p>
							<ul class="space-y-2">
								{#each invalidReferences as key, referenceIndex (referenceIndex)}
									{@const referenced = definition.scales.find((candidate) => candidate.key === key)}
									<li class="flex flex-wrap items-center justify-between gap-2">
										<p class="min-w-0 break-words text-sm">
											{key}: {key === scale.key
												? 'ไม่สามารถรวมคะแนนจากตัวเอง'
												: referenced?.sourceScaleKeys.length
													? 'มาตรวัดรวมไม่สามารถใช้เป็นแหล่งคะแนน'
													: 'ไม่พบมาตรวัดพื้นฐานตามรหัสนี้'}
										</p>
										<Button
											type="button"
											variant="outline"
											size="sm"
											aria-label={`นำรหัสมาตรวัด ${key} ออกจากมาตรวัด ${i + 1}`}
											onclick={() =>
												(scale.sourceScaleKeys = scale.sourceScaleKeys.filter(
													(reference) => reference !== key
												))}>นำออก</Button
										>
									</li>
								{/each}
							</ul>
						</div>{/if}
				</fieldset>
				{#each scale.bands as band, j (j)}<div
						class="grid gap-3 rounded-lg bg-muted/40 p-3 sm:grid-cols-2"
					>
						<div class="space-y-2">
							<Label for={`band-label-${i}-${j}`}>ชื่อผลช่วง {i + 1}.{j + 1}</Label><Input
								id={`band-label-${i}-${j}`}
								bind:value={band.label}
							/>
						</div>
						<details class="space-y-2 rounded-lg border border-dashed p-3" open={showReferences}>
							<summary class="cursor-pointer text-sm text-muted-foreground"
								>รหัสอ้างอิงช่วงคะแนน</summary
							>
							<div class="mt-3 space-y-2">
								<Label for={`band-key-${i}-${j}`}>รหัสช่วง {i + 1}.{j + 1}</Label><Input
									id={`band-key-${i}-${j}`}
									bind:value={band.key}
								/>
								<p class="text-xs text-muted-foreground">
									ใช้เชื่อมโยงการตั้งค่า รหัสต้องไม่ซ้ำภายในกลุ่มเดียวกัน
								</p>
							</div>
						</details>
						<div class="space-y-2">
							<Label for={`band-min-${i}-${j}`}>คะแนนตั้งแต่ {i + 1}.{j + 1}</Label><Input
								id={`band-min-${i}-${j}`}
								type="number"
								bind:value={band.minimum}
							/>
						</div>
						<div class="space-y-2">
							<Label for={`band-max-${i}-${j}`}>คะแนนถึง {i + 1}.{j + 1}</Label><Input
								id={`band-max-${i}-${j}`}
								type="number"
								bind:value={band.maximum}
							/>
						</div>
						<div class="flex items-center gap-2">
							<Checkbox id={`band-urgent-${i}-${j}`} bind:checked={band.urgent} /><Label
								for={`band-urgent-${i}-${j}`}>แจ้งผู้รับผิดชอบให้ติดตามเร่งด่วน</Label
							>
						</div>
						<Button
							type="button"
							variant="ghost"
							onclick={() => scale.bands.splice(j, 1)}
							aria-label={`ลบช่วง ${i + 1}.${j + 1}`}>ลบช่วงคะแนน</Button
						>
					</div>{/each}
				<div class="flex flex-wrap gap-2">
					<Button
						type="button"
						variant="outline"
						onclick={() =>
							scale.bands.push({
								key: `band_${scale.bands.length + 1}`,
								label: '',
								minimum: (scale.bands.at(-1)?.maximum ?? -1) + 1,
								maximum: (scale.bands.at(-1)?.maximum ?? -1) + 1,
								urgent: false
							})}>เพิ่มช่วงคะแนนมาตรวัด {i + 1}</Button
					><Button type="button" variant="ghost" onclick={() => definition.scales.splice(i, 1)}
						>ลบมาตรวัด {i + 1}</Button
					>
				</div>
			</fieldset>
		{/each}
		<Button type="button" variant="outline" onclick={addScale}
			><Plus aria-hidden="true" />เพิ่มมาตรวัด</Button
		>
	</CareSection>
	{#if definition.mode === 'digital' || definition.questions.length}<CareSection
			title="4. คำถามและตัวเลือก"
			icon={MessageSquareText}
			id="care-editor-questions"
			description="คะแนนกลับทิศคำนวณจากคะแนนต่ำสุดและสูงสุดของตัวเลือก"
		>
			{#each definition.questions as question, i (i)}<fieldset
					class="space-y-3 rounded-xl border p-3"
				>
					<legend class="px-1 text-sm font-semibold">ข้อ {i + 1}</legend>
					<div class="space-y-2">
						<Label for={`question-text-${i}`}>คำถามข้อ {i + 1}</Label><Textarea
							id={`question-text-${i}`}
							bind:value={question.text}
						/>
					</div>
					<div class="grid gap-3 sm:grid-cols-2">
						<CareSelect
							id={`question-scale-${i}`}
							label={`มาตรวัดข้อ ${i + 1}`}
							value={question.scaleKey}
							options={definition.scales
								.filter((s) => !s.sourceScaleKeys.length)
								.map((s) => ({ value: s.key, label: s.label || s.key }))}
							onValueChange={(value) => (question.scaleKey = value)}
						/>
						<details class="space-y-2 rounded-lg border border-dashed p-3" open={showReferences}>
							<summary class="cursor-pointer text-sm text-muted-foreground"
								>รหัสอ้างอิงคำถาม</summary
							>
							<div class="mt-3 space-y-2">
								<Label for={`question-key-${i}`}>รหัสข้อ {i + 1}</Label><Input
									id={`question-key-${i}`}
									bind:value={question.key}
								/>
								<p class="text-xs text-muted-foreground">
									ใช้เชื่อมโยงการตั้งค่า รหัสต้องไม่ซ้ำภายในกลุ่มเดียวกัน
								</p>
							</div>
						</details>
					</div>

					<div class="flex items-center gap-2">
						<Checkbox id={`question-reverse-${i}`} bind:checked={question.reverse} /><Label
							for={`question-reverse-${i}`}>กลับทิศคะแนนข้อ {i + 1}</Label
						>
					</div>
					{#each question.options as option, j (j)}<div
							class="grid gap-3 rounded-lg bg-muted/40 p-3 sm:grid-cols-3"
						>
							<div class="space-y-2">
								<Label for={`option-label-${i}-${j}`}>ข้อความตัวเลือก {i + 1}.{j + 1}</Label><Input
									id={`option-label-${i}-${j}`}
									bind:value={option.label}
								/>
							</div>
							<details class="space-y-2 rounded-lg border border-dashed p-3" open={showReferences}>
								<summary class="cursor-pointer text-sm text-muted-foreground"
									>รหัสอ้างอิงตัวเลือก</summary
								>
								<div class="mt-3 space-y-2">
									<Label for={`option-key-${i}-${j}`}>รหัสตัวเลือก {i + 1}.{j + 1}</Label><Input
										id={`option-key-${i}-${j}`}
										bind:value={option.key}
									/>
									<p class="text-xs text-muted-foreground">
										ใช้เชื่อมโยงการตั้งค่า รหัสต้องไม่ซ้ำภายในกลุ่มเดียวกัน
									</p>
								</div>
							</details>
							<div class="space-y-2">
								<Label for={`option-points-${i}-${j}`}>คะแนนตัวเลือก {i + 1}.{j + 1}</Label><Input
									id={`option-points-${i}-${j}`}
									type="number"
									bind:value={option.points}
								/>
							</div>
							<div class="flex items-center gap-2 sm:col-span-2">
								<Checkbox id={`option-urgent-${i}-${j}`} bind:checked={option.urgent} /><Label
									for={`option-urgent-${i}-${j}`}>แจ้งผู้รับผิดชอบให้ติดตามเร่งด่วน</Label
								>
							</div>
							<Button type="button" variant="ghost" onclick={() => question.options.splice(j, 1)}
								>ลบตัวเลือก {i + 1}.{j + 1}</Button
							>
						</div>{/each}
					<div class="flex flex-wrap gap-2">
						<Button
							type="button"
							variant="outline"
							onclick={() =>
								question.options.push({
									key: `option_${question.options.length + 1}`,
									label: '',
									points: 0,
									urgent: false
								})}>เพิ่มตัวเลือกข้อ {i + 1}</Button
						><Button type="button" variant="ghost" onclick={() => definition.questions.splice(i, 1)}
							>ลบข้อ {i + 1}</Button
						>
					</div>
				</fieldset>{/each}
			<Button type="button" variant="outline" onclick={addQuestion}
				><Plus aria-hidden="true" />เพิ่มคำถาม</Button
			>
		</CareSection>{/if}
	<details class="rounded-xl border bg-card p-4" open={showTransfer}>
		<summary class="cursor-pointer text-sm font-medium">นำเข้า / ส่งออกการตั้งค่า</summary>
		<div class="mt-4 space-y-3">
			<p class="text-sm text-muted-foreground">
				สำหรับผู้ดูแลที่มีไฟล์การตั้งค่าที่ได้รับอนุญาตแล้ว ข้อมูลจะผ่านการตรวจสอบก่อนนำเข้า
			</p>
			<Label for="care-json">ข้อมูลการตั้งค่า JSON</Label><Textarea
				id="care-json"
				bind:value={json}
				rows={8}
			/>
			<div class="flex gap-2">
				<Button type="button" variant="outline" onclick={importDefinition}
					><Upload aria-hidden="true" />นำเข้าการตั้งค่า</Button
				><Button type="button" variant="outline" onclick={exportDefinition}
					><Download aria-hidden="true" />ส่งออกการตั้งค่า</Button
				>
			</div>
		</div>
	</details>
	{#if errors.length}<p class="text-sm text-destructive">
			ยังมีข้อมูลที่ต้องแก้ไข <a href="#care-editor-basic" class="underline underline-offset-4"
				>กลับไปตรวจสอบการตั้งค่าด้านบน</a
			>
		</p>{/if}
	{#if error}<PageState variant="error" title="บันทึกแบบประเมินไม่ได้" description={error} />{/if}
	<div class="flex justify-end gap-2">
		<Button href={careLink('/library', term)} variant="outline">ยกเลิก</Button><LoadingButton
			type="submit"
			loading={pending}
			disabled={!$can.has(PERMISSIONS.STUDENT_CARE_CONFIG_MANAGE_SCHOOL)}
			><Save aria-hidden="true" />บันทึกรุ่นใหม่</LoadingButton
		>
	</div>
</form>
