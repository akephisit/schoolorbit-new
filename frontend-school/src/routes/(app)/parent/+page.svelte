<script lang="ts">
	import type { PageProps } from './$types';
	import { Button } from '#lib/components/ui/button/index.js';
	import { goto, preloadData } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { onDestroy, untrack } from 'svelte';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { captureRouteLoad } from '#lib/navigation/route-load.js';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { authStore } from '#lib/stores/auth.js';
	import { can } from '#lib/stores/permissions.js';
	import {
		listParentAcademicContextOptions,
		type AcademicContextOptionsResponse
	} from '#lib/api/academic-context.js';
	import { resolveScopedAcademicYearUrl } from '#lib/academic-context/scoped-year.js';
	import { getOwnParentProfile, type ParentProfile } from '#lib/api/parents.js';
	import ScopedAcademicYearSelect from '#lib/components/academic-context/ScopedAcademicYearSelect.svelte';
	import { Card } from '#lib/components/ui/card/index.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { User, ChevronRight } from '@lucide/svelte';
	import PrivateFileImage from '#lib/components/files/PrivateFileImage.svelte';

	let { data }: PageProps = $props();
	const identityKey = $derived.by(() => {
		void $authStore;
		void $can;
		return appIdentityKey();
	});
	const ownerKey = $derived(`${identityKey}|${data.requestKey}`);
	const allowed = $derived($authStore.user?.user_type === 'parent');
	let contextOptions = $state.raw<AcademicContextOptionsResponse | null>(null);
	let selectedYearId = $state('');
	let profile = $state.raw<ParentProfile | null>(null);
	let loading = $state(true),
		loaded = $state(false),
		error = $state('');
	let contextLoading = $state(true),
		contextError = $state('');
	let disposed = false,
		owner = '';
	const contextRequest = new LatestRequest(),
		profileRequest = new LatestRequest();
	let consumedContext: typeof data.context | null = null,
		consumedProfile: typeof data.profile | null = null;

	$effect.pre(() => {
		const key = ownerKey,
			a = data.context,
			b = data.profile,
			canRead = allowed;
		untrack(() => {
			if (owner !== key || !canRead) {
				owner = key;

				contextRequest.abort();
				profileRequest.abort();
				contextOptions = null;
				selectedYearId = '';
				profile = null;
				loaded = false;
				loading = canRead;
				contextLoading = canRead;
				error = '';
				contextError = '';
			}
			if (!canRead) return;
			if (a !== consumedContext) {
				consumedContext = a;
				const t = contextRequest.begin();
				contextLoading = true;
				void a.then((v) => applyContext(v, t.revision, key));
			}
			if (b !== consumedProfile) {
				consumedProfile = b;
				const t = profileRequest.begin();
				loading = true;
				void b.then((v) => applyProfile(v, t.revision, key));
			}
		});
	});
	onDestroy(() => {
		disposed = true;

		contextRequest.abort();
		profileRequest.abort();
	});
	function current(key: string) {
		return !disposed && allowed && key === ownerKey;
	}
	function applyContext(v: Awaited<typeof data.context>, revision: number, key: string) {
		if (!current(key) || !contextRequest.isCurrent(revision)) return;
		contextLoading = false;
		if (!v.ok) {
			contextError = v.error;
			return;
		}
		if (v.data.ownerKey !== key) return;
		contextError = '';
		contextOptions = v.data.options;
		selectedYearId = v.data.academicYearId;
		if (v.data.replaceHref) {
			const url = new URL(v.data.replaceHref);
			goto(resolve('parent') + url.search, {
				shallow: true,
				replace: true,
				state: page.state
			});
		}
	}
	function applyProfile(v: Awaited<typeof data.profile>, revision: number, key: string) {
		if (!current(key) || !profileRequest.isCurrent(revision)) return;
		loading = false;
		if (!v.ok) {
			error = v.error;
			return;
		}
		if (v.data.ownerKey !== key) return;
		profile = v.data.profile;
		loaded = v.data.profile !== null;
		error = '';
	}
	async function loadProfile() {
		if (!allowed || disposed || !selectedYearId) return;
		const key = ownerKey,
			yearId = selectedYearId,
			t = profileRequest.begin();
		loading = true;
		error = '';
		const v = await captureRouteLoad(
			getOwnParentProfile(yearId, { signal: t.signal }).then((profile) => ({
				ownerKey: key,
				profile
			})),
			'โหลดข้อมูลนักเรียนไม่สำเร็จ'
		);
		applyProfile(v, t.revision, key);
	}
	async function retryContext() {
		const key = ownerKey,
			t = contextRequest.begin();
		contextLoading = true;
		contextError = '';
		const v = await captureRouteLoad(
			listParentAcademicContextOptions(t.signal).then((options) => {
				const selection = resolveScopedAcademicYearUrl(options, new URL(data.requestHref));
				return {
					ownerKey: key,
					options,
					academicYearId: selection.academicYearId ?? '',
					replaceHref: selection.replaceUrl?.href ?? null
				};
			}),
			'โหลดประวัติปีการศึกษาไม่สำเร็จ'
		);
		if (!current(key) || !contextRequest.isCurrent(t.revision)) return;
		applyContext(v, t.revision, key);
		if (v.ok && selectedYearId) await loadProfile();
	}

	async function changeAcademicYear(yearId: string) {
		if (!contextOptions?.years.some((y) => y.id === yearId) || yearId === selectedYearId) return;
		await goto(resolve(`parent?academicYearId=${encodeURIComponent(yearId)}`), {
			reset: false
		});
	}

	async function goToStudent(id: string) {
		if (!allowed || disposed || !selectedYearId) return;
		const key = ownerKey;
		const target = resolve(
			`parent/student/${encodeURIComponent(id)}?academicYearId=${encodeURIComponent(selectedYearId)}`
		);

		await preloadData(target);
		if (!current(key)) return;
		await goto(target);
	}
