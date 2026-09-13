<script lang="ts">
	import { untrack } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import { Plus, Trash2 } from 'lucide-svelte';
	import { Button } from '$lib/components/ui/button';
	import { Label } from '$lib/components/ui/label';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Select from '$lib/components/ui/select';
	import { LoadingButton, PageState } from '$lib/components/app-state';
	import { can } from '$lib/stores/permissions';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import {
		replacePromotionProgressions,
		type GradeProgressionInput,
		type GradeProgressionSet,
		type PromotionPolicyOptions
	} from '$lib/api/academic-promotion';
	import { ApiClientError } from '$lib/api/client';

	type Choice = { value: string; label: string };
	let {
		options,
		grades,
		onclose,
		onupdated
	}: {
		options: PromotionPolicyOptions;
		grades: Choice[];
		onclose: () => void;
		onupdated: (set: GradeProgressionSet) => void;
	} = $props();
	// A dialog edits one explicit optimistic revision, not live-changing props.
	const initial = untrack(() => options.progressionSet);
	let rows = $state(
		initial.progressions.map((mapping) => ({
			key: mapping.id,
			input: {
				fromGradeLevelId: mapping.fromGradeLevelId,
				toGradeLevelId: mapping.toGradeLevelId ?? null,
				transitionKind: mapping.transitionKind,
				curriculumId: mapping.curriculumId ?? null,
				isActive: mapping.isActive
			} satisfies GradeProgressionInput
		}))
	);
	let saving = $state(false);
	let error = $state('');
	let conflict = $state(false);
	const canManage = $derived($can.has(PERMISSIONS.ACADEMIC_YEAR_MANAGE_SCHOOL));
	const kinds: Choice[] = [
		{ value: 'promote', label: 'เลื่อนชั้น' },
		{ value: 'repeat', label: 'ซ้ำชั้น' },
		{ value: 'graduate', label: 'จบการศึกษา' },
		{ value: 'exception', label: 'กรณียกเว้น' }
	];
	const curricula = $derived.by(() => {
		const labels = new SvelteMap<string, string>();
		for (const program of options.programs)
			labels.set(program.curriculumId, program.curriculumName);
		return [
			{ value: 'all', label: 'ทุกหลักสูตร' },
			...Array.from(labels, ([value, label]) => ({ value, label }))
		];
	});
	const keys = $derived(
		rows.map(
			({ input }) =>
				`${input.fromGradeLevelId}/${input.toGradeLevelId ?? ''}/${input.transitionKind}/${input.curriculumId ?? ''}`
		)
	);
	const valid = $derived(
		canManage &&
			!saving &&
			!conflict &&
			rows.length > 0 &&
			rows.length <= 500 &&
			new Set(keys).size === keys.length &&
			rows.every(
				({ input }) =>
					!!input.fromGradeLevelId &&
					(input.transitionKind === 'graduate' ? !input.toGradeLevelId : !!input.toGradeLevelId) &&
					(input.transitionKind !== 'promote' || input.fromGradeLevelId !== input.toGradeLevelId)
			)
	);
	function add() {
		rows.push({
			key: crypto.randomUUID(),
			input: {
				fromGradeLevelId: '',
				toGradeLevelId: null,
				transitionKind: 'promote',
				curriculumId: null,
				isActive: true
			}
		});
	}
	async function save() {
		if (!valid) return;
		saving = true;
		error = '';
		try {
			const result = await replacePromotionProgressions({
				rowVersion: initial.rowVersion,
				progressions: rows.map((row) => ({ ...row.input }))
			});
			onupdated(result);
			onclose();
		} catch (problem) {
			error = problem instanceof Error ? problem.message : 'บันทึกลำดับชั้นไม่สำเร็จ';
			conflict = problem instanceof ApiClientError && problem.status === 409;
		} finally {
			saving = false;
		}
	}
</script>

