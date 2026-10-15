<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import {
		createSubjectGroup,
		updateSubjectGroup,
		listSubjectGroups,
		type SubjectGroup
	} from '#lib/api/academic-core.js';
	import { listOrganizationMembers, type OrganizationMemberItem } from '#lib/api/staff.js';
	import { requireApiData } from '#lib/api/client.js';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { captureRouteLoad, type RouteLoadResult } from '#lib/navigation/route-load.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Switch } from '#lib/components/ui/switch/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import {
		PageSkeleton,
		PageState,
		LoadingButton,
		RegionUpdatingState
	} from '#lib/components/app-state/index.js';
	import OrganizationMembersSection from './OrganizationMembersSection.svelte';
	import OrganizationPermissionDialog from './OrganizationPermissionDialog.svelte';
	import { Layers3, Plus, Pencil } from '@lucide/svelte';

	let { source }: { source: Promise<RouteLoadResult<SubjectGroup[]>> } = $props();
	let groups = $state<SubjectGroup[]>([]),
		loading = $state(true),
		ready = $state(false),
		error = $state('');
	const groupRequest = new LatestRequest(),
		memberRequest = new LatestRequest();
	let disposed = false;
	const url = $derived(page.shallow?.url ?? page.url);
	const selectedId = $derived(url.searchParams.get('group'));
	const selected = $derived(groups.find((group) => group.id === selectedId) ?? groups[0] ?? null);
	const detail = $derived(url.searchParams.get('section') ?? 'info');
	const canRead = $derived(
		$can.hasAny(
			PERMISSIONS.ROLES_READ_ALL,
			PERMISSIONS.ACADEMIC_CATALOG_READ_SCHOOL,
			PERMISSIONS.ACADEMIC_CATALOG_MANAGE_SCHOOL
		)
	);
	const canCreate = $derived(
		$can.hasAny(PERMISSIONS.ROLES_CREATE_ALL, PERMISSIONS.ACADEMIC_CATALOG_MANAGE_SCHOOL)
	);
	const canEdit = $derived(
		$can.hasAny(PERMISSIONS.ROLES_UPDATE_ALL, PERMISSIONS.ACADEMIC_CATALOG_MANAGE_SCHOOL)
	);
	const canDeactivate = $derived(
		$can.has(PERMISSIONS.ACADEMIC_CATALOG_MANAGE_SCHOOL) ||
			$can.hasAll(PERMISSIONS.ROLES_UPDATE_ALL, PERMISSIONS.ROLES_DELETE_ALL)
	);
	const canReadGrants = $derived($can.has(PERMISSIONS.ROLES_READ_ALL));
	let search = $state('');
	const visibleGroups = $derived(
		groups.filter((group) =>
			`${group.nameTh} ${group.nameEn}`.toLowerCase().includes(search.trim().toLowerCase())
		)
	);
	let dialogOpen = $state(false),
		editing = $state<SubjectGroup | null>(null),
		saving = $state(false),
		formError = $state('');
	let draft = $state({ nameTh: '', nameEn: '', displayOrder: 1, isActive: true });
	let permissionOpen = $state(false);
	let members = $state<OrganizationMemberItem[]>([]),
		membersLoading = $state(false),
		membersReady = $state(false),
		membersError = $state('');

	function navigate(group: string, section = 'info') {
		const next = new URL(url.href);
		next.searchParams.set('group', group);
		next.searchParams.set('section', section);
		void goto(next.pathname + next.search, { shallow: true });
	}
	function applyGroups(result: RouteLoadResult<SubjectGroup[]>, revision: number) {
		if (!groupRequest.isCurrent(revision) || !canRead) return;
		loading = false;
		if (result.ok) {
			groups = result.data;
			ready = true;
			error = '';
		} else error = result.error;
	}
	async function refresh() {
		if (!canRead) return;
		const ticket = groupRequest.begin();
		loading = true;
		error = '';
		applyGroups(
			await captureRouteLoad(
				listSubjectGroups({ signal: ticket.signal }),
				'โหลดกลุ่มสาระไม่สำเร็จ'
			),
			ticket.revision
		);
	}
	$effect.pre(() => {
		const pending = source,
			allowed = canRead;
		untrack(() => {
			const ticket = groupRequest.begin();
			loading = allowed;
			error = '';
			if (!allowed) {
				groups = [];
				ready = false;
				dialogOpen = false;
				return;
			}
			void pending.then((result) => applyGroups(result, ticket.revision));
		});
		return () => groupRequest.abort();
	});
	async function refreshMembers() {
		const owner = selected?.organizationUnitId;
		if (!owner || !canRead || detail !== 'members') return;
		const ticket = memberRequest.begin();
		membersLoading = true;
		membersError = '';
		const result = await captureRouteLoad(
			listOrganizationMembers(owner, undefined, { signal: ticket.signal }).then((reply) =>
				requireApiData(reply, 'โหลดสมาชิกไม่สำเร็จ')
			),
			'โหลดสมาชิกไม่สำเร็จ'
		);
		if (
			!memberRequest.isCurrent(ticket.revision) ||
			selected?.organizationUnitId !== owner ||
			!canRead
		)
			return;
		membersLoading = false;
		if (result.ok) {
			members = result.data;
			membersReady = true;
		} else membersError = result.error;
	}
	$effect.pre(() => {
		const owner = selected?.organizationUnitId,
			section = detail,
			allowed = canRead;
		untrack(() => {
			memberRequest.abort();
			members = [];
			membersReady = false;
			membersError = '';
			membersLoading = false;
			permissionOpen = false;
			if (owner && allowed && section === 'members') {
				membersLoading = true;
				void refreshMembers();
			}
		});
		return () => memberRequest.abort();
	});
	function edit(group: SubjectGroup | null) {
		if (group ? !canEdit : !canCreate) return;
		editing = group;
		formError = '';
		draft = group
			? {
					nameTh: group.nameTh,
					nameEn: group.nameEn,
					displayOrder: group.displayOrder ?? 0,
					isActive: group.isActive ?? true
				}
			: {
					nameTh: '',
					nameEn: '',
					displayOrder: Math.max(0, ...groups.map((item) => item.displayOrder ?? 0)) + 1,
					isActive: true
				};
		dialogOpen = true;
	}
	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (saving || !dialogOpen || (editing ? !canEdit : !canCreate)) return;
		if (!draft.nameTh.trim()) {
			formError = 'กรุณาระบุชื่อกลุ่มสาระ';
			return;
		}
		const target = editing;
		saving = true;
		formError = '';
		groupRequest.abort();
		loading = false;
		try {
			const group = target
				? await updateSubjectGroup(target.id, {
						...draft,
						isActive: draft.isActive === target.isActive ? undefined : draft.isActive,
						rowVersion: target.rowVersion
					})
				: await createSubjectGroup({ nameTh: draft.nameTh, nameEn: draft.nameEn });
			if (disposed || !canRead) return;
			const current = groups.find((item) => item.id === group.id);
			const authoritative = current && current.rowVersion > group.rowVersion ? current : group;
			groups = [...groups.filter((item) => item.id !== group.id), authoritative].sort(
				(a, b) => (a.displayOrder ?? 0) - (b.displayOrder ?? 0) || a.id.localeCompare(b.id)
			);
			dialogOpen = false;
			navigate(group.id);
		} catch (cause) {
			if (!disposed)
				formError = cause instanceof Error ? cause.message : 'บันทึกกลุ่มสาระไม่สำเร็จ';
		} finally {
			if (!disposed) saving = false;
		}
	}
	onDestroy(() => {
		disposed = true;
		groupRequest.abort();
		memberRequest.abort();
	});