</script>

<PageShell
	title={`สวัสดี, คุณ${profile?.first_name || '...'} ${profile?.last_name || ''}`}
	description="ติดตามการเรียนและความเป็นอยู่ของบุตรหลาน"
>
	<Button
		variant="outline"
		disabled={loading || contextLoading || !selectedYearId}
		onclick={loadProfile}>โหลดข้อมูลใหม่</Button
	>
	{#if contextOptions && contextOptions.years.length > 0}
		<div class="flex max-w-sm flex-col gap-2 rounded-xl border bg-card p-4">
			<Label for="parent-year">ปีการศึกษา</Label>
			<ScopedAcademicYearSelect
				id="parent-year"
				years={contextOptions.years}
				value={selectedYearId}
				disabled={contextLoading}
				onchange={changeAcademicYear}
			/>
		</div>
	{/if}

	{#if contextError}<PageState
			variant="error"
			title="โหลดประวัติปีการศึกษาไม่สำเร็จ"
			description={contextError}
			actionLabel="ลองบริบทอีกครั้ง"
			onaction={retryContext}
		/>{/if}
	{#if error}<PageState
			variant="error"
			title="โหลดข้อมูลไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={loadProfile}
		/>{/if}
	<div data-testid="parent-profile-region" aria-busy={loading || contextLoading}>
		{#if loading && loaded}<p
				role="status"
				aria-label="กำลังอัปเดตข้อมูล"
				class="text-muted-foreground text-sm"
			>
				กำลังอัปเดตข้อมูล…
			</p>{/if}
		{#if contextLoading || (loading && !loaded)}
			<div role="status" aria-label="กำลังโหลดข้อมูลผู้ปกครอง">
				<PageSkeleton variant="cards" rows={3} />
			</div>
		{:else if contextOptions && !contextError && contextOptions.years.length === 0}
			<PageState
				title="ยังไม่มีประวัติปีการศึกษาสำหรับบัญชีนี้"
				description="กรุณาติดต่อโรงเรียนเพื่อตรวจสอบการเชื่อมโยงบุตรหลาน"
			/>
		{:else if profile}
			<!-- Children List -->
			<div>
				<h2 class="text-xl font-semibold mb-4">บุตรหลานของคุณ</h2>

				{#if profile.children.length === 0}
					<PageState
						title="ไม่พบข้อมูลบุตรหลาน"
						description="ยังไม่มีข้อมูลนักเรียนที่เชื่อมโยงกับบัญชีนี้ กรุณาติดต่อทางโรงเรียนหากข้อมูลไม่ถูกต้อง"
					/>
				{:else}
					<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
						{#each profile.children as child (child.id)}
							<Card
								class="overflow-hidden hover:shadow-lg transition-all cursor-pointer group"
								onclick={() => goToStudent(String(child.id))}
							>
								<div class="p-6">
									<div class="flex items-start gap-4">
										<div
											class="w-16 h-16 rounded-full bg-primary/10 flex items-center justify-center overflow-hidden border-2 border-background shadow-sm"
										>
											{#if child.profile_image_file_id}
												<PrivateFileImage
													fileId={child.profile_image_file_id}
													resourceId={child.id}
													alt={child.first_name}
													class="w-full h-full object-cover"
												/>
											{:else}
												<User class="w-8 h-8 text-primary" />
											{/if}
										</div>
										<div class="flex-1 min-w-0">
											<h3
												class="font-semibold text-lg truncate group-hover:text-primary transition-colors"
											>
												{child.first_name}
												{child.last_name}
											</h3>
											<p class="text-sm text-muted-foreground mb-1">
												รหัสนักเรียน: {child.student_id || '-'}
											</p>
											<div class="flex flex-wrap gap-2">
												<Badge variant="secondary" class="font-normal">
													{child.grade_level || 'ไม่ระบุชั้น'}
												</Badge>
												<Badge variant="outline" class="font-normal text-muted-foreground">
													ห้อง {child.homeroom || '-'}
												</Badge>
											</div>
										</div>
										<ChevronRight
											class="w-5 h-5 text-muted-foreground/30 group-hover:text-primary transition-colors"
										/>
									</div>
								</div>
								<div class="bg-muted/30 px-6 py-3 border-t flex justify-between items-center">
									<span class="text-xs text-muted-foreground">สถานะ: {child.relationship}</span>
									<span class="text-xs font-medium text-primary flex items-center">
										ดูรายละเอียด
									</span>
								</div>
							</Card>
						{/each}
					</div>
				{/if}
			</div>
		{:else if loaded && !contextError && !error}
			<PageState
				title="ไม่พบข้อมูลผู้ปกครอง"
				description="ไม่พบโปรไฟล์ผู้ปกครองของบัญชีนี้ กรุณาติดต่อผู้ดูแลระบบ"
			/>
		{/if}
	</div>
</PageShell>
