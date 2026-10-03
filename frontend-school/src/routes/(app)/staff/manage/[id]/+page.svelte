<script lang="ts">
	import { ACADEMIC_RANK_LABELS, EDUCATION_LEVEL_LABELS } from '$lib/forms/staff-personnel';
	import { onDestroy, untrack } from 'svelte';
	import { LatestRequest } from '$lib/async/latest-request';
	import { captureRouteLoad } from '$lib/navigation/route-load';
	import { requireApiData } from '$lib/api/client';
	import type { PageProps } from './$types';
	import { getStaffProfile, type StaffProfileResponse } from '$lib/api/staff';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { authStore } from '$lib/stores/auth';
	import { can } from '$lib/stores/permissions';
	import { Button } from '$lib/components/ui/button';
	import { PageShell } from '$lib/components/app-layout';
	import { page } from '$app/state';
	import { staffReturnHref, withStaffReturn } from '$lib/navigation/staff-management';
	import StaffBreadcrumb from '$lib/components/staff/StaffBreadcrumb.svelte';
	import { Badge } from '$lib/components/ui/badge';
	import { staffStatusLabel } from '$lib/forms/staff-status';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import * as Dialog from '$lib/components/ui/dialog';
	import {
		User,
		Mail,
		Phone,
		GraduationCap,
		Building2,
		BookOpen,
		Pencil,
		Award,
		Plus,
		IdCard
	} from '@lucide/svelte';
	import type { Achievement } from '$lib/types/achievement';
	import {
		getAchievements,
		createAchievement,
		updateAchievement,
		deleteAchievement
	} from '$lib/api/achievement';
	import AchievementCard from '$lib/components/achievement/AchievementCard.svelte';
	import AchievementDialog from '$lib/components/achievement/AchievementDialog.svelte';
	import PrivateFileImage from '$lib/components/files/PrivateFileImage.svelte';
	import { toast } from 'svelte-sonner';

	let staff: StaffProfileResponse | null = $state(null);
	let loading = $state(true);
	let error = $state('');

	// Achievement State
	let achievements: Achievement[] = $state([]);
	let loadingAchievements = $state(true);
	let achievementsLoaded = $state(false);
	let achievementsError = $state('');
	let achievementSaving = $state(false);
	let showAchievementDialog = $state(false);
	let selectedAchievement: Achievement | null = $state(null);
	let showDeleteDialog = $state(false);
	let deleteId = $state<string | null>(null);

	let { data }: PageProps = $props();
	const staffId = $derived(data.staffId);
	const returnHref = $derived(staffReturnHref(page.url));
	const staffSource = $derived(data.staff),
		achievementSource = $derived(data.achievements);
	const staffRequest = new LatestRequest(),
		achievementRequest = new LatestRequest();
	let activeId = '',
		disposed = false,
		personEpoch = 0,
		draftEpoch = 0;
	const currentUserId = $derived($authStore.user?.id ?? '');
	const canReadStaff = $derived(
		$can.hasAny(
			PERMISSIONS.STAFF_PROFILE_READ_ORGANIZATION_UNIT,
			PERMISSIONS.STAFF_PROFILE_READ_ORGANIZATION_TREE,
			PERMISSIONS.STAFF_PROFILE_READ_SCHOOL
		) ||
			(staffId === currentUserId && $can.has(PERMISSIONS.STAFF_PROFILE_READ_OWN))
	);
	const canUpdateStaff = $derived($can.has(PERMISSIONS.STAFF_UPDATE_ALL));
	const canReadStaffPii = $derived(
		$can.has(PERMISSIONS.STAFF_PII_READ_SCHOOL) ||
			($can.has(PERMISSIONS.STAFF_PII_READ_OWN) && staffId === currentUserId)
	);
	const canReadAchievements = $derived(
		$can.has(PERMISSIONS.ACHIEVEMENT_READ_ALL) ||
			($can.has(PERMISSIONS.ACHIEVEMENT_READ_OWN) && staffId === currentUserId)
	);
	const canCreateAchievementForStaff = $derived(
		$can.has(PERMISSIONS.ACHIEVEMENT_CREATE_ALL) ||
			($can.has(PERMISSIONS.ACHIEVEMENT_CREATE_OWN) && staffId === currentUserId)
	);
	const canUpdateAchievementForStaff = $derived(
		$can.has(PERMISSIONS.ACHIEVEMENT_UPDATE_ALL) ||
			($can.has(PERMISSIONS.ACHIEVEMENT_UPDATE_OWN) && staffId === currentUserId)
	);
	const canDeleteAchievementForStaff = $derived(
		$can.has(PERMISSIONS.ACHIEVEMENT_DELETE_ALL) ||
			($can.has(PERMISSIONS.ACHIEVEMENT_DELETE_OWN) && staffId === currentUserId)
	);

	$effect.pre(() => {
		const id = staffId;
		untrack(() => {
			if (activeId !== id) {
				activeId = id;
				personEpoch++;
				staff = null;
				achievements = [];
				achievementsLoaded = false;
				achievementSaving = false;
				showAchievementDialog = false;
				selectedAchievement = null;
				showDeleteDialog = false;
				deleteId = null;
			}
		});
	});
	$effect.pre(() => {
		const read = staffSource;
		untrack(() => {
			const ticket = staffRequest.begin();
			loading = true;
			error = '';
			void read.then((result) => applyStaff(result, ticket.revision));
		});
		return () => staffRequest.abort();
	});
	$effect.pre(() => {
		const read = achievementSource,
			allowed = canReadAchievements;
		untrack(() => {
			if (!allowed) {
				achievementRequest.abort();
				achievements = [];
				achievementsLoaded = false;
				loadingAchievements = false;
				showAchievementDialog = false;
				showDeleteDialog = false;
				return;
			}
			const ticket = achievementRequest.begin();
			loadingAchievements = true;
			achievementsError = '';
			void read.then((result) => applyAchievements(result, ticket.revision));
		});
		return () => achievementRequest.abort();
	});
	$effect.pre(() => {
		const open = showAchievementDialog;
		untrack(() => {
			draftEpoch++;
			if (!open) selectedAchievement = null;
		});
	});
	onDestroy(() => {
		disposed = true;
		personEpoch++;
		staffRequest.abort();
		achievementRequest.abort();
	});
	function applyStaff(result: Awaited<typeof data.staff>, revision: number) {
		if (!staffRequest.isCurrent(revision)) return;
		loading = false;
		if (result.ok) staff = result.data;
		else error = result.error;
	}
	function applyAchievements(result: Awaited<typeof data.achievements>, revision: number) {
		if (!achievementRequest.isCurrent(revision)) return;
		loadingAchievements = false;
		if (result.ok) {
			achievements = result.data;
			achievementsLoaded = true;
		} else achievementsError = result.error;
	}
	async function loadStaffProfile() {
		if (!canReadStaff) return;
		const ticket = staffRequest.begin();
		loading = true;
		error = '';
		applyStaff(
			await captureRouteLoad(
				getStaffProfile(staffId, { signal: ticket.signal }).then((reply) =>
					requireApiData(reply, 'โหลดข้อมูลบุคลากรไม่สำเร็จ')
				),
				'โหลดข้อมูลบุคลากรไม่สำเร็จ'
			),
			ticket.revision
		);
	}
	async function loadAchievements() {
		if (!canReadAchievements) return;
		const ticket = achievementRequest.begin();
		loadingAchievements = true;
		achievementsError = '';
		applyAchievements(
			await captureRouteLoad(
				getAchievements({ user_id: staffId }, { signal: ticket.signal }).then((reply) =>
					requireApiData(reply, 'โหลดผลงานไม่สำเร็จ')
				),
				'โหลดผลงานไม่สำเร็จ'
			),
			ticket.revision
		);
	}
	async function handleSaveAchievement(payload: Partial<Achievement>) {
		if (
			!showAchievementDialog ||
			achievementSaving ||
			(payload.user_id && payload.user_id !== staffId)
		)
			return;
		if (payload.id ? !canUpdateAchievementForStaff : !canCreateAchievementForStaff) return;
		const owner = staffId,
			epoch = personEpoch,
			draft = draftEpoch;
		const current = () => !disposed && epoch === personEpoch && owner === staffId;
		achievementRequest.abort();
		loadingAchievements = false;
		achievementSaving = true;
		try {
			const fields = {
				title: payload.title ?? '',
				description: payload.description,
				achievement_date: payload.achievement_date ?? '',
				image_file_id: payload.image_file_id
			};
			const saved = requireApiData(
				await (payload.id
					? updateAchievement(payload.id, fields)
					: createAchievement({ ...fields, user_id: owner })),
				'บันทึกผลงานไม่สำเร็จ'
			);
			if (!current()) return;
			achievements = payload.id
				? achievements.map((item) => (item.id === saved.id ? saved : item))
				: [saved, ...achievements];
			achievementsLoaded = true;
			if (draft === draftEpoch) {
				toast.success(payload.id ? 'แก้ไขผลงานเรียบร้อย' : 'เพิ่มผลงานเรียบร้อย');
				showAchievementDialog = false;
			}
		} catch (e) {
			if (current() && draft === draftEpoch)
				toast.error(e instanceof Error ? e.message : 'บันทึกผลงานไม่สำเร็จ');
		} finally {
			if (current()) achievementSaving = false;
		}
	}
	function confirmDelete(achievement: Achievement) {
		if (!canDeleteAchievementForStaff || achievementSaving) return;
		deleteId = achievement.id;
		showDeleteDialog = true;
	}
	async function handleDeleteAchievement() {
		if (!deleteId || !canDeleteAchievementForStaff || achievementSaving) return;
		const id = deleteId,
			owner = staffId,
			epoch = personEpoch;
		const current = () => !disposed && epoch === personEpoch && owner === staffId;
		achievementRequest.abort();
		loadingAchievements = false;
		achievementSaving = true;
		try {
			const reply = await deleteAchievement(id);
			if (!current()) return;
			if (!reply.success) throw new Error(reply.error || 'ลบผลงานไม่สำเร็จ');
			achievements = achievements.filter((item) => item.id !== id);
			toast.success('ลบผลงานเรียบร้อย');
			if (deleteId === id) {
				showDeleteDialog = false;
				deleteId = null;
			}
		} catch (e) {
			if (current()) toast.error(e instanceof Error ? e.message : 'ลบผลงานไม่สำเร็จ');
		} finally {
			if (current()) achievementSaving = false;
		}
	}
