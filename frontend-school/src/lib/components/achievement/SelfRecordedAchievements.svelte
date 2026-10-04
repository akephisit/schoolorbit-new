<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { captureRouteLoad, type RouteLoadResult } from '#lib/navigation/route-load.js';
	import { requireApiData } from '#lib/api/client.js';
	import { authStore } from '#lib/stores/auth.js';
	import { can } from '#lib/stores/permissions.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import * as Tabs from '#lib/components/ui/tabs/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import {
		Table,
		TableBody,
		TableCell,
		TableHead,
		TableHeader,
		TableRow
	} from '#lib/components/ui/table/index.js';
	import {
		Card,
		CardContent,
		CardDescription,
		CardHeader,
		CardTitle
	} from '#lib/components/ui/card/index.js';
	import {
		Plus,
		Search,
		Calendar,
		FileText,
		User as UserIcon,
		Trash2,
		Pencil,
		ExternalLink,
		LoaderCircle
	} from '@lucide/svelte';
	import {
		getAchievements,
		createAchievement,
		updateAchievement,
		deleteAchievement
	} from '#lib/api/achievement.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import type { Achievement, AchievementListFilter } from '#lib/types/achievement.js';
	import AchievementDialog from '#lib/components/achievement/AchievementDialog.svelte';
	import PrivateFileImage from '#lib/components/files/PrivateFileImage.svelte';
	import { toast } from 'svelte-sonner';

	// State
	let {
		initialAchievements
	}: {
		initialAchievements: Promise<
			RouteLoadResult<{ identityKey: string; records: Achievement[] | null }>
		>;
	} = $props();
	const request = new LatestRequest();
	let loaded = $state(false),
		loadError = $state(''),
		saving = $state(false);
	let readOwner = '',
		identityOwner = '',
		ownerEpoch = 0,
		draftEpoch = 0,
		disposed = false,
		consumed: typeof initialAchievements | null = null;
	let loading = $state(true);
	let achievements = $state<Achievement[]>([]);
	let searchTerm = $state('');
	let activeTab = $state('own'); // 'own' | 'all'

	// Dialog State
	let showDialog = $state(false);
	let selectedAchievement = $state<Achievement | null>(null);
	let showDeleteDialog = $state(false);
	let deleteId = $state<string | null>(null);
	let deleting = $state(false);

	// File Preview State
	let showFileDialog = $state(false);
	let viewingFileId = $state('');
	let viewingResourceId = $state('');

	// User & Permissions - permissions auto-loaded by authStore
	const user = $derived($authStore.user);
	const userId = $derived(user?.id || '');
	const permissions = $derived($can); // Use enhanced permission store

	// Permission checks - much simpler now!
	const canReadOwn = $derived(permissions.has(PERMISSIONS.ACHIEVEMENT_READ_OWN));
	const canReadAll = $derived(permissions.has(PERMISSIONS.ACHIEVEMENT_READ_ALL));
	const canCreateOwn = $derived(permissions.has(PERMISSIONS.ACHIEVEMENT_CREATE_OWN));
	const canCreateAll = $derived(permissions.has(PERMISSIONS.ACHIEVEMENT_CREATE_ALL));
	const canUpdateOwn = $derived(permissions.has(PERMISSIONS.ACHIEVEMENT_UPDATE_OWN));
	const canUpdateAll = $derived(permissions.has(PERMISSIONS.ACHIEVEMENT_UPDATE_ALL));
	const canDeleteOwn = $derived(permissions.has(PERMISSIONS.ACHIEVEMENT_DELETE_OWN));
	const canDeleteAll = $derived(permissions.has(PERMISSIONS.ACHIEVEMENT_DELETE_ALL));
	const canReadAchievements = $derived(canReadOwn || canReadAll);
	const canCreateAchievement = $derived(canCreateOwn || canCreateAll);
	const canUpdateAchievement = $derived(canUpdateOwn || canUpdateAll);
	const canDeleteAchievement = $derived(canDeleteOwn || canDeleteAll);

	// Derived state for filtering
	const filteredAchievements = $derived(
		achievements.filter((a) => {
			const staffName = `${a.user_first_name || ''} ${a.user_last_name || ''}`;
			const searchLower = searchTerm.toLowerCase();

			return (
				a.title.toLowerCase().includes(searchLower) ||
				(a.description || '').toLowerCase().includes(searchLower) ||
				staffName.toLowerCase().includes(searchLower)
			);
		})
	);

	const identityKey = $derived.by(() => {
		void user;
		void permissions;
		return appIdentityKey();
	});
	$effect.pre(() => {
		const operation = initialAchievements,
			identity = identityKey,
			mode = activeTab,
			allowed = canReadAchievements,
			all = canReadAll;
		untrack(() => {
			if (identityOwner !== identity) {
				identityOwner = identity;
				ownerEpoch++;
				saving = false;
				deleting = false;
				showDialog = false;
				showDeleteDialog = false;
				showFileDialog = false;
				viewingFileId = '';
				viewingResourceId = '';
			}
			if (mode === 'all' && !all) {
				activeTab = 'own';
				return;
			}
			const key = `${identity}|${mode}`,
				changed = readOwner !== key;
			readOwner = key;
			if (changed) {
				request.abort();
				achievements = [];
				loaded = false;
				loadError = '';
				loading = allowed;
			}
			if (!allowed) return;
			if (operation !== consumed) {
				consumed = operation;
				if (mode === 'own') {
					const t = request.begin();
					loading = true;
					loadError = '';
					void operation.then((r) => applyAchievements(r, t.revision, identity, mode));
					return;
				}
			}
			if (changed) void loadData();
		});
	});
	$effect.pre(() => {
		const opened = `${showDialog}|${showDeleteDialog}|${selectedAchievement?.id ?? ''}`;
		untrack(() => {
			void opened;
			draftEpoch++;
		});
	});
	onDestroy(() => {
		disposed = true;
		ownerEpoch++;
		draftEpoch++;
		request.abort();
	});
	function applyAchievements(
		r: Awaited<typeof initialAchievements>,
		revision: number,
		identity: string,
		mode: string
	) {
		if (!request.isCurrent(revision) || identity !== identityKey || mode !== activeTab) return;
		loading = false;
		if (!r.ok) {
			loadError = r.error;
			return;
		}
		if (r.data.identityKey !== identity) return;
		achievements = r.data.records ?? [];
		loaded = true;
	}
	async function loadData() {
		if (disposed || !canReadAchievements || !userId) return;
		const identity = identityKey,
			mode = activeTab,
			t = request.begin();
		loading = true;
		loadError = '';
		const filter: AchievementListFilter = mode === 'own' ? { user_id: userId } : {};
		const result = await captureRouteLoad(
			getAchievements(filter, { signal: t.signal }).then((reply) => ({
				identityKey: identity,
				records: requireApiData(reply, 'โหลดผลงานไม่สำเร็จ')
			})),
			'โหลดผลงานไม่สำเร็จ'
		);
		applyAchievements(result, t.revision, identity, mode);
	}
	function handleTabChange(value: string) {
		if (value === 'all' && !canReadAll) return;
		activeTab = value;
	}

	function achievementMatchesCurrentTab(achievement: Achievement) {
		return activeTab === 'all' || achievement.user_id === userId;
	}

	function replaceAchievement(achievement: Achievement) {
		if (!achievementMatchesCurrentTab(achievement)) {
			removeAchievement(achievement.id);
			return;
		}

		achievements = achievements.some((item) => item.id === achievement.id)
			? achievements.map((item) => (item.id === achievement.id ? achievement : item))
			: [achievement, ...achievements];
		achievements = achievements.sort(
			(a, b) =>
				new Date(b.achievement_date).getTime() - new Date(a.achievement_date).getTime() ||
				b.created_at.localeCompare(a.created_at)
		);
	}

	function removeAchievement(id: string) {
		achievements = achievements.filter((achievement) => achievement.id !== id);
	}

	function formatDate(dateStr: string) {
		return new Date(dateStr).toLocaleDateString('th-TH', {
			year: 'numeric',
			month: 'long',
			day: 'numeric'
		});
	}

	// Actions
	function viewFile(fileId: string, resourceId: string) {
		viewingFileId = fileId;
		viewingResourceId = resourceId;
		showFileDialog = true;
	}

	function openCreateDialog() {
		if (!canCreateAchievement) return;
		selectedAchievement = null;
		showDialog = true;
	}

	function openEditDialog(achievement: Achievement) {
		if (!(canUpdateAll || (canUpdateOwn && achievement.user_id === userId))) return;
		selectedAchievement = achievement;
		showDialog = true;
	}

	async function handleSave(payload: Partial<Achievement>) {
		if (disposed || saving || !showDialog) return;
		const target = selectedAchievement,
			epoch = ownerEpoch,
			draft = draftEpoch,
			targetUserId = payload.user_id || userId;
		const permitted = () =>
			payload.id
				? Boolean(
						target?.id === payload.id &&
						(canUpdateAll || (canUpdateOwn && target.user_id === userId))
					)
				: canCreateAll || (canCreateOwn && targetUserId === userId);
		if (!permitted()) return;
		const current = () => !disposed && epoch === ownerEpoch && permitted();
		const ownsDraft = () => current() && showDialog && draft === draftEpoch;
		saving = true;
		try {
			const fields = {
				title: payload.title ?? '',
				description: payload.description,
				achievement_date: payload.achievement_date ?? '',
				image_file_id: payload.image_file_id
			};
			const reply = payload.id
				? await updateAchievement(payload.id, fields)
				: await createAchievement({ ...fields, user_id: targetUserId });
			const saved = requireApiData(reply, 'บันทึกข้อมูลไม่สำเร็จ');
			if (!current()) return;
			request.abort();
			loading = false;
			loadError = '';
			if (loaded) replaceAchievement(saved);
			else await loadData();
			if (ownsDraft()) {
				toast.success('บันทึกข้อมูลเรียบร้อย');
				showDialog = false;
			}
		} catch (error) {
			if (ownsDraft())
				toast.error(error instanceof Error ? error.message : 'บันทึกข้อมูลไม่สำเร็จ');
		} finally {
			if (current()) saving = false;
		}
	}

	function handleDelete(id: string) {
		const achievement = achievements.find((item) => item.id === id);
		if (!achievement || !(canDeleteAll || (canDeleteOwn && achievement.user_id === userId))) return;
		deleteId = id;
		showDeleteDialog = true;
	}

	async function confirmDelete() {
		const target = achievements.find((item) => item.id === deleteId);
		if (disposed || deleting || !showDeleteDialog || !target) return;
		const permitted = () => canDeleteAll || (canDeleteOwn && target.user_id === userId);
		if (!permitted()) return;
		const epoch = ownerEpoch,
			draft = draftEpoch,
			current = () => !disposed && epoch === ownerEpoch && permitted();
		const ownsDraft = () => current() && showDeleteDialog && draft === draftEpoch;
		deleting = true;
		try {
			const reply = await deleteAchievement(target.id);
			if (!reply.success) throw new Error(reply.error || 'ลบข้อมูลไม่สำเร็จ');
			if (!current()) return;
			request.abort();
			loading = false;
			if (loaded) removeAchievement(target.id);
			else await loadData();
			if (ownsDraft()) {
				toast.success('ลบข้อมูลเรียบร้อย');
				showDeleteDialog = false;
				deleteId = null;
			}
		} catch (error) {
			if (ownsDraft()) toast.error(error instanceof Error ? error.message : 'ลบข้อมูลไม่สำเร็จ');
		} finally {
			if (current()) deleting = false;
		}
	}
