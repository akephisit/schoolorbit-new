<script lang="ts">
	import { untrack } from 'svelte';
	import { formatEffectiveResultValue } from '$lib/academic/results/presentation';
	import type {
		AcademicResultCorrectionInput,
		EffectiveResultSearchItem
	} from '$lib/api/academicResults';
	import { LoadingButton } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import { ArrowRight, History } from 'lucide-svelte';

	let {
		open,
		item,
		busy = false,
		errorMessage = '',
		onopenchange,
		oncorrect,
		onrefresh
	}: {
		open: boolean;
		item: EffectiveResultSearchItem;
		busy?: boolean;
		errorMessage?: string;
		onopenchange: (open: boolean) => void;
		oncorrect: (input: AcademicResultCorrectionInput) => void;
		onrefresh: () => void;
	} = $props();

	let selectedValue = $state(untrack(() => optionValue(item)));

	function optionValue(target: EffectiveResultSearchItem): string {
		const value = target.result.effective;
		if (value.kind === 'course') {
			if (value.outcome === 'incomplete') return 'incomplete';
			if (value.outcome === 'insufficient_attendance') return 'insufficient_attendance';
			return `numeric:${value.numericGrade ?? '0'}`;
		}
		if (value.kind === 'activity') return value.outcome;
		return `${value.qualityLevel}`;
	}

	function correctionInput(): AcademicResultCorrectionInput | null {
		if (selectedValue === optionValue(item)) return null;
		const expectedEffectiveVersion = item.result.effectiveVersion;
		if (item.kind === 'course') {
			if (selectedValue.startsWith('numeric:')) {
				return {
					kind: 'course',
					courseResultId: item.result.resultId,
					outcome: 'numeric',
					numericGrade: selectedValue.slice('numeric:'.length),
					expectedEffectiveVersion
				};
			}
			return {
				kind: 'course',
				courseResultId: item.result.resultId,
				outcome: selectedValue as 'incomplete' | 'insufficient_attendance',
				numericGrade: null,
				expectedEffectiveVersion
			};
		}
		if (item.kind === 'activity') {
			return {
				kind: 'activity',
				activityResultId: item.result.resultId,
				outcome: selectedValue as 'pass' | 'fail',
				expectedEffectiveVersion
			};
		}
		return {
			kind: 'learner_evaluation',
			subjectStudentEvaluationId: item.result.resultId,
			qualityLevel: Number(selectedValue),
			expectedEffectiveVersion
		};
	}

	function selectedOptionLabel(): string {
		if (item.kind === 'course') {
			if (selectedValue === 'incomplete') return 'ร';
			if (selectedValue === 'insufficient_attendance') return 'มส';
			return selectedValue.startsWith('numeric:')
				? selectedValue.slice('numeric:'.length)
				: 'เลือกผลใหม่';
		}
		if (item.kind === 'activity') return selectedValue === 'pass' ? 'ผ · ผ่าน' : 'มผ · ไม่ผ่าน';
		return `${selectedValue} · ${['ไม่ผ่าน', 'ผ่าน', 'ดี', 'ดีเยี่ยม'][Number(selectedValue)] ?? ''}`;
	}

	function submit(event: SubmitEvent) {
		event.preventDefault();
		const input = correctionInput();
		if (input) oncorrect(input);
	}
</script>

<Dialog.Root {open} onOpenChange={onopenchange}>
	<Dialog.Content class="max-h-[90vh] overflow-y-auto sm:max-w-xl">
		<form class="space-y-5" onsubmit={submit}>
			<Dialog.Header>
				<Dialog.Title>แก้ผลการเรียน</Dialog.Title>
				<Dialog.Description
					>{item.displayName} · {item.offeringCode} · {item.groupName}</Dialog.Description
				>
			</Dialog.Header>

			<div class="grid grid-cols-[1fr_auto_1fr] items-center gap-3">
				<div class="rounded-lg border bg-muted/20 p-3">
					<p class="text-xs text-muted-foreground">ผลเริ่มต้น</p>
					<p class="mt-1 text-lg font-semibold">
						{formatEffectiveResultValue(item.result.initial)}
					</p>
				</div>
				<ArrowRight class="size-4 text-muted-foreground" />
				<div class="rounded-lg border border-primary/20 bg-primary/5 p-3">
					<p class="text-xs text-muted-foreground">ผลที่ใช้ปัจจุบัน</p>
					<p class="mt-1 text-lg font-semibold">
						{formatEffectiveResultValue(item.result.effective)}
					</p>
				</div>
			</div>

			<div class="space-y-1.5">
				<Label for="corrected-result">ผลใหม่</Label>
				<Select.Root type="single" bind:value={selectedValue} disabled={busy}>
					<Select.Trigger id="corrected-result" class="w-full"
						>{selectedOptionLabel()}</Select.Trigger
					>
					<Select.Content>
						{#if item.kind === 'course'}
							{#each ['0', '0.5', '1', '1.5', '2', '2.5', '3', '3.5', '4'] as grade (grade)}
								<Select.Item value={`numeric:${grade}`}>{grade}</Select.Item>
							{/each}
							<Select.Item value="incomplete">ร</Select.Item>
							<Select.Item value="insufficient_attendance">มส</Select.Item>
						{:else if item.kind === 'activity'}
							<Select.Item value="pass">ผ · ผ่าน</Select.Item>
							<Select.Item value="fail">มผ · ไม่ผ่าน</Select.Item>
						{:else}
							<Select.Item value="3">3 · ดีเยี่ยม</Select.Item>
							<Select.Item value="2">2 · ดี</Select.Item>
							<Select.Item value="1">1 · ผ่าน</Select.Item>
							<Select.Item value="0">0 · ไม่ผ่าน</Select.Item>
						{/if}
					</Select.Content>
				</Select.Root>
			</div>

			{#if errorMessage}
				<div
					class="rounded-lg border border-destructive/30 bg-destructive/5 p-3 text-sm text-destructive"
				>
					<p>{errorMessage}</p>
					<Button type="button" variant="outline" size="sm" class="mt-2" onclick={onrefresh}
						>โหลดข้อมูลล่าสุด</Button
					>
				</div>
			{/if}

			<div class="space-y-2">
				<p class="flex items-center gap-2 text-sm font-medium">
					<History class="size-4" /> ประวัติการแก้ไข
				</p>
				{#if item.result.corrections.length > 0}
					<div class="space-y-2">
						{#each item.result.corrections as correction (correction.id)}
							<div class="rounded-lg border p-3 text-sm">
								<p>
									{formatEffectiveResultValue(correction.previous)} →
									<strong>{formatEffectiveResultValue(correction.corrected)}</strong>
								</p>
								<p class="mt-1 text-xs text-muted-foreground">
									{new Date(correction.correctedAt).toLocaleString('th-TH')}
								</p>
							</div>
						{/each}
					</div>
				{:else}
					<p class="rounded-lg border border-dashed p-3 text-sm text-muted-foreground">
						ยังไม่เคยแก้ไข ผลปัจจุบันยังเป็นผลเริ่มต้น
					</p>
				{/if}
			</div>

			<Dialog.Footer>
				<Button type="button" variant="outline" disabled={busy} onclick={() => onopenchange(false)}
					>ยกเลิก</Button
				>
				<LoadingButton type="submit" loading={busy} disabled={correctionInput() === null}
					>บันทึกผลใหม่</LoadingButton
				>
			</Dialog.Footer>
		</form>
	</Dialog.Content>
</Dialog.Root>