{#snippet choice(title: string, value: string, items: Choice[], change: (value: string) => void)}
	<div class="space-y-1.5">
		<Label>{title}</Label><Select.Root type="single" {value} onValueChange={change}>
			<Select.Trigger class="w-full" aria-label={title}
				><span class="truncate"
					>{items.find((item) => item.value === value)?.label ?? 'เลือกข้อมูล'}</span
				></Select.Trigger
			>
			<Select.Content
				>{#each items as item (item.value)}<Select.Item value={item.value} label={item.label}
						>{item.label}</Select.Item
					>{/each}</Select.Content
			>
		</Select.Root>
	</div>
{/snippet}

<Dialog.Root
	open
	onOpenChange={(open) => {
		if (!open && !saving) onclose();
	}}
>
	<Dialog.Content
		class="max-h-[85dvh] overflow-y-auto sm:max-w-3xl"
		showCloseButton={!saving}
		onEscapeKeydown={(event) => {
			if (saving) event.preventDefault();
		}}
		onInteractOutside={(event) => {
			if (saving) event.preventDefault();
		}}
	>
		<Dialog.Header
			><Dialog.Title>จัดการลำดับชั้น</Dialog.Title><Dialog.Description
				>บันทึกกฎชุดนี้แทนการตั้งค่าปัจจุบันของโรงเรียน ไม่แก้เกณฑ์ที่ยืนยันไปแล้ว
				และยังไม่เลื่อนชั้นนักเรียน</Dialog.Description
			></Dialog.Header
		>
		{#if error}<PageState variant="error" title="บันทึกไม่ได้" description={error} />{/if}
		{#if conflict}<p class="text-destructive text-sm">
				มีผู้แก้ไขลำดับชั้นแล้ว กรุณาปิดและโหลดข้อมูลล่าสุดก่อนแก้ไขใหม่
			</p>{/if}
		{#each rows as row, index (row.key)}
			<fieldset class="rounded-xl border p-3">
				<legend class="px-1 text-sm font-medium">กฎ {index + 1}</legend>
				<div class="grid gap-3 sm:grid-cols-2">
					{@render choice(
						`ชั้นต้นทางของกฎ ${index + 1}`,
						row.input.fromGradeLevelId,
						grades,
						(value) => (row.input.fromGradeLevelId = value)
					)}
					{@render choice(`ประเภทกฎ ${index + 1}`, row.input.transitionKind, kinds, (value) => {
						if (
							value === 'promote' ||
							value === 'repeat' ||
							value === 'graduate' ||
							value === 'exception'
						) {
							row.input.transitionKind = value;
							if (value === 'graduate') row.input.toGradeLevelId = null;
						}
					})}
					{#if row.input.transitionKind !== 'graduate'}{@render choice(
							`ชั้นปลายทางของกฎ ${index + 1}`,
							row.input.toGradeLevelId ?? '',
							grades,
							(value) => (row.input.toGradeLevelId = value)
						)}{/if}
					{@render choice(
						`หลักสูตรของกฎ ${index + 1}`,
						row.input.curriculumId ?? 'all',
						curricula,
						(value) => (row.input.curriculumId = value === 'all' ? null : value)
					)}
				</div>
				<div class="mt-3 flex items-center justify-between">
					<div class="flex items-center gap-2">
						<Checkbox id={`active-${row.key}`} bind:checked={row.input.isActive} /><Label
							for={`active-${row.key}`}>ใช้กฎนี้</Label
						>
					</div>
					<Button
						variant="ghost"
						size="icon"
						aria-label={`ลบกฎ ${index + 1}`}
						disabled={rows.length === 1 || saving}
						onclick={() => (rows = rows.filter((item) => item.key !== row.key))}
						><Trash2 class="size-4" /></Button
					>
				</div>
			</fieldset>
		{/each}
		<Button variant="outline" disabled={rows.length >= 500 || saving} onclick={add}
			><Plus class="size-4" />เพิ่มกฎลำดับชั้น</Button
		>
		<Dialog.Footer
			><Button variant="outline" disabled={saving} onclick={onclose}>ยกเลิก</Button><LoadingButton
				loading={saving}
				disabled={!valid}
				onclick={save}>บันทึกลำดับชั้น</LoadingButton
			></Dialog.Footer
		>
	</Dialog.Content>
</Dialog.Root>
