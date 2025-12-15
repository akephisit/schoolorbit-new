<script lang="ts">
	import {
		Users,
		GraduationCap,
		BookOpen,
		School,
		TrendingUp,
		Calendar,
		Bell,
		Award
	} from 'lucide-svelte';
	import { Button } from '$lib/components/ui/button';

	// Mock data
	const stats = [
		{
			title: 'นักเรียนทั้งหมด',
			value: '1,234',
			change: '+12%',
			trend: 'up',
			icon: Users,
			color: 'bg-blue-500'
		},
		{
			title: 'ครูและบุคลากร',
			value: '87',
			change: '+3%',
			trend: 'up',
			icon: GraduationCap,
			color: 'bg-green-500'
		},
		{
			title: 'รายวิชา',
			value: '45',
			change: '+5',
			trend: 'up',
			icon: BookOpen,
			color: 'bg-purple-500'
		},
		{
			title: 'ห้องเรียน',
			value: '32',
			change: '0%',
			trend: 'neutral',
			icon: School,
			color: 'bg-orange-500'
		}
	];

	const recentActivities = [
		{
			title: 'นักเรียนใหม่ลงทะเบียน',
			description: 'สมชาย ใจดี ม.1/1',
			time: '5 นาทีที่แล้ว',
			type: 'student'
		},
		{
			title: 'อัพเดทข้อมูลครู',
			description: 'ครูสมหญิง อัพเดทข้อมูลวิชาคณิตศาสตร์',
			time: '30 นาทีที่แล้ว',
			type: 'teacher'
		},
		{
			title: 'เพิ่มรายวิชาใหม่',
			description: 'วิทยาศาสตร์ขั้นสูง',
			time: '1 ชั่วโมงที่แล้ว',
			type: 'subject'
		},
		{
			title: 'อนุมัติคำร้อง',
			description: 'คำร้องลา 3 รายการ',
			time: '2 ชั่วโมงที่แล้ว',
			type: 'approval'
		}
	];

	const upcomingEvents = [
		{
			title: 'ประชุมผู้ปกครอง',
			date: '20 ธ.ค. 2567',
			time: '13:00 น.',
			location: 'ห้องประชุมใหญ่'
		},
		{
			title: 'สอบกลางภาค',
			date: '22-24 ธ.ค. 2567',
			time: '08:00 น.',
			location: 'ห้องสอบ 1-10'
		},
		{
			title: 'วันกีฬาสี',
			date: '28 ธ.ค. 2567',
			time: '08:00 น.',
			location: 'สนามกีฬา'
		}
	];
</script>

<svelte:head>
	<title>แดชบอร์ด - SchoolOrbit</title>
</svelte:head>

