<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { onMount } from 'svelte';
	import {
		listParentAcademicContextOptions,
		type AcademicContextOptionsResponse
	} from '$lib/api/academic-context';
	import {
		resolveScopedAcademicYearUrl,
		urlWithAcademicYear
	} from '$lib/academic-context/scoped-year';
	import { getOwnParentProfile, type ParentProfile } from '$lib/api/parents';
	import ScopedAcademicYearSelect from '$lib/components/academic-context/ScopedAcademicYearSelect.svelte';
	import { Card } from '$lib/components/ui/card';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import { Label } from '$lib/components/ui/label';
	import { User, ChevronRight } from 'lucide-svelte';
	import PrivateFileImage from '$lib/components/files/PrivateFileImage.svelte';

	let contextOptions = $state<AcademicContextOptionsResponse | null>(null);
	let selectedYearId = $state('');
	let profile = $state<ParentProfile | null>(null);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let revision = 0;

	function goToStudent(id: string): void {
		if (!selectedYearId) return;
		void goto(
			resolve(
				`/parent/student/${encodeURIComponent(id)}?academicYearId=${encodeURIComponent(selectedYearId)}`
			)
		);
	}

	async function loadProfile() {
		const current = ++revision;
		if (!selectedYearId) {
			profile = null;
			loading = false;
			return;
		}
		loading = true;
		error = null;
		try {
			const loaded = await getOwnParentProfile(selectedYearId);
			if (current !== revision) return;
			profile = loaded;
		} catch (e) {
			if (current !== revision) return;
			console.error('Failed to load profile:', e);
			error = e instanceof Error ? e.message : 'ไม่สามารถโหลดข้อมูลได้';
		} finally {
			if (current === revision) loading = false;
		}
	}

	async function initialize(): Promise<void> {
		const current = ++revision;
		loading = true;
		error = null;
		try {
			const options = await listParentAcademicContextOptions();
			if (current !== revision) return;
			contextOptions = options;
			const selection = resolveScopedAcademicYearUrl(options, page.url);
			selectedYearId = selection.academicYearId ?? '';

			if (selection.replaceUrl) {
				await goto(resolve(`/parent${selection.replaceUrl.search}${selection.replaceUrl.hash}`), {
					replaceState: true,
					noScroll: true,
					keepFocus: true
				});
				if (current !== revision) return;
			}

			if (!selectedYearId) {
				profile = null;
				loading = false;
				return;
			}
			await loadProfile();
		} catch (loadError) {
			if (current !== revision) return;
			error = loadError instanceof Error ? loadError.message : 'โหลดประวัติปีการศึกษาไม่สำเร็จ';
			loading = false;
		}
	}

	async function changeAcademicYear(academicYearId: string): Promise<void> {
		if (academicYearId === selectedYearId) return;
		const current = ++revision;
		selectedYearId = academicYearId;
		loading = true;
		error = null;
		try {
			const nextUrl = urlWithAcademicYear(page.url, academicYearId);
			await goto(resolve(`/parent${nextUrl.search}${nextUrl.hash}`), {
				replaceState: true,
				noScroll: true,
				keepFocus: true
			});
			if (current !== revision) return;
			await loadProfile();
		} catch (loadError) {
			if (current !== revision) return;
			error = loadError instanceof Error ? loadError.message : 'เปลี่ยนปีการศึกษาไม่สำเร็จ';
			loading = false;
		}
	}

	function retry(): void {
		if (contextOptions && selectedYearId) void loadProfile();
		else void initialize();
	}

	onMount(() => {
		void initialize();
	});
</script>

<PageShell
	title={`สวัสดี, คุณ${profile?.first_name || '...'} ${profile?.last_name || ''}`}
	description="ติดตามการเรียนและความเป็นอยู่ของบุตรหลาน"
>
	{#if contextOptions && contextOptions.years.length > 0}
		<div class="flex max-w-sm flex-col gap-2 rounded-xl border bg-card p-4">
			<Label for="parent-year">ปีการศึกษา</Label>
			<ScopedAcademicYearSelect
				id="parent-year"
				years={contextOptions.years}
				value={selectedYearId}
				disabled={loading}
				onchange={changeAcademicYear}
			/>
		</div>
	{/if}

	{#if loading}
		<PageSkeleton variant="cards" rows={3} />
	{:else if error}
		<PageState
			variant="error"
			title="โหลดข้อมูลผู้ปกครองไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={retry}
		/>
	{:else if contextOptions && contextOptions.years.length === 0}
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
	{:else}
		<PageState
			title="ไม่พบข้อมูลผู้ปกครอง"
			description="ไม่พบโปรไฟล์ผู้ปกครองของบัญชีนี้ กรุณาติดต่อผู้ดูแลระบบ"
		/>
	{/if}
</PageShell>
