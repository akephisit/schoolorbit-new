<script lang="ts">
	import { Card } from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import { Users, GraduationCap, Calendar, FileText, Settings } from 'lucide-svelte';

	// Quick stats (placeholder - should fetch from API)
	let stats = $state({
		totalStaff: 0,
		totalStudents: 0,
		activeClasses: 0
	});
</script>

<svelte:head>
	<title>Staff Dashboard - SchoolOrbit</title>
</svelte:head>

<div class="space-y-6">
	<!-- Header -->
	<div>
		<h1 class="text-3xl font-bold text-foreground">Staff Dashboard</h1>
		<p class="text-muted-foreground mt-1">ภาพรวมและการจัดการระบบ</p>
	</div>

	<!-- Quick Stats -->
	<div class="grid grid-cols-1 md:grid-cols-3 gap-6">
		<Card class="p-6 hover:shadow-md transition-shadow">
			<div class="flex items-start justify-between">
				<div class="space-y-2">
					<p class="text-sm text-muted-foreground font-medium">บุคลากรทั้งหมด</p>
					<p class="text-3xl font-bold text-foreground">{stats.totalStaff}</p>
				</div>
				<div class="w-12 h-12 bg-primary/10 rounded-lg flex items-center justify-center">
					<Users class="w-6 h-6 text-primary" />
				</div>
			</div>
		</Card>

		<Card class="p-6 hover:shadow-md transition-shadow">
			<div class="flex items-start justify-between">
				<div class="space-y-2">
					<p class="text-sm text-muted-foreground font-medium">นักเรียนทั้งหมด</p>
					<p class="text-3xl font-bold text-foreground">{stats.totalStudents}</p>
				</div>
				<div class="w-12 h-12 bg-blue-500/10 rounded-lg flex items-center justify-center">
					<GraduationCap class="w-6 h-6 text-blue-500" />
				</div>
			</div>
		</Card>

		<Card class="p-6 hover:shadow-md transition-shadow">
			<div class="flex items-start justify-between">
				<div class="space-y-2">
					<p class="text-sm text-muted-foreground font-medium">ห้องเรียนที่เปิด</p>
					<p class="text-3xl font-bold text-foreground">{stats.activeClasses}</p>
				</div>
				<div class="w-12 h-12 bg-green-500/10 rounded-lg flex items-center justify-center">
					<Calendar class="w-6 h-6 text-green-500" />
				</div>
			</div>
		</Card>
	</div>

	<!-- Quick Actions -->
	<Card class="p-6">
		<h2 class="text-xl font-semibold mb-4">เมนูด่วน</h2>
		<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
			<Button variant="outline" class="h-auto py-4 flex-col gap-2" href="/staff/manage">
				<Users class="w-6 h-6" />
				<span>จัดการบุคลากร</span>
			</Button>

			<Button variant="outline" class="h-auto py-4 flex-col gap-2" href="/staff/students">
				<GraduationCap class="w-6 h-6" />
				<span>จัดการนักเรียน</span>
			</Button>

			<Button variant="outline" class="h-auto py-4 flex-col gap-2" href="/departments">
				<FileText class="w-6 h-6" />
				<span>ฝ่าย/แผนก</span>
			</Button>

			<Button variant="outline" class="h-auto py-4 flex-col gap-2" href="/settings">
				<Settings class="w-6 h-6" />
				<span>ตั้งค่า</span>
			</Button>
		</div>
	</Card>

	<!-- Recent Activities (placeholder) -->
	<Card class="p-6">
		<h2 class="text-xl font-semibold mb-4">กิจกรรมล่าสุด</h2>
		<div class="text-center py-8 text-muted-foreground">
			<p>ยังไม่มีกิจกรรม</p>
		</div>
	</Card>
</div>