</script>

<section aria-label="กลุ่มสาระการเรียนรู้" aria-busy={loading}>
	<div class="mb-4 flex flex-wrap items-center justify-between gap-3">
		<h2 class="font-semibold">กลุ่มสาระการเรียนรู้</h2>
		<div class="flex gap-2">
			<Button variant="outline" onclick={refresh} disabled={loading}>รีเฟรช</Button>
			{#if canCreate}<Button onclick={() => edit(null)}
					><Plus class="size-4" /> เพิ่มกลุ่มสาระ</Button
				>{/if}
		</div>
	</div>
	{#if loading && !ready}<PageSkeleton variant="cards" rows={3} />
	{:else if !ready && error}<PageState
			variant="error"
			title="โหลดกลุ่มสาระไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={refresh}
		/>
	{:else if ready}
		<div class="relative space-y-4" data-testid="catalog-subject-groups-ready">
			{#if loading}<RegionUpdatingState label="กำลังอัปเดตกลุ่มสาระ" />{/if}
			{#if error}<PageState
					variant="error"
					title="อัปเดตกลุ่มสาระไม่สำเร็จ"
					description={error}
					actionLabel="ลองอีกครั้ง"
					onaction={refresh}
				/>{/if}
			<div class="rounded-xl border bg-card p-3 sm:p-4">
				<Label for="learning-area-search">ค้นหากลุ่มสาระ</Label><Input
					id="learning-area-search"
					bind:value={search}
					placeholder="ชื่อกลุ่มสาระ"
					class="mt-2"
				/>
			</div>
			<div class="grid items-start gap-4 lg:grid-cols-[minmax(220px,1fr)_minmax(0,2fr)]">
				<div class="grid gap-2" aria-label="รายการกลุ่มสาระ">
					{#each visibleGroups as group (group.id)}
						<Button
							variant={selected?.id === group.id ? 'secondary' : 'outline'}
							class="h-auto justify-start whitespace-normal py-3 text-left"
							onclick={() => navigate(group.id)}
							aria-pressed={selected?.id === group.id}
						>
							<Layers3 class="size-4 shrink-0" /><span class="min-w-0 break-words"
								>{group.nameTh}{#if !group.isActive}<span class="ml-2 text-xs text-muted-foreground"
										>ปิดใช้งาน</span
									>{/if}</span
							>
						</Button>
					{:else}<PageState
							title={groups.length ? 'ไม่พบกลุ่มสาระที่ค้นหา' : 'ยังไม่มีกลุ่มสาระ'}
							description="เพิ่มกลุ่มสาระเพื่อเชื่อมรายวิชาและจัดสมาชิก"
						/>{/each}
				</div>
				{#if selected}<section
						class="min-w-0 space-y-4 rounded-xl border bg-card p-4 sm:p-5"
						aria-label="รายละเอียดกลุ่มสาระ"
					>
						<div class="flex items-start justify-between gap-3">
							<div class="min-w-0">
								<h3 class="break-words font-semibold">{selected.nameTh}</h3>
								<p class="mt-1 text-sm text-muted-foreground">{selected.nameEn}</p>
							</div>
							<Badge variant={selected.isActive ? 'secondary' : 'outline'}
								>{selected.isActive ? 'เปิดใช้งาน' : 'ปิดใช้งาน'}</Badge
							>
						</div>
						<nav class="flex flex-wrap gap-2" aria-label="ข้อมูลกลุ่มสาระ">
							<Button
								size="sm"
								variant={detail === 'info' ? 'secondary' : 'ghost'}
								onclick={() => navigate(selected.id, 'info')}>ข้อมูล</Button
							>
							<Button
								size="sm"
								variant={detail === 'members' ? 'secondary' : 'ghost'}
								onclick={() => navigate(selected.id, 'members')}>หัวหน้าและสมาชิก</Button
							>
							{#if canReadGrants}<Button
									size="sm"
									variant={detail === 'permissions' ? 'secondary' : 'ghost'}
									onclick={() => navigate(selected.id, 'permissions')}>สิทธิ์</Button
								>{/if}
						</nav>
						{#if detail === 'members'}
							<OrganizationMembersSection
								organizationUnitId={selected.organizationUnitId}
								canAssignMembers={!!selected.isActive && $can.has(PERMISSIONS.ROLES_ASSIGN_ALL)}
								{members}
								loadingMembers={membersLoading}
								membersLoaded={membersReady}
								{membersError}
								onRefresh={refreshMembers}
								onChanged={refreshMembers}
							/>
						{:else if detail === 'permissions' && canReadGrants}
							<p class="text-sm text-muted-foreground">
								กำหนดสิทธิ์สำหรับหัวหน้า รองหัวหน้า ผู้ประสานงาน และสมาชิกของกลุ่มสาระนี้
							</p>
							<Button variant="outline" onclick={() => (permissionOpen = true)}
								>{$can.has(PERMISSIONS.ROLES_UPDATE_ALL)
									? 'กำหนดสิทธิ์ตามตำแหน่ง'
									: 'ดูสิทธิ์ตามตำแหน่ง'}</Button
							>
						{:else}
							<dl class="grid gap-3 text-sm sm:grid-cols-2">
								<div>
									<dt class="text-muted-foreground">ชื่อภาษาอังกฤษ</dt>
									<dd class="mt-1 break-words">{selected.nameEn || 'ไม่ได้ระบุ'}</dd>
								</div>
								<div>
									<dt class="text-muted-foreground">ลำดับ</dt>
									<dd class="mt-1">{selected.displayOrder}</dd>
								</div>
							</dl>
							{#if canEdit}<Button size="sm" variant="outline" onclick={() => edit(selected)}
									><Pencil class="size-4" /> แก้ไขข้อมูล</Button
								>{/if}
						{/if}
					</section>{/if}
			</div>
		</div>
	{/if}
</section>

<Dialog.Root
	bind:open={
		() => dialogOpen,
		(next) => {
			if (!saving) dialogOpen = next;
		}
	}
>
	<Dialog.Content
		class="max-h-[90dvh] overflow-y-auto sm:max-w-lg"
		onInteractOutside={(event) => {
			if (saving) event.preventDefault();
		}}
		onEscapeKeydown={(event) => {
			if (saving) event.preventDefault();
		}}
	>
		<Dialog.Header
			><Dialog.Title>{editing ? 'แก้ไขกลุ่มสาระ' : 'เพิ่มกลุ่มสาระ'}</Dialog.Title
			><Dialog.Description>ระบุชื่อกลุ่มสาระเพื่อใช้ในทะเบียนรายวิชาและจัดสมาชิก</Dialog.Description
			></Dialog.Header
		>
		<form onsubmit={submit} class="space-y-4">
			<div class="space-y-2">
				<Label for="area-name">ชื่อกลุ่มสาระ *</Label><Input
					id="area-name"
					bind:value={draft.nameTh}
					required
					maxlength={200}
					disabled={saving}
					aria-invalid={!!formError}
				/>
			</div>
			<div class="space-y-2">
				<Label for="area-name-en">ชื่อภาษาอังกฤษ</Label><Input
					id="area-name-en"
					bind:value={draft.nameEn}
					maxlength={200}
					disabled={saving}
				/>
			</div>
			{#if editing}<div class="space-y-2">
					<Label for="area-order">ลำดับ</Label><Input
						id="area-order"
						type="number"
						min={0}
						step={1}
						bind:value={draft.displayOrder}
						disabled={saving}
					/>
				</div>
				{#if editing.code !== 'AC'}<div class="flex items-center gap-2">
						<Switch
							id="area-status"
							bind:checked={draft.isActive}
							disabled={saving || (!!editing.isActive && !canDeactivate)}
						/><Label for="area-status">เปิดใช้งาน</Label>
					</div>{/if}
			{/if}
			{#if formError}<p role="alert" class="text-sm text-destructive">{formError}</p>{/if}
			<Dialog.Footer
				><Button
					variant="outline"
					type="button"
					disabled={saving}
					onclick={() => (dialogOpen = false)}>ยกเลิก</Button
				><LoadingButton type="submit" loading={saving}>บันทึก</LoadingButton></Dialog.Footer
			>
		</form>
	</Dialog.Content>
</Dialog.Root>
{#if selected && canReadGrants}<OrganizationPermissionDialog
		bind:open={permissionOpen}
		organizationUnit={{ id: selected.organizationUnitId }}
		readOnly={!$can.has(PERMISSIONS.ROLES_UPDATE_ALL)}
	/>{/if}
