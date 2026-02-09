<script lang="ts">
	import { Tabs, TabsContent, TabsList, TabsTrigger } from '$lib/components/ui/tabs';
	import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import { Users, BookOpen, ArrowRight } from 'lucide-svelte';
	import InstructorConstraints from './InstructorConstraints.svelte';
	import SubjectConstraints from './SubjectConstraints.svelte';
</script>

<div class="space-y-6">
	<div class="flex items-center justify-between">
		<div>
			<h1 class="text-3xl font-bold tracking-tight">ตั้งค่าเงื่อนไขการจัดตาราง</h1>
			<p class="text-muted-foreground">
				กำหนดข้อจำกัดและความต้องการของครูและรายวิชา เพื่อให้ระบบจัดตารางได้เหมาะสมที่สุด
			</p>
		</div>
		<a href="/staff/academic/timetable/scheduling/auto-schedule">
			<Button>
				ถัดไป: จัดตารางอัตโนมัติ
				<ArrowRight class="ml-2 h-4 w-4" />
			</Button>
		</a>
	</div>

	<Tabs value="instructors" class="w-full">
		<TabsList class="grid w-full grid-cols-2 lg:w-[400px]">
			<TabsTrigger value="instructors" class="flex gap-2">
				<Users class="h-4 w-4" />
				ครูผู้สอน (Instructors)
			</TabsTrigger>
			<TabsTrigger value="subjects" class="flex gap-2">
				<BookOpen class="h-4 w-4" />
				รายวิชา (Subjects)
			</TabsTrigger>
		</TabsList>

		<TabsContent value="instructors" class="mt-6">
			<Card>
				<CardHeader>
					<CardTitle>เงื่อนไขครูผู้สอน</CardTitle>
				</CardHeader>
				<CardContent>
					<InstructorConstraints />
				</CardContent>
			</Card>
		</TabsContent>

		<TabsContent value="subjects" class="mt-6">
			<Card>
				<CardHeader>
					<CardTitle>เงื่อนไขรายวิชา</CardTitle>
				</CardHeader>
				<CardContent>
					<SubjectConstraints />
				</CardContent>
			</Card>
		</TabsContent>
	</Tabs>
</div>
