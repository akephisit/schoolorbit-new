<script lang="ts">
	import { onDestroy, tick, untrack } from 'svelte';
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
	import * as Card from '#lib/components/ui/card/index.js';
	import * as ToggleGroup from '#lib/components/ui/toggle-group/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { PageSkeleton, PageState, LoadingButton } from '#lib/components/app-state/index.js';
	import OrganizationMembersSection from './OrganizationMembersSection.svelte';
	import OrganizationPermissionDialog from './OrganizationPermissionDialog.svelte';
	import {
		Layers3,
		Plus,
		Pencil,
		ArrowLeft,
		ChevronRight,
		Search,
		ShieldCheck
	} from '@lucide/svelte';

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
	const selected = $derived(
		groups.find((group) => group.id === selectedId) ?? (selectedId ? null : (groups[0] ?? null))
	);
	const detail = $derived(
		['info', 'members', 'permissions'].find(
			(section) => section === url.searchParams.get('section')
		) ?? 'info'
	);
	let detailHeading: HTMLHeadingElement | undefined = $state();
	let listHeading: HTMLHeadingElement | undefined = $state();
	let workspaceToolbar: HTMLDivElement | undefined = $state();
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
	const search = $derived(url.searchParams.get('group_q') ?? '');
	const statusFilters = [
		{ value: 'all', label: 'ทั้งหมด' },
		{ value: 'active', label: 'เปิดใช้งาน' },
		{ value: 'inactive', label: 'ปิดใช้งาน' }
	];
	const status = $derived(
		statusFilters.find((item) => item.value === url.searchParams.get('group_status'))?.value ??
			'all'
	);
	const visibleGroups = $derived(
		groups.filter(
			(group) =>
				`${group.nameTh} ${group.nameEn}`.toLowerCase().includes(search.trim().toLowerCase()) &&
				(status === 'all' || (status === 'active' ? group.isActive : !group.isActive))
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

	async function navigate(group: string, section = 'info') {
		const changingGroup = selectedId !== group;
		const next = new URL(url.href);
		next.searchParams.set('tab', 'subject-groups');
		next.searchParams.set('group', group);
		next.searchParams.set('section', section);
		await goto(next.pathname + next.search, { shallow: true, reset: false });
		await tick();
		if (changingGroup && window.matchMedia('(max-width: 1023px)').matches) {
			detailHeading?.focus({ preventScroll: true });
			workspaceToolbar?.scrollIntoView({ block: 'start' });
		}
	}
	async function showList() {
		const next = new URL(url.href);
		next.searchParams.delete('group');
		next.searchParams.delete('section');
		await goto(next.pathname + next.search, { shallow: true, reset: false });
		await tick();
		listHeading?.focus({ preventScroll: true });
		workspaceToolbar?.scrollIntoView({ block: 'start' });
	}
	function filterGroups(query: string, nextStatus: string) {
		const next = new URL(url.href);
		if (query) next.searchParams.set('group_q', query);
		else next.searchParams.delete('group_q');
		if (nextStatus !== 'all') next.searchParams.set('group_status', nextStatus);
		else next.searchParams.delete('group_status');
		void goto(next.pathname + next.search, { shallow: true, replace: true, reset: false });
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
		if (!ready || saving || (group ? !canEdit : !canCreate)) return;
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
		const selectionAtStart = selectedId;
		const sectionAtStart = detail;
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
			if (selectedId === selectionAtStart && detail === sectionAtStart) {
				void navigate(group.id, target ? sectionAtStart : 'info');
			}
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

<section aria-label="กลุ่มสาระการเรียนรู้" aria-busy={loading} class="space-y-4">
	<div
		bind:this={workspaceToolbar}
		class="flex scroll-mt-20 flex-wrap items-start justify-between gap-3"
		data-testid="learning-area-toolbar"
	>
		<div class={selectedId ? 'hidden lg:block' : ''}>
			<h2 bind:this={listHeading} tabindex="-1" class="font-semibold outline-none">
				เลือกกลุ่มสาระ
			</h2>
			{#if ready}<p class="mt-1 text-sm text-muted-foreground">
					{groups.length} กลุ่มสาระในโรงเรียน
				</p>{/if}
		</div>
		{#if selectedId}<Button variant="outline" onclick={showList} class="lg:hidden"
				><ArrowLeft class="size-4" />กลับไปเลือกกลุ่มสาระ</Button
			>{/if}
		{#if canRead && canCreate}<Button onclick={() => edit(null)} disabled={!ready || saving}
				><Plus class="size-4" />เพิ่มกลุ่มสาระ</Button
			>{/if}
	</div>
	{#if !canRead}
		<PageState variant="permission" title="ไม่มีสิทธิ์ดูทะเบียนกลุ่มสาระ" />
	{:else if loading && !ready}
		<div role="status" aria-label="กำลังโหลดกลุ่มสาระ"><PageSkeleton variant="detail" /></div>
	{:else if !ready && error}
		<PageState
			variant="error"
			title="โหลดกลุ่มสาระไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={refresh}
		/>
	{:else if ready}
		<div class="relative space-y-4" data-testid="catalog-subject-groups-ready">
			{#if error}<PageState
					variant="error"
					title="อัปเดตกลุ่มสาระไม่สำเร็จ"
					description={error}
					actionLabel="ลองอีกครั้ง"
					onaction={refresh}
				/>{/if}
			{#if groups.length === 0}
				<PageState
					title="ยังไม่มีกลุ่มสาระ"
					description={canCreate
						? 'เพิ่มกลุ่มสาระเพื่อเริ่มจัดรายวิชา หัวหน้า และสมาชิก'
						: 'โรงเรียนยังไม่ได้เพิ่มข้อมูลกลุ่มสาระ'}
				/>
			{:else}
				<div class="grid items-start gap-4 lg:grid-cols-[minmax(0,2fr)_minmax(0,3fr)]">
					<div
						class={`min-w-0 scroll-mt-20 space-y-4 ${selectedId ? 'hidden lg:block' : ''}`}
						data-testid="learning-area-list"
					>
						<div class="space-y-3 rounded-xl border bg-card p-3 sm:p-4">
							<div class="space-y-2">
								<Label for="learning-area-search">ค้นหากลุ่มสาระ</Label>
								<div class="relative">
									<Search
										class="absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
									/><Input
										id="learning-area-search"
										value={search}
										oninput={(event) => filterGroups(event.currentTarget.value, status)}
										placeholder="ชื่อกลุ่มสาระหรือชื่อภาษาอังกฤษ"
										class="pl-10"
									/>
								</div>
							</div>
							<ToggleGroup.Root
								type="single"
								aria-label="สถานะกลุ่มสาระ"
								class="flex-wrap justify-start"
								bind:value={
									() => status,
									(value) => {
										const choice = statusFilters.find((item) => item.value === value);
										if (choice) filterGroups(search, choice.value);
									}
								}
							>
								{#each statusFilters as item (item.value)}<ToggleGroup.Item value={item.value}
										>{item.label}</ToggleGroup.Item
									>{/each}
							</ToggleGroup.Root>
						</div>
						{#if visibleGroups.length === 0}
							<PageState
								title="ไม่พบกลุ่มสาระที่ตรงเงื่อนไข"
								description="ลองปรับชื่อหรือสถานะที่เลือก"
								actionLabel="ล้างตัวกรอง"
								onaction={() => filterGroups('', 'all')}
							/>
						{:else}
							<Card.Root class="gap-0 py-0">
								<nav aria-label="รายการกลุ่มสาระ" class="p-2 lg:max-h-[40rem] lg:overflow-y-auto">
									<ul class="space-y-1">
										{#each visibleGroups as group (group.id)}
											<li>
												<Button
													variant="ghost"
													class={`h-auto w-full justify-start whitespace-normal py-2 text-left ${selected?.id === group.id ? 'bg-primary/10 text-primary hover:bg-primary/15' : ''}`}
													onclick={() => {
														if (selectedId !== group.id) void navigate(group.id);
													}}
													aria-pressed={selected?.id === group.id}
												>
													<Layers3 class="size-4 self-start" /><span
														class="min-w-0 flex-1 break-words"
														>{group.nameTh}{#if !group.isActive}<span
																class="mt-1 block text-xs font-normal text-muted-foreground"
																>ปิดใช้งาน</span
															>{/if}</span
													><ChevronRight class="size-4 text-muted-foreground" />
												</Button>
											</li>
										{/each}
									</ul>
								</nav>
							</Card.Root>
						{/if}
					</div>
					<div
						class={`min-w-0 scroll-mt-20 space-y-4 ${selectedId ? '' : 'hidden lg:block'}`}
						data-testid="learning-area-detail"
					>
						{#if selected}
							<Card.Root class="gap-0 overflow-hidden py-0" aria-label="รายละเอียดกลุ่มสาระ">
								<Card.Content class="border-b py-3">
									<nav class="flex flex-wrap gap-2" aria-label="ข้อมูลกลุ่มสาระ">
										<Button
											size="sm"
											variant={detail === 'info' ? 'secondary' : 'ghost'}
											aria-current={detail === 'info' ? 'page' : undefined}
											onclick={() => navigate(selected.id, 'info')}>ข้อมูล</Button
										>
										<Button
											size="sm"
											variant={detail === 'members' ? 'secondary' : 'ghost'}
											aria-current={detail === 'members' ? 'page' : undefined}
											onclick={() => navigate(selected.id, 'members')}>หัวหน้าและสมาชิก</Button
										>
										{#if canReadGrants}<Button
												size="sm"
												variant={detail === 'permissions' ? 'secondary' : 'ghost'}
												aria-current={detail === 'permissions' ? 'page' : undefined}
												onclick={() => navigate(selected.id, 'permissions')}>สิทธิ์</Button
											>{/if}
									</nav>
								</Card.Content>
								<div class="space-y-4 bg-gradient-to-br from-primary/5 via-card to-card p-4 sm:p-5">
									<div class="flex flex-wrap gap-2">
										<Badge variant="secondary">กลุ่มสาระการเรียนรู้</Badge><Badge
											variant="outline"
											class={selected.isActive
												? 'border-success/20 bg-success/10 text-success'
												: ''}>{selected.isActive ? 'เปิดใช้งาน' : 'ปิดใช้งาน'}</Badge
										>
									</div>
									<h2
										bind:this={detailHeading}
										tabindex="-1"
										class="break-words text-xl font-semibold outline-none"
									>
										{selected.nameTh}
									</h2>
									{#if canEdit}<Button
											size="sm"
											variant="outline"
											onclick={() => edit(selected)}
											disabled={saving}><Pencil class="size-4" />แก้ไขข้อมูล</Button
										>{/if}
								</div>
							</Card.Root>
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
							{:else if detail === 'permissions'}
								{#if canReadGrants}
									<Card.Root class="gap-0 py-0"
										><Card.Content class="space-y-4 py-4 sm:py-5">
											<h3 class="flex items-center gap-2 font-semibold">
												<ShieldCheck class="size-4 text-primary" />สิทธิ์ตามตำแหน่ง
											</h3>
											<p class="text-sm text-muted-foreground">
												กำหนดแยกสำหรับหัวหน้า รองหัวหน้า ผู้ประสานงาน และสมาชิก
												หัวหน้าจะได้รับเฉพาะสิทธิ์ที่กำหนดให้ตำแหน่งนั้น
											</p>
											<Button variant="outline" onclick={() => (permissionOpen = true)}
												>{$can.has(PERMISSIONS.ROLES_UPDATE_ALL)
													? 'กำหนดสิทธิ์ตามตำแหน่ง'
													: 'ดูสิทธิ์ตามตำแหน่ง'}</Button
											>
										</Card.Content></Card.Root
									>
								{:else}<PageState
										variant="permission"
										title="ไม่มีสิทธิ์ดูการกำหนดสิทธิ์"
										description="เลือกดูข้อมูลหรือหัวหน้าและสมาชิกของกลุ่มสาระได้"
									/>{/if}
							{:else}
								<Card.Root class="gap-0 py-0"
									><Card.Content class="space-y-4 py-4 sm:py-5">
										<h3 class="font-semibold">ข้อมูลกลุ่มสาระ</h3>
										<dl class="grid gap-4 text-sm sm:grid-cols-2">
											<div>
												<dt class="text-muted-foreground">ชื่อภาษาอังกฤษ</dt>
												<dd class="mt-1 break-words">{selected.nameEn || 'ไม่ได้ระบุ'}</dd>
											</div>
											<div>
												<dt class="text-muted-foreground">ลำดับแสดงผล</dt>
												<dd class="mt-1">{selected.displayOrder}</dd>
											</div>
										</dl>
									</Card.Content></Card.Root
								>
							{/if}
						{:else}<PageState
								title="ไม่พบกลุ่มสาระที่เลือก"
								description="กลับไปเลือกกลุ่มสาระจากรายการปัจจุบัน"
								actionLabel="เลือกรายการกลุ่มสาระ"
								onaction={showList}
							/>{/if}
					</div>
				</div>
			{/if}
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