<div class="space-y-6">
	<!-- Page Header -->
	<div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
		<div>
			<h1 class="text-3xl font-bold text-foreground">แดชบอร์ด</h1>
			<p class="text-muted-foreground mt-1">ภาพรวมระบบจัดการโรงเรียน</p>
		</div>
		<div class="flex gap-2">
			<Button variant="outline">
				<Calendar class="w-4 h-4 mr-2" />
				ปฏิทิน
			</Button>
			<Button>
				<Bell class="w-4 h-4 mr-2" />
				การแจ้งเตือน
			</Button>
		</div>
	</div>

	<!-- Stats Grid -->
	<div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
		{#each stats as stat}
			{@const Icon = stat.icon}
			<div
				class="bg-card border border-border rounded-lg p-6 hover:shadow-lg transition-shadow cursor-pointer"
			>
				<div class="flex items-start justify-between">
					<div class="flex-1">
						<p class="text-sm text-muted-foreground font-medium">{stat.title}</p>
						<p class="text-3xl font-bold text-foreground mt-2">{stat.value}</p>
						<div class="flex items-center gap-1 mt-2">
							<TrendingUp
								class="w-4 h-4 {stat.trend === 'up' ? 'text-green-500' : 'text-gray-500'}"
							/>
							<span
								class="text-sm font-medium {stat.trend === 'up'
									? 'text-green-500'
									: 'text-gray-500'}"
							>
								{stat.change}
							</span>
							<span class="text-sm text-muted-foreground">จากเดือนที่แล้ว</span>
						</div>
					</div>
					<div class="{stat.color} rounded-lg p-3">
						<Icon class="w-6 h-6 text-white" />
					</div>
				</div>
			</div>
		{/each}
	</div>

	<!-- Content Grid -->
	<div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
		<!-- Recent Activities -->
		<div class="lg:col-span-2 bg-card border border-border rounded-lg">
			<div class="p-6 border-b border-border">
				<h2 class="text-xl font-bold text-foreground">กิจกรรมล่าสุด</h2>
			</div>
			<div class="p-6">
				<div class="space-y-4">
					{#each recentActivities as activity}
						<div class="flex items-start gap-4 p-4 rounded-lg hover:bg-accent transition-colors">
							<div class="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center">
								{#if activity.type === 'student'}
									<Users class="w-5 h-5 text-primary" />
								{:else if activity.type === 'teacher'}
									<GraduationCap class="w-5 h-5 text-primary" />
								{:else if activity.type === 'subject'}
									<BookOpen class="w-5 h-5 text-primary" />
								{:else}
									<Award class="w-5 h-5 text-primary" />
								{/if}
							</div>
							<div class="flex-1">
								<p class="font-medium text-foreground">{activity.title}</p>
								<p class="text-sm text-muted-foreground">{activity.description}</p>
								<p class="text-xs text-muted-foreground mt-1">{activity.time}</p>
							</div>
						</div>
					{/each}
				</div>
				<div class="mt-4 text-center">
					<Button variant="outline" class="w-full">ดูทั้งหมด</Button>
				</div>
			</div>
		</div>

		<!-- Upcoming Events -->
		<div class="bg-card border border-border rounded-lg">
			<div class="p-6 border-b border-border">
				<h2 class="text-xl font-bold text-foreground">กิจกรรมที่จะมาถึง</h2>
			</div>
			<div class="p-6">
				<div class="space-y-4">
					{#each upcomingEvents as event}
						<div class="p-4 rounded-lg border border-border hover:border-primary transition-colors">
							<div class="flex items-start gap-3">
								<div
									class="w-12 h-12 rounded-lg bg-primary/10 flex flex-col items-center justify-center"
								>
									<span class="text-xs text-primary font-medium">{event.date.split(' ')[0]}</span>
									<span class="text-xs text-primary font-bold">{event.date.split(' ')[1]}</span>
								</div>
								<div class="flex-1">
									<p class="font-medium text-foreground">{event.title}</p>
									<div class="space-y-1 mt-2">
										<div class="flex items-center gap-2">
											<Calendar class="w-3 h-3 text-muted-foreground" />
											<p class="text-xs text-muted-foreground">{event.time}</p>
										</div>
										<div class="flex items-center gap-2">
											<School class="w-3 h-3 text-muted-foreground" />
											<p class="text-xs text-muted-foreground">{event.location}</p>
										</div>
									</div>
								</div>
							</div>
						</div>
					{/each}
				</div>
				<div class="mt-4 text-center">
					<Button variant="outline" class="w-full">ดูปฏิทินทั้งหมด</Button>
				</div>
			</div>
		</div>
	</div>

	<!-- Quick Actions -->
	<div class="bg-card border border-border rounded-lg p-6">
		<h2 class="text-xl font-bold text-foreground mb-4">การดำเนินการด่วน</h2>
		<div class="grid grid-cols-2 sm:grid-cols-4 gap-4">
			<Button variant="outline" class="h-auto py-6 flex-col gap-2">
				<Users class="w-6 h-6" />
				<span>เพิ่มนักเรียน</span>
			</Button>
			<Button variant="outline" class="h-auto py-6 flex-col gap-2">
				<GraduationCap class="w-6 h-6" />
				<span>เพิ่มครู</span>
			</Button>
			<Button variant="outline" class="h-auto py-6 flex-col gap-2">
				<BookOpen class="w-6 h-6" />
				<span>สร้างรายวิชา</span>
			</Button>
			<Button variant="outline" class="h-auto py-6 flex-col gap-2">
				<School class="w-6 h-6" />
				<span>จัดห้องเรียน</span>
			</Button>
		</div>
	</div>
</div>
