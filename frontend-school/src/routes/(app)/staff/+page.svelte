<script lang="ts">
	import { onMount } from 'svelte';
	import {
		Card,
		CardContent,
		CardDescription,
		CardHeader,
		CardTitle
	} from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { getStaffDashboard, type StaffDashboardOverview } from '$lib/api/staff';
	import { PERMISSION_MODULES } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import {
		BookOpen,
		Building2,
		Calendar,
		FileText,
		GraduationCap,
		Icon as LucideIcon,
		RefreshCw,
		Settings,
		ShieldCheck,
		Users
	} from 'lucide-svelte';

	let stats = $state<StaffDashboardOverview | null>(null);
	let loadingStats = $state(true);
	let statsError = $state('');

	let canOpenStaffModule = $derived($can.hasModule(PERMISSION_MODULES.STAFF_PROFILE));
	let canOpenStudentModule = $derived($can.hasModule(PERMISSION_MODULES.STUDENT));
	let canOpenRolesModule = $derived($can.hasModule(PERMISSION_MODULES.ROLES));
	let canOpenSettingsModule = $derived($can.hasModule(PERMISSION_MODULES.SETTINGS));

	const numberFormatter = new Intl.NumberFormat('th-TH');

	type StatCard = {
		label: string;
		value: number;
		description: string;
		toneClass: string;
		icon: typeof LucideIcon;
	};

	let summaryCards = $derived.by((): StatCard[] =>
		stats
			? [
					{
						label: 'บุคลากรทั้งหมด',
						value: stats.totalStaff,
						description: 'ครูและเจ้าหน้าที่ที่ใช้งานอยู่',
						toneClass: 'bg-sky-500/10 text-sky-600',
						icon: Users
					},
					{
						label: 'นักเรียนทั้งหมด',
						value: stats.totalStudents,
						description: 'นักเรียนสถานะใช้งานอยู่',
						toneClass: 'bg-emerald-500/10 text-emerald-600',
						icon: GraduationCap
					},
					{
						label: 'ห้องเรียนที่เปิด',
						value: stats.activeClassrooms,
						description: 'ห้องเรียน active ในระบบ',
						toneClass: 'bg-amber-500/10 text-amber-600',
						icon: Building2
					}
				]
			: []
	);

	async function loadDashboard() {
		loadingStats = true;
		statsError = '';

		try {
			const response = await getStaffDashboard();
			if (!response.success || !response.data) {
				throw new Error(response.error || 'ไม่สามารถโหลดภาพรวมโรงเรียนได้');
			}
			stats = response.data;
		} catch (error) {
			statsError = error instanceof Error ? error.message : 'ไม่สามารถโหลดภาพรวมโรงเรียนได้';
		} finally {
			loadingStats = false;
		}
	}

	onMount(() => {
		void loadDashboard();
	});
</script>

<svelte:head>
	<title>แดชบอร์ดบุคลากร - SchoolOrbit</title>
</svelte:head>

<PageShell title="แดชบอร์ดบุคลากร" description="ภาพรวมโรงเรียนและทางลัดสำหรับการทำงานประจำวัน">
	{#if loadingStats}
		<PageSkeleton variant="cards" rows={3} />
	{:else if statsError}
		<PageState
			variant="error"
			title="โหลดภาพรวมโรงเรียนไม่สำเร็จ"
			description={statsError}
			actionLabel="ลองอีกครั้ง"
			onaction={loadDashboard}
		/>
	{:else if stats}
		<div class="grid gap-4 md:grid-cols-3">
			{#each summaryCards as item (item.label)}
				{@const Icon = item.icon}
				<Card class="overflow-hidden">
					<CardContent class="p-5">
						<div class="flex items-start justify-between gap-4">
							<div class="min-w-0 space-y-2">
								<p class="text-muted-foreground text-sm font-medium">{item.label}</p>
								<p class="text-foreground text-3xl font-semibold tracking-normal">
									{numberFormatter.format(item.value)}
								</p>
								<p class="text-muted-foreground text-xs">{item.description}</p>
							</div>
							<div
								class={`flex h-11 w-11 shrink-0 items-center justify-center rounded-lg ${item.toneClass}`}
							>
								<Icon class="h-5 w-5" />
							</div>
						</div>
					</CardContent>
				</Card>
			{/each}
		</div>
	{/if}

	<div class="grid gap-6 xl:grid-cols-[minmax(0,1fr)_360px]">
		<Card>
			<CardHeader>
				<CardTitle>เมนูด่วน</CardTitle>
				<CardDescription>ทางลัดจะแสดงตามสิทธิ์ของบัญชีที่ใช้งานอยู่</CardDescription>
			</CardHeader>
			<CardContent>
				<div class="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
					<Button variant="outline" class="h-auto justify-start gap-3 p-4" href="/staff/timetable">
						<Calendar class="h-5 w-5 text-sky-600" />
						<span class="text-left">ตารางสอนของฉัน</span>
					</Button>

					{#if canOpenStaffModule}
						<Button variant="outline" class="h-auto justify-start gap-3 p-4" href="/staff/manage">
							<Users class="h-5 w-5 text-sky-600" />
							<span class="text-left">จัดการบุคลากร</span>
						</Button>
					{/if}

					{#if canOpenStudentModule}
						<Button variant="outline" class="h-auto justify-start gap-3 p-4" href="/staff/students">
							<GraduationCap class="h-5 w-5 text-emerald-600" />
							<span class="text-left">จัดการนักเรียน</span>
						</Button>
					{/if}

					{#if canOpenRolesModule}
						<Button
							variant="outline"
							class="h-auto justify-start gap-3 p-4"
							href="/staff/organization"
						>
							<FileText class="h-5 w-5 text-violet-600" />
							<span class="text-left">โครงสร้างโรงเรียน</span>
						</Button>
					{/if}

					{#if canOpenSettingsModule}
						<Button
							variant="outline"
							class="h-auto justify-start gap-3 p-4"
							href="/staff/school-settings"
						>
							<ShieldCheck class="h-5 w-5 text-amber-600" />
							<span class="text-left">ตั้งค่าโรงเรียน</span>
						</Button>
					{/if}

					<Button variant="outline" class="h-auto justify-start gap-3 p-4" href="/staff/settings">
						<Settings class="h-5 w-5 text-muted-foreground" />
						<span class="text-left">ตั้งค่าบัญชี</span>
					</Button>
				</div>
			</CardContent>
		</Card>

		<Card>
			<CardHeader>
				<CardTitle>สถานะข้อมูล</CardTitle>
				<CardDescription>ตัวเลขรวมไม่เปิดเผยรายชื่อหรือข้อมูลส่วนบุคคล</CardDescription>
			</CardHeader>
			<CardContent class="space-y-4">
				<div class="rounded-lg border bg-muted/30 p-4">
					<div class="flex items-start gap-3">
						<BookOpen class="mt-0.5 h-5 w-5 text-emerald-600" />
						<div class="space-y-1">
							<p class="font-medium">ข้อมูลภาพรวมโรงเรียน</p>
							<p class="text-muted-foreground text-sm">
								ครูทุกคนเห็นจำนวนรวมของบุคลากร นักเรียน และห้องเรียนที่เปิดอยู่ได้
							</p>
						</div>
					</div>
				</div>

				<Button variant="outline" class="w-full gap-2" onclick={loadDashboard}>
					<RefreshCw class="h-4 w-4" />
					รีเฟรชข้อมูล
				</Button>
			</CardContent>
		</Card>
	</div>
</PageShell>