</script>

<PageShell
	title="ผลงานที่บันทึกเอง"
	description="บันทึกและจัดการผลงาน รางวัล หรือหลักฐานอื่นที่ไม่ได้ออกจากระบบเกียรติบัตร"
>
	{#snippet actions()}
		{#if canCreateAchievement}
			<Button disabled={!loaded} onclick={openCreateDialog}>
				<Plus class="w-4 h-4 mr-2" />
				เพิ่มรายการใหม่
			</Button>
		{/if}
	{/snippet}

	{#if !canReadAchievements}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ดูข้อมูลเกียรติบัตร"
			description="บัญชีนี้เข้า module เกียรติบัตรได้ แต่ยังไม่มีสิทธิ์อ่านรายการผลงานของตนเองหรือภาพรวม"
		/>
	{:else}
		{#if canReadAll}
			<Tabs.Root value={activeTab} onValueChange={handleTabChange} class="w-full">
				<Tabs.List class="grid w-full grid-cols-2 max-w-[400px] mb-4">
					<Tabs.Trigger value="own">ของฉัน</Tabs.Trigger>
					<Tabs.Trigger value="all">ภาพรวม (ทั้งหมด)</Tabs.Trigger>
				</Tabs.List>
			</Tabs.Root>
		{/if}

		<Card>
			<CardHeader>
				<CardTitle>รายการผลงาน{activeTab === 'all' ? 'ทั้งหมด' : 'ของฉัน'}</CardTitle>
				<CardDescription
					>แสดงรายการผลงาน{activeTab === 'all'
						? 'ของบุคลากรในระบบ'
						: 'ที่คุณบันทึกไว้'}</CardDescription
				>
			</CardHeader>
			<CardContent>
				<!-- Search -->
				<div class="flex items-center gap-2 mb-6">
					<div class="relative flex-1 max-w-sm">
						<Search class="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
						<Input
							type="search"
							placeholder="ค้นหาตามชื่อผลงาน หรือชื่อเจ้าของ..."
							class="pl-9"
							bind:value={searchTerm}
						/>
					</div>
				</div>

				{#if loadError}<PageState
						variant="error"
						title="โหลดผลงานไม่สำเร็จ"
						description={loadError}
						actionLabel="ลองโหลดผลงานอีกครั้ง"
						onaction={loadData}
					/>{/if}
				{#if loading && loaded}<p role="status">กำลังอัปเดตผลงาน</p>{/if}
				<!-- Table -->
				<div class="rounded-md border">
					<Table>
						<TableHeader>
							<TableRow>
								<TableHead>วันที่ได้รับ</TableHead>
								<TableHead>ชื่อผลงาน / รางวัล</TableHead>
								<TableHead>เจ้าของผลงาน</TableHead>
								<TableHead>หลักฐาน</TableHead>
								<TableHead class="text-right">จัดการ</TableHead>
							</TableRow>
						</TableHeader>
						<TableBody>
							{#if loading && !loaded}
								<TableRow>
									<TableCell colspan={5} class="p-0">
										<div role="status" aria-label="กำลังโหลดผลงาน">
											<PageSkeleton variant="table" rows={5} columns={5} />
										</div>
									</TableCell>
								</TableRow>
							{:else if !loaded}<TableRow><TableCell colspan={5}>รอข้อมูลผลงาน</TableCell></TableRow
								>
							{:else if filteredAchievements.length === 0}
								<TableRow>
									<TableCell colspan={5} class="h-24">
										<PageState
											title="ไม่พบข้อมูล"
											description="ลองปรับคำค้นหา หรือเพิ่มรายการใหม่เมื่อมีสิทธิ์"
											actionLabel={canCreateAchievement ? 'เพิ่มรายการใหม่' : undefined}
											onaction={openCreateDialog}
										/>
									</TableCell>
								</TableRow>
							{:else}
								{#each filteredAchievements as achievement (achievement.id)}
									<TableRow>
										<TableCell class="font-medium whitespace-nowrap">
											<div class="flex items-center gap-2">
												<Calendar class="w-4 h-4 text-muted-foreground" />
												{formatDate(achievement.achievement_date)}
											</div>
										</TableCell>
										<TableCell>
											<div class="font-medium">{achievement.title}</div>
											{#if achievement.description}
												<div class="text-xs text-muted-foreground line-clamp-1">
													{achievement.description}
												</div>
											{/if}
										</TableCell>
										<TableCell>
											{#if achievement.user_first_name}
												<div class="flex items-center gap-2">
													<UserIcon class="w-4 h-4 text-muted-foreground" />
													<span>
														{achievement.user_first_name}
														{achievement.user_last_name}
													</span>
												</div>
											{:else}
												<span class="text-muted-foreground text-xs">Unknown User</span>
											{/if}
										</TableCell>
										<TableCell>
											{#if achievement.image_file_id}
												<button
													type="button"
													onclick={() => viewFile(achievement.image_file_id!, achievement.id)}
													class="flex items-center gap-1 text-primary hover:underline text-sm bg-transparent border-0 p-0 cursor-pointer"
												>
													<FileText class="w-4 h-4" />
													ดูไฟล์
												</button>
											{:else}
												<span class="text-muted-foreground text-xs">-</span>
											{/if}
										</TableCell>
										<TableCell class="text-right">
											<div class="flex justify-end gap-2">
												{#if canReadAll || canUpdateAll}
													<Button
														variant="ghost"
														size="icon"
														class="h-8 w-8"
														href={`/staff/view/${achievement.user_id}`}
														data-sveltekit-preload-data={false}
														title="ดูโปรไฟล์"
													>
														<ExternalLink class="w-4 h-4" />
													</Button>
												{/if}

												{#if canUpdateAll || (canUpdateOwn && achievement.user_id === userId)}
													<Button
														variant="ghost"
														size="icon"
														class="h-8 w-8 hover:bg-muted"
														aria-label={`แก้ไขผลงาน ${achievement.title}`}
														onclick={() => openEditDialog(achievement)}
														title="แก้ไข"
													>
														<Pencil class="w-4 h-4" />
													</Button>
												{/if}

												{#if canDeleteAll || (canDeleteOwn && achievement.user_id === userId)}
													<Button
														variant="ghost"
														size="icon"
														class="h-8 w-8 text-destructive hover:text-destructive hover:bg-destructive/10"
														disabled={saving || deleting}
														aria-label={`ลบผลงาน ${achievement.title}`}
														onclick={() => handleDelete(achievement.id)}
														title="ลบ"
													>
														<Trash2 class="w-4 h-4" />
													</Button>
												{/if}
											</div>
										</TableCell>
									</TableRow>
								{/each}
							{/if}
						</TableBody>
					</Table>
				</div>
			</CardContent>
		</Card>

		{#if showDialog && (canCreateAchievement || canUpdateAchievement)}
			<AchievementDialog
				open={showDialog}
				achievement={selectedAchievement}
				{userId}
				canSelectUser={!selectedAchievement && canCreateAll}
				busy={saving}
				onclose={() => (showDialog = false)}
				onsave={handleSave}
			/>
		{/if}

		<!-- File Preview Dialog -->
		<Dialog.Root bind:open={showFileDialog}>
			<Dialog.Content
				class="max-w-[95vw] md:max-w-7xl max-h-[95vh] overflow-hidden flex flex-col p-0 gap-0"
			>
				<div
					class="relative flex-1 bg-muted/30 min-h-[200px] flex items-center justify-center overflow-auto p-4"
				>
					{#if showFileDialog && viewingFileId}
						<PrivateFileImage
							fileId={viewingFileId}
							resourceId={viewingResourceId}
							alt="Preview"
							class="max-w-full max-h-[80vh] object-contain shadow-sm rounded-sm"
						/>
					{:else}
						<div class="text-center p-8">
							<p class="mb-4 text-muted-foreground">ไม่สามารถแสดงตัวอย่างไฟล์ประเภทนี้ได้</p>
						</div>
					{/if}
				</div>
				<div class="p-4 border-t flex justify-end bg-background">
					<Button variant="outline" onclick={() => (showFileDialog = false)}>ปิด</Button>
				</div>
			</Dialog.Content>
		</Dialog.Root>

		<!-- Delete Confirmation Dialog -->
		{#if canDeleteAchievement}
			<Dialog.Root bind:open={showDeleteDialog}>
				<Dialog.Content class="sm:max-w-[425px]">
					<Dialog.Header>
						<Dialog.Title>ยืนยันการลบข้อมูล</Dialog.Title>
						<Dialog.Description>
							คุณต้องการลบรายการนี้ใช่หรือไม่? การกระทำนี้ไม่สามารถย้อนกลับได้
						</Dialog.Description>
					</Dialog.Header>
					<Dialog.Footer>
						<Button variant="outline" onclick={() => (showDeleteDialog = false)} disabled={deleting}
							>ยกเลิก</Button
						>
						<Button variant="destructive" onclick={confirmDelete} disabled={deleting}>
							{#if deleting}<LoaderCircle class="mr-2 h-4 w-4 animate-spin" />{/if}
							ลบข้อมูล
						</Button>
					</Dialog.Footer>
				</Dialog.Content>
			</Dialog.Root>
		{/if}
	{/if}
</PageShell>