</script>

<PageShell
	title="ข้อมูลบุคลากร"
	description={staff?.username && canReadStaff
		? `รายละเอียดบุคลากร • ${staff.username}`
		: 'รายละเอียดบุคลากร'}
	backHref={returnHref}
	backLabel="กลับรายชื่อบุคลากร"
	backPreload="off"
>
	{#snippet meta()}<StaffBreadcrumb
			{returnHref}
			name={staff && canReadStaff ? `${staff.first_name} ${staff.last_name}` : undefined}
		/>{/snippet}
	{#snippet actions()}
		<Button variant="outline" onclick={loadStaffProfile} disabled={loading || !canReadStaff}
			>รีเฟรชข้อมูล</Button
		>
		<Button
			variant="outline"
			onclick={loadAchievements}
			disabled={loadingAchievements || achievementSaving || !canReadAchievements}
			>รีเฟรชผลงาน</Button
		>
		{#if staff && canReadStaff && canUpdateStaff}
			<Button
				href={withStaffReturn(`/staff/manage/${staff.id}/edit`, returnHref)}
				data-sveltekit-preload-data="tap"
				class="flex items-center gap-2"
			>
				<Pencil class="w-4 h-4" />
				แก้ไข
			</Button>
		{/if}
	{/snippet}

	<section data-testid="staff-profile" aria-busy={loading}>
		{#if error && staff && canReadStaff}<PageState
				title="อัปเดตข้อมูลบุคลากรไม่สำเร็จ"
				description={error}
				actionLabel="ลองอีกครั้ง"
				onaction={loadStaffProfile}
			/>{/if}
		{#if loading && staff && canReadStaff}<p role="status">กำลังอัปเดตข้อมูลบุคลากร...</p>{/if}
		{#if !canReadStaff}<PageState variant="permission" title="ไม่มีสิทธิ์ดูข้อมูลบุคลากร" />
		{:else if loading && !staff}<div role="status" aria-label="กำลังโหลดข้อมูลบุคลากร">
				<PageSkeleton variant="detail" />
			</div>
		{:else if error && !staff}
			<PageState
				variant="error"
				title="โหลดข้อมูลบุคลากรไม่สำเร็จ"
				description={error}
				actionLabel="ลองอีกครั้ง"
				onaction={loadStaffProfile}
			/>
		{:else if staff}
			<!-- Profile Card -->
			<div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
				<!-- Left Column - Basic Info -->
				<div class="lg:col-span-1 space-y-6">
					<!-- Profile Card -->
					<div class="bg-card border border-border rounded-lg p-6">
						<div class="text-center">
							<div
								class="w-24 h-24 rounded-full bg-primary/10 flex items-center justify-center mx-auto mb-4 overflow-hidden"
							>
								{#if staff.profile_image_file_id}
									<PrivateFileImage
										fileId={staff.profile_image_file_id}
										resourceId={staff.id}
										alt={`${staff.first_name} ${staff.last_name}`}
										class="w-full h-full object-cover"
									/>
								{:else}
									<User class="w-12 h-12 text-primary" />
								{/if}
							</div>
							<h2 class="text-2xl font-bold text-foreground">
								{staff.title || ''}{staff.first_name}
								{staff.last_name}
							</h2>
							<div class="flex flex-col items-center mt-1 space-y-1">
								{#if staff.nickname}
									<p class="text-muted-foreground">({staff.nickname})</p>
								{/if}
								<div
									class="flex items-center gap-1.5 text-sm text-muted-foreground bg-muted/50 px-3 py-1 rounded-full"
								>
									<IdCard class="w-3.5 h-3.5" />
									<span>{staff.username}</span>
								</div>
							</div>

							<div class="mt-4">
								<Badge variant={staff.status === 'active' ? 'default' : 'secondary'}
									>{staffStatusLabel(staff.status)}</Badge
								>
							</div>
						</div>

						<div class="mt-6 space-y-3 border-t border-border pt-6">
							{#if canUpdateStaff}<Button
									href={withStaffReturn(
										`/staff/manage/${staff.id}/edit?section=personal`,
										returnHref
									)}
									variant="outline"
									size="sm">แก้ไขข้อมูลติดต่อ</Button
								>{/if}
							{#if staff.email}
								<div class="flex items-center gap-3 text-sm">
									<Mail class="w-4 h-4 text-muted-foreground" />
									<span class="text-foreground">{staff.email}</span>
								</div>
							{/if}
							{#if staff.phone}
								<div class="flex items-center gap-3 text-sm">
									<Phone class="w-4 h-4 text-muted-foreground" />
									<span class="text-foreground">{staff.phone}</span>
								</div>
							{/if}
							{#if staff.national_id && canReadStaffPii}
								<div class="flex items-center gap-3 text-sm">
									<User class="w-4 h-4 text-muted-foreground" />
									<span class="text-foreground">บัตรปชช.: {staff.national_id}</span>
								</div>
							{/if}
						</div>
					</div>

					<div class="rounded-lg border bg-card p-6 space-y-4">
						<h3 class="font-semibold">ตำแหน่งและการศึกษา</h3>
						<dl class="space-y-3 text-sm">
							<div>
								<dt class="text-muted-foreground">ตำแหน่งงาน</dt>
								<dd>{staff.staff_info?.job_position?.name ?? 'ยังไม่ระบุ'}</dd>
							</div>
							<div>
								<dt class="text-muted-foreground">วิทยฐานะ</dt>
								<dd>
									{staff.staff_info?.academic_rank
										? ACADEMIC_RANK_LABELS[staff.staff_info.academic_rank]
										: 'ยังไม่ระบุ'}
								</dd>
							</div>
							<div>
								<dt class="text-muted-foreground">วุฒิการศึกษาสูงสุด</dt>
								<dd>
									{staff.staff_info?.education_level
										? EDUCATION_LEVEL_LABELS[staff.staff_info.education_level]
										: 'ยังไม่ระบุ'}
								</dd>
							</div>
							<div>
								<dt class="text-muted-foreground">สาขาวิชา</dt>
								<dd>{staff.staff_info?.major ?? 'ยังไม่ระบุ'}</dd>
							</div>
							<div>
								<dt class="text-muted-foreground">สถาบันการศึกษา</dt>
								<dd>{staff.staff_info?.university ?? 'ยังไม่ระบุ'}</dd>
							</div>
							<div>
								<dt class="text-muted-foreground">กลุ่มสาระจากสังกัดปัจจุบัน</dt>
								<dd>
									{(staff.subject_groups ?? []).map((group) => group.name).join(', ') ||
										'ยังไม่มีสังกัดกลุ่มสาระ'}
								</dd>
							</div>
						</dl>
					</div>
				</div>

				<!-- Right Column - Details -->
				<div class="lg:col-span-2 space-y-6">
					<!-- Roles Card -->
					<div class="bg-card border border-border rounded-lg p-6">
						<h3 class="font-semibold text-foreground mb-4 flex items-center gap-2">
							<GraduationCap class="w-5 h-5" />
							บทบาทและตำแหน่ง
						</h3>
						{#if $can.has(PERMISSIONS.ROLES_READ_ALL)}<Button
								href={withStaffReturn(`/staff/manage/${staff.id}/roles`, returnHref)}
								variant="outline"
								size="sm"
								class="mb-4">จัดการบทบาทและสิทธิ์</Button
							>{/if}
						{#if staff.roles.length > 0}
							<div class="flex flex-wrap gap-2">
								{#each staff.roles as role (role.id)}
									<div
										class="px-4 py-2 rounded-lg border border-border {role.is_primary
											? 'bg-primary/10 border-primary'
											: 'bg-muted'}"
									>
										<div class="flex items-center gap-2">
											<span class="font-medium text-foreground">{role.name}</span>
											{#if role.is_primary}
												<span
													class="text-xs px-2 py-0.5 bg-primary text-primary-foreground rounded-full"
												>
													หลัก
												</span>
											{/if}
										</div>
										<p class="text-xs text-muted-foreground mt-1">
											{role.user_type} • ระดับ {role.level}
										</p>
									</div>
								{/each}
							</div>
						{:else}
							<p class="text-muted-foreground">ยังไม่มีบทบาท</p>
						{/if}
					</div>

					<!-- Organization Units Card -->
					<div class="bg-card border border-border rounded-lg p-6">
						<h3 class="font-semibold text-foreground mb-4 flex items-center gap-2">
							<Building2 class="w-5 h-5" />
							สังกัดหน่วยงาน
						</h3>
						{#if canUpdateStaff}<Button
								href={withStaffReturn(
									`/staff/manage/${staff.id}/edit?section=organizations`,
									returnHref
								)}
								variant="outline"
								size="sm"
								class="mb-4">แก้ไขสังกัด</Button
							>{/if}
						{#if staff.organization_units.length > 0}
							<div class="space-y-3">
								{#each staff.organization_units as dept (dept.id)}
									<div
										class="px-4 py-3 rounded-lg border border-border {dept.is_primary
											? 'bg-primary/5 border-primary/30'
											: 'bg-muted/50'}"
									>
										<div class="flex items-start justify-between">
											<div>
												<p class="font-medium text-foreground">{dept.name}</p>
												<p class="text-sm text-muted-foreground mt-1">
													{dept.position_title || dept.position_code || 'สมาชิก'}
												</p>
											</div>
											{#if dept.is_primary}
												<span
													class="text-xs px-2 py-1 bg-primary text-primary-foreground rounded-full"
												>
													สังกัดหลัก
												</span>
											{/if}
										</div>
									</div>
								{/each}
							</div>
						{:else}
							<p class="text-muted-foreground">ยังไม่ได้สังกัดหน่วยงาน</p>
						{/if}
					</div>

					<!-- หน้าที่ทางวิชาการ: ครูที่ปรึกษา + วิชาที่สอน -->
					<div class="bg-card border border-border rounded-lg p-6">
						<h3 class="font-semibold text-foreground mb-4 flex items-center gap-2">
							<BookOpen class="w-5 h-5" />
							หน้าที่ทางวิชาการ
						</h3>

						{#if staff.advisor_homerooms.length === 0 && staff.teaching_assignments.length === 0}
							<p class="text-muted-foreground">ยังไม่มีข้อมูลการสอน/ครูที่ปรึกษา</p>
						{:else}
							<!-- Group โดยปีการศึกษา (ใหม่สุดก่อน) — รวมทั้งครูประจำชั้นและงานสอน -->
							{@const yearGroups = (() => {
								const map = new Map<
									number,
									{
										label: string;
										advisors: typeof staff.advisor_homerooms;
										assignments: typeof staff.teaching_assignments;
									}
								>();
								for (const advisor of staff.advisor_homerooms) {
									if (!map.has(advisor.academicYear)) {
										map.set(advisor.academicYear, {
											label: advisor.academicYearLabel,
											advisors: [],
											assignments: []
										});
									}
									map.get(advisor.academicYear)!.advisors.push(advisor);
								}
								for (const assignment of staff.teaching_assignments) {
									if (!map.has(assignment.academicYear)) {
										map.set(assignment.academicYear, {
											label: assignment.academicYearLabel,
											advisors: [],
											assignments: []
										});
									}
									map.get(assignment.academicYear)!.assignments.push(assignment);
								}
								return Array.from(map.entries()).sort((a, b) => b[0] - a[0]);
							})()}

							<div class="space-y-5">
								{#each yearGroups as [year, g], index (year)}
									<details open={index === 0} class="rounded-lg border p-3">
										<summary class="mb-2 cursor-pointer text-sm font-semibold text-foreground"
											>{g.label}</summary
										>

										{#if g.advisors.length > 0}
											<div class="mb-2 flex flex-wrap gap-1.5">
												<span class="text-xs text-muted-foreground self-center">ครูที่ปรึกษา:</span>
												{#each g.advisors as advisor (advisor.homeroomId)}
													<span
														class="text-xs px-2 py-0.5 rounded-full {advisor.role === 'primary'
															? 'bg-primary/10 text-primary'
															: 'bg-secondary text-secondary-foreground'}"
													>
														{advisor.role === 'primary' ? '⭐ ' : ''}{advisor.homeroomName}
													</span>
												{/each}
											</div>
										{/if}

										{#if g.assignments.length > 0}
											<div class="space-y-2">
												{#each g.assignments as assignment (`${assignment.academicTermId}-${assignment.learningGroupId}-${assignment.subjectId}-${assignment.role}`)}
													<div
														class="px-3 py-2 rounded-lg bg-muted/50 border border-border flex items-start justify-between"
													>
														<div>
															<p class="font-medium text-foreground text-sm">
																{assignment.subjectName}
																<span class="text-xs text-muted-foreground"
																	>({assignment.subjectCode})</span
																>
															</p>
															<p class="text-xs text-muted-foreground mt-0.5">
																{assignment.learningGroupName} • {assignment.termName}
																{#if assignment.hours}
																	• {assignment.hours} ชม.
																{/if}
															</p>
														</div>
														<span
															class="text-xs px-2 py-0.5 rounded-full shrink-0 {assignment.role ===
															'primary'
																? 'bg-primary/10 text-primary'
																: 'bg-secondary text-secondary-foreground'}"
														>
															{assignment.role === 'primary' ? 'ครูหลัก' : 'ครูร่วม'}
														</span>
													</div>
												{/each}
											</div>
										{/if}
									</details>
								{/each}
							</div>
						{/if}
					</div>
				</div>
			</div>
		{:else}
			<PageState
				title="ไม่พบข้อมูลบุคลากร"
				description="ข้อมูลบุคลากรนี้อาจถูกลบหรือคุณอาจไม่มีสิทธิ์เข้าถึง"
				actionLabel="กลับหน้าบุคลากร"
				href={returnHref}
			/>
		{/if}
	</section>
	<!-- Achievements Card -->
	<section data-testid="staff-achievements" aria-busy={loadingAchievements}>
		{#if achievementsError && achievementsLoaded && canReadAchievements}<PageState
				title="อัปเดตผลงานไม่สำเร็จ"
				description={achievementsError}
				actionLabel="ลองอีกครั้ง"
				onaction={loadAchievements}
			/>{/if}
		{#if loadingAchievements && achievementsLoaded && canReadAchievements}<p role="status">
				กำลังอัปเดตผลงาน...
			</p>{/if}
		<div class="bg-card border border-border rounded-lg p-6">
			<div class="flex items-center justify-between mb-4">
				<h3 class="font-semibold text-foreground flex items-center gap-2">
					<Award class="w-5 h-5" />
					ผลงานและรางวัล
				</h3>
				{#if canCreateAchievementForStaff && !achievementSaving}
					<Button
						variant="outline"
						size="sm"
						class="h-8"
						onclick={() => {
							selectedAchievement = null;
							showAchievementDialog = true;
						}}
					>
						<Plus class="w-4 h-4 mr-2" />
						เพิ่มผลงาน
					</Button>
				{/if}
			</div>

			{#if !canReadAchievements}
				<div class="py-8 text-center bg-muted/20 rounded-lg border border-dashed">
					<Award class="w-10 h-10 text-muted-foreground/30 mx-auto mb-2" />
					<p class="text-sm text-muted-foreground">ไม่มีสิทธิ์ดูผลงานและรางวัล</p>
				</div>
			{:else if loadingAchievements && !achievementsLoaded}<div
					role="status"
					aria-label="กำลังโหลดผลงาน"
				>
					<PageSkeleton variant="cards" rows={2} />
				</div>
			{:else if achievementsError && !achievementsLoaded}<PageState
					variant="error"
					title="โหลดผลงานไม่สำเร็จ"
					description={achievementsError}
					actionLabel="ลองอีกครั้ง"
					onaction={loadAchievements}
				/>
			{:else if achievements.length > 0}
				<div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
					{#each achievements as achievement (achievement.id)}
						<AchievementCard
							{achievement}
							canEdit={canUpdateAchievementForStaff && !achievementSaving}
							canDelete={canDeleteAchievementForStaff && !achievementSaving}
							onedit={(a) => {
								if (!canUpdateAchievementForStaff) return;
								selectedAchievement = a;
								showAchievementDialog = true;
							}}
							ondelete={(a) => {
								if (!canDeleteAchievementForStaff) return;
								confirmDelete(a);
							}}
						/>
					{/each}
				</div>
			{:else}
				<div class="py-8 text-center bg-muted/20 rounded-lg border border-dashed">
					<Award class="w-10 h-10 text-muted-foreground/30 mx-auto mb-2" />
					<p class="text-sm text-muted-foreground">ยังไม่มีข้อมูลผลงาน</p>
				</div>
			{/if}
		</div>
	</section>

	{#if showAchievementDialog}
		<AchievementDialog
			open={showAchievementDialog}
			achievement={selectedAchievement}
			userId={staffId ?? ''}
			onclose={() => (showAchievementDialog = false)}
			onsave={handleSaveAchievement}
		/>
	{/if}

	<!-- Delete Confirmation Dialog -->
	<Dialog.Root bind:open={showDeleteDialog}>
		<Dialog.Content class="sm:max-w-[425px]">
			<Dialog.Header>
				<Dialog.Title>ยืนยันการลบข้อมูล</Dialog.Title>
				<Dialog.Description>
					คุณต้องการลบรายการนี้ใช่หรือไม่? การกระทำนี้ไม่สามารถย้อนกลับได้
				</Dialog.Description>
			</Dialog.Header>
			<Dialog.Footer>
				<Button variant="outline" onclick={() => (showDeleteDialog = false)}>ยกเลิก</Button>
				<Button variant="destructive" onclick={handleDeleteAchievement} disabled={achievementSaving}
					>ลบข้อมูล</Button
				>
			</Dialog.Footer>
		</Dialog.Content>
	</Dialog.Root>
</PageShell>
