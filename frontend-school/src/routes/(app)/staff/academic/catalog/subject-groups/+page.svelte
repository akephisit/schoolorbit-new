<script lang="ts">
	import { invalidate } from '$app/navigation';
	import { untrack } from 'svelte';
	import {
		createSubjectGroup,
		deleteSubjectGroup,
		updateSubjectGroup,
		type SubjectGroup
	} from '$lib/api/academic-core';
	import { CATALOG_SUBJECT_GROUPS_DEPENDENCY } from '$lib/academic-core/catalog-route';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState, RegionUpdatingState } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import { Layers3, Plus, Save, Trash2 } from '@lucide/svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	let groups = $state<SubjectGroup[]>([]);
	let loading = $state(true);
	let groupsReady = $state(false);
	let errorMessage = $state('');
	let draft = $state({ code: '', nameTh: '', nameEn: '', displayOrder: 1, isActive: true });
	let editing = $state<SubjectGroup | null>(null);
	const canManage = $derived(
		$can.hasAny(
			PERMISSIONS.ACADEMIC_CATALOG_MANAGE_SCHOOL,
			PERMISSIONS.ACADEMIC_CATALOG_MANAGE_ORGANIZATION_TREE,
			PERMISSIONS.ACADEMIC_CATALOG_MANAGE_ORGANIZATION_UNIT
		)
	);

	function loadGroups() {
		return invalidate(CATALOG_SUBJECT_GROUPS_DEPENDENCY);
	}

	$effect.pre(() => {
		const routeResult = data.groups;
		let current = true;
		untrack(() => {
			loading = Boolean(routeResult);
			errorMessage = '';
		});
		if (routeResult) {
			void routeResult.then((result) => {
				if (!current) return;
				untrack(() => {
					if (result.ok) {
						groups = [...result.data].sort((a, b) => (a.displayOrder ?? 0) - (b.displayOrder ?? 0));
						groupsReady = true;
					} else errorMessage = result.error;
					loading = false;
				});
			});
		}
		return () => {
			current = false;
		};
	});

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		try {
			if (editing) {
				const updated = await updateSubjectGroup(editing.id, {
					...draft,
					rowVersion: editing.rowVersion
				});
				groups = groups
					.map((group) => (group.id === updated.id ? updated : group))
					.sort((a, b) => (a.displayOrder ?? 0) - (b.displayOrder ?? 0));
			} else {
				groups = [...groups, await createSubjectGroup(draft)].sort(
					(a, b) => (a.displayOrder ?? 0) - (b.displayOrder ?? 0)
				);
			}
			draft = { code: '', nameTh: '', nameEn: '', displayOrder: groups.length + 1, isActive: true };
			editing = null;
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'บันทึกกลุ่มสาระไม่สำเร็จ';
		}
	}

	function startEdit(group: SubjectGroup) {
		editing = group;
		draft = {
			code: group.code,
			nameTh: group.nameTh,
			nameEn: group.nameEn,
			displayOrder: group.displayOrder ?? 0,
			isActive: group.isActive ?? false
		};
	}

	async function remove(group: SubjectGroup) {
		if (!confirm(`ลบกลุ่มสาระ ${group.nameTh}?`)) return;
		try {
			await deleteSubjectGroup(group.id);
			groups = groups.filter((item) => item.id !== group.id);
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'ลบกลุ่มสาระไม่สำเร็จ';
		}
	}
</script>

<PageShell
	title="กลุ่มสาระการเรียนรู้"
	description="ทะเบียนกลุ่มสาระส่วนกลางสำหรับเชื่อมรายวิชา หน่วยงาน และรายงาน"
>
	<section aria-label="กลุ่มสาระการเรียนรู้" aria-busy={loading}>
		{#if loading && !groupsReady}<PageSkeleton
				variant="cards"
				rows={5}
			/>{:else if errorMessage && !groupsReady}<PageState
				variant="error"
				title="โหลดกลุ่มสาระไม่สำเร็จ"
				description={errorMessage}
				actionLabel="ลองอีกครั้ง"
				onaction={loadGroups}
			/>{:else}
			<div
				class="relative grid gap-5 xl:grid-cols-[minmax(0,1fr)_340px]"
				data-testid="catalog-subject-groups-ready"
			>
				{#if loading}<RegionUpdatingState label="กำลังอัปเดตกลุ่มสาระ" />{/if}
				<section class="grid content-start gap-3 md:grid-cols-2">
					{#each groups as group (group.id)}<article
							class="rounded-xl border bg-card p-5 shadow-sm"
						>
							<div class="flex items-start justify-between">
								<div class="rounded-lg bg-primary/10 p-2 text-primary">
									<Layers3 class="size-5" />
								</div>
								<span class="text-sm font-semibold tabular-nums">#{group.displayOrder}</span>
							</div>
							<h2 class="mt-4 font-semibold">{group.nameTh}</h2>
							<p class="text-xs text-muted-foreground">{group.code} · {group.nameEn}</p>
							{#if canManage}<div class="mt-4 flex gap-2 border-t pt-3">
									<Button size="sm" variant="outline" onclick={() => startEdit(group)}>แก้ไข</Button
									><Button
										size="sm"
										variant="ghost"
										class="text-destructive"
										onclick={() => remove(group)}><Trash2 class="size-4" /> ลบ</Button
									>
								</div>{/if}
						</article>{:else}<div
							class="col-span-full rounded-xl border border-dashed p-10 text-center text-sm text-muted-foreground"
						>
							ยังไม่มีกลุ่มสาระ
						</div>{/each}
				</section>
				{#if canManage}<form
						class="space-y-3 rounded-xl border bg-card p-5 shadow-sm"
						onsubmit={submit}
					>
						<h2 class="font-semibold">{editing ? 'แก้ไขกลุ่มสาระ' : 'เพิ่มกลุ่มสาระ'}</h2>
						<div class="grid grid-cols-2 gap-3">
							<div class="space-y-1.5">
								<Label for="group-code">รหัส</Label><Input
									id="group-code"
									bind:value={draft.code}
									required
								/>
							</div>
							<div class="space-y-1.5">
								<Label for="group-order">ลำดับ</Label><Input
									id="group-order"
									type="number"
									min="1"
									bind:value={draft.displayOrder}
									required
								/>
							</div>
						</div>
						<div class="space-y-1.5">
							<Label for="group-name-th">ชื่อภาษาไทย</Label><Input
								id="group-name-th"
								bind:value={draft.nameTh}
								required
							/>
						</div>
						<div class="space-y-1.5">
							<Label for="group-name-en">ชื่อภาษาอังกฤษ</Label><Input
								id="group-name-en"
								bind:value={draft.nameEn}
								required
							/>
						</div>
						<label class="flex items-center gap-2 text-sm"
							><input type="checkbox" bind:checked={draft.isActive} /> เปิดใช้งาน</label
						><Button class="w-full" type="submit"
							>{#if editing}<Save class="size-4" /> บันทึก{:else}<Plus class="size-4" /> เพิ่มกลุ่มสาระ{/if}</Button
						>
					</form>{/if}
			</div>
			{#if errorMessage}<div
					role="alert"
					class="flex flex-wrap items-center gap-2 text-sm text-destructive"
				>
					<p>{errorMessage}</p>
					<Button size="sm" variant="outline" onclick={loadGroups}>ลองอีกครั้ง</Button>
				</div>{/if}
		{/if}
	</section>
</PageShell>
