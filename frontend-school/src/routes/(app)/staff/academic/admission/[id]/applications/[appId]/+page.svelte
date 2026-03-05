<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import {
		getApplication,
		verifyApplication,
		rejectApplication,
		type AdmissionApplication,
		applicationStatusLabel
	} from '$lib/api/admission';
	import { Button } from '$lib/components/ui/button';
	import { Badge } from '$lib/components/ui/badge';
	import * as Card from '$lib/components/ui/card';
	import { Separator } from '$lib/components/ui/separator';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Textarea } from '$lib/components/ui/textarea';
	import { Label } from '$lib/components/ui/label';
	import { toast } from 'svelte-sonner';
	import {
		ArrowLeft,
		Check,
		X,
		FileText,
		User,
		Users,
		MapPin,
		School,
		Loader2
	} from 'lucide-svelte';

	let { data } = $props();

	let roundId = $derived($page.params.id);
	let appId = $derived($page.params.appId);

	let application: AdmissionApplication | null = $state(null);
	let loading = $state(true);

	let showRejectDialog = $state(false);
	let rejectReason = $state('');
	let rejecting = $state(false);

	const statusVariant: Record<string, 'default' | 'secondary' | 'outline' | 'destructive'> = {
		submitted: 'secondary',
		verified: 'default',
		rejected: 'destructive',
		accepted: 'default',
		enrolled: 'default',
		withdrawn: 'outline'
	};

	async function loadApp() {
		if (!appId) return;
		loading = true;
		try {
			application = await getApplication(appId);
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'ไม่สามารถโหลดข้อมูลผู้สมัครได้');
		} finally {
			loading = false;
		}
	}

	async function handleVerify() {
		if (!application) return;
		try {
			await verifyApplication(application.id);
			toast.success(`ยืนยันข้อมูลแล้ว`);
			await loadApp();
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'ยืนยันไม่สำเร็จ');
		}
	}

	async function handleRejectConfirm() {
		if (!application || !rejectReason.trim()) return;
		rejecting = true;
		try {
			await rejectApplication(application.id, rejectReason);
			toast.success('ปฏิเสธใบสมัครแล้ว');
			showRejectDialog = false;
			rejectReason = '';
			await loadApp();
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'ปฏิเสธไม่สำเร็จ');
		} finally {
			rejecting = false;
		}
	}

	function formatDate(iso: string) {
		return new Date(iso).toLocaleDateString('th-TH', {
			year: 'numeric',
			month: 'long',
			day: 'numeric'
		});
	}

	function formatThaiDateFull(dateStr: string) {
		const date = new Date(dateStr);
		const weekdays = ['อาทิตย์', 'จันทร์', 'อังคาร', 'พุธ', 'พฤหัสบดี', 'ศุกร์', 'เสาร์'];
		const months = [
			'มกราคม',
			'กุมภาพันธ์',
			'มีนาคม',
			'เมษายน',
			'พฤษภาคม',
			'มิถุนายน',
			'กรกฎาคม',
			'สิงหาคม',
			'กันยายน',
			'ตุลาคม',
			'พฤศจิกายน',
			'ธันวาคม'
		];
		const day = date.getDate();
		const month = months[date.getMonth()];
		const year = date.getFullYear() + 543;
		const weekdayName = weekdays[date.getDay()];
		return `วัน${weekdayName} ที่ ${day} เดือน ${month} ปี พ.ศ. ${year}`;
	}

	onMount(loadApp);
</script>

<svelte:head>
	<title>{data.title} - SchoolOrbit</title>
</svelte:head>

<div class="space-y-6">
	<div class="flex items-center gap-3">
		<!-- Add leading slash internally here to redirect back properly -->
		<Button href="/staff/academic/admission/{roundId}/applications" variant="ghost" size="sm">
			<ArrowLeft class="w-4 h-4 mr-1" /> ย้อนกลับ
		</Button>
		<h1 class="text-2xl font-bold flex items-center gap-2">
			<FileText class="w-6 h-6" /> รายละเอียดใบสมัคร
		</h1>
	</div>

	{#if loading}
		<Card.Root>
			<Card.Content class="flex justify-center py-20">
				<Loader2 class="w-8 h-8 animate-spin text-primary" />
			</Card.Content>
		</Card.Root>
	{:else if application}
		<div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
			<!-- ข้อมูลหลัก (ซ้าย) -->
			<div class="lg:col-span-2 space-y-6">
				<!-- ข้อมูลส่วนตัว -->
				<Card.Root>
					<Card.Header>
						<Card.Title class="flex items-center gap-2">
							<User class="w-5 h-5 text-muted-foreground" /> ข้อมูลผู้สมัคร
						</Card.Title>
					</Card.Header>
					<Separator />
					<Card.Content class="pt-6 space-y-4">
						<div class="grid grid-cols-2 gap-4">
							<div>
								<p class="text-sm text-muted-foreground">เลขที่ใบสมัคร</p>
								<p class="font-mono font-medium">{application.applicationNumber ?? '-'}</p>
							</div>
							<div>
								<p class="text-sm text-muted-foreground">เลขประจำตัวประชาชน</p>
								<p class="font-mono font-medium">{application.nationalId}</p>
							</div>
							<div>
								<p class="text-sm text-muted-foreground">ชื่อ-นามสกุล</p>
								<p class="font-medium">
									{application.title ?? ''}{application.firstName}
									{application.lastName}
								</p>
							</div>
							<div>
								<p class="text-sm text-muted-foreground">เพศ</p>
								<p class="font-medium">
									{application.gender === 'Male'
										? 'ชาย'
										: application.gender === 'Female'
											? 'หญิง'
											: '-'}
								</p>
							</div>
							<div>
								<p class="text-sm text-muted-foreground">วันเกิด</p>
								<p class="font-medium">
									{application.dateOfBirth ? formatThaiDateFull(application.dateOfBirth) : '-'}
								</p>
							</div>
							<div>
								<p class="text-sm text-muted-foreground">เบอร์โทรศัพท์</p>
								<p class="font-medium">{application.phone || '-'}</p>
							</div>
							<div class="col-span-2">
								<p class="text-sm text-muted-foreground">อีเมล</p>
								<p class="font-medium">{application.email || '-'}</p>
							</div>
						</div>
					</Card.Content>
				</Card.Root>

				<!-- ที่อยู่ -->
				<Card.Root>
					<Card.Header>
						<Card.Title class="flex items-center gap-2">
							<MapPin class="w-5 h-5 text-muted-foreground" /> ข้อมูลที่อยู่
						</Card.Title>
					</Card.Header>
					<Separator />
					<Card.Content class="pt-6">
						<p class="font-medium leading-relaxed">
							{application.addressLine || ''}
							{#if application.subDistrict}ต.{application.subDistrict}{/if}
							{#if application.district}อ.{application.district}{/if}
							{#if application.province}จ.{application.province}{/if}
							{application.postalCode || ''}
						</p>
					</Card.Content>
				</Card.Root>

				<!-- โรงเรียนเดิม -->
				<Card.Root>
					<Card.Header>
						<Card.Title class="flex items-center gap-2">
							<School class="w-5 h-5 text-muted-foreground" /> ข้อมูลโรงเรียนเดิม
						</Card.Title>
					</Card.Header>
					<Separator />
					<Card.Content class="pt-6 space-y-4">
						<div class="grid grid-cols-3 gap-4">
							<div class="col-span-3 sm:col-span-1">
								<p class="text-sm text-muted-foreground">ชื่อโรงเรียน</p>
								<p class="font-medium">{application.previousSchool || '-'}</p>
							</div>
							<div class="col-span-1 sm:col-span-1">
								<p class="text-sm text-muted-foreground">ระดับชั้น</p>
								<p class="font-medium">{application.previousGrade || '-'}</p>
							</div>
							<div class="col-span-1 sm:col-span-1">
								<p class="text-sm text-muted-foreground">เกรดเฉลี่ยสะสม (GPA)</p>
								<p class="font-medium">
									{application.previousGpa ? application.previousGpa.toFixed(2) : '-'}
								</p>
							</div>
						</div>
					</Card.Content>
				</Card.Root>

				<!-- ครอบครัว -->
				<Card.Root>
					<Card.Header>
						<Card.Title class="flex items-center gap-2">
							<Users class="w-5 h-5 text-muted-foreground" /> ข้อมูลครอบครัว
						</Card.Title>
					</Card.Header>
					<Separator />
					<Card.Content class="pt-6 space-y-6">
						<!-- บิดา -->
						<div>
							<p class="text-xs font-semibold uppercase tracking-wide text-muted-foreground mb-2">
								บิดา
							</p>
							<div class="grid grid-cols-2 gap-4">
								<div>
									<p class="text-sm text-muted-foreground">ชื่อ-นามสกุล</p>
									<p class="font-medium">{application.fatherName || '-'}</p>
								</div>
								<div>
									<p class="text-sm text-muted-foreground">เบอร์โทรศัพท์</p>
									<p class="font-medium">{application.fatherPhone || '-'}</p>
								</div>
								<div>
									<p class="text-sm text-muted-foreground">เลขประชาชน</p>
									<p class="font-mono font-medium">{application.fatherNationalId || '-'}</p>
								</div>
								<div>
									<p class="text-sm text-muted-foreground">อาชีพ</p>
									<p class="font-medium">{application.fatherOccupation || '-'}</p>
								</div>
							</div>
						</div>

						<Separator />

						<!-- มารดา -->
						<div>
							<p class="text-xs font-semibold uppercase tracking-wide text-muted-foreground mb-2">
								มารดา
							</p>
							<div class="grid grid-cols-2 gap-4">
								<div>
									<p class="text-sm text-muted-foreground">ชื่อ-นามสกุล</p>
									<p class="font-medium">{application.motherName || '-'}</p>
								</div>
								<div>
									<p class="text-sm text-muted-foreground">เบอร์โทรศัพท์</p>
									<p class="font-medium">{application.motherPhone || '-'}</p>
								</div>
								<div>
									<p class="text-sm text-muted-foreground">เลขประชาชน</p>
									<p class="font-mono font-medium">{application.motherNationalId || '-'}</p>
								</div>
								<div>
									<p class="text-sm text-muted-foreground">อาชีพ</p>
									<p class="font-medium">{application.motherOccupation || '-'}</p>
								</div>
							</div>
						</div>

						<Separator />

						<!-- ผู้ปกครอง -->
						<div>
							<p class="text-xs font-semibold uppercase tracking-wide text-muted-foreground mb-2">
								ผู้ปกครอง (ในกรณีไม่ใช่บิดามารดา)
							</p>
							<div class="grid grid-cols-2 gap-4">
								<div>
									<p class="text-sm text-muted-foreground">ชื่อ-นามสกุล</p>
									<p class="font-medium">{application.guardianName || '-'}</p>
								</div>
								<div>
									<p class="text-sm text-muted-foreground">เบอร์โทรศัพท์</p>
									<p class="font-medium">{application.guardianPhone || '-'}</p>
								</div>
								<div>
									<p class="text-sm text-muted-foreground">ความสัมพันธ์</p>
									<p class="font-medium">{application.guardianRelation || '-'}</p>
								</div>
								<div>
									<p class="text-sm text-muted-foreground">เลขประชาชน</p>
									<p class="font-mono font-medium">{application.guardianNationalId || '-'}</p>
								</div>
							</div>
						</div>
					</Card.Content>
				</Card.Root>
			</div>

			<!-- Sidebar สถานะ (ขวา) -->
			<div class="space-y-6">
				<Card.Root>
					<Card.Header class="bg-muted/30">
						<Card.Title>สายการเรียน / รอบ</Card.Title>
					</Card.Header>
					<Card.Content class="pt-6 space-y-4">
						<div>
							<p class="text-sm text-muted-foreground">รอบรับสมัคร</p>
							<p class="font-medium">{application.roundName || '-'}</p>
						</div>
						<div>
							<p class="text-sm text-muted-foreground">สายการเรียน</p>
							<p class="font-medium">{application.trackName || '-'}</p>
						</div>
					</Card.Content>
				</Card.Root>

				<Card.Root>
					<Card.Header class="bg-muted/30">
						<Card.Title>สถานะใบสมัคร</Card.Title>
					</Card.Header>
					<Card.Content class="pt-6 space-y-6">
						<div>
							<Badge
								variant={statusVariant[application.status] ?? 'outline'}
								class="text-sm px-3 py-1"
							>
								{applicationStatusLabel[application.status] ?? application.status}
							</Badge>
						</div>

						<div class="space-y-3">
							<p class="text-sm text-muted-foreground">วันที่สมัคร</p>
							<p class="font-medium text-sm">{formatDate(application.createdAt)}</p>
						</div>

						{#if application.status === 'rejected' && application.rejectionReason}
							<div
								class="bg-destructive/10 text-destructive p-4 rounded-md text-sm border border-destructive/20"
							>
								<p class="font-semibold mb-1">เหตุผลที่ปฏิเสธ:</p>
								<p>{application.rejectionReason}</p>
							</div>
						{/if}

						{#if application.status === 'submitted'}
							<Separator />
							<div class="grid grid-cols-2 gap-2">
								<Button
									variant="outline"
									class="w-full text-destructive border-destructive/30 hover:bg-destructive/10"
									onclick={() => {
										showRejectDialog = true;
									}}
								>
									<X class="w-4 h-4 mr-1" /> ไม่อนุมัติ
								</Button>
								<Button class="w-full bg-green-600 hover:bg-green-700" onclick={handleVerify}>
									<Check class="w-4 h-4 mr-1" /> อนุมัติ
								</Button>
							</div>
						{/if}
					</Card.Content>
				</Card.Root>
			</div>
		</div>

		<!-- Reject Dialog -->
		<Dialog.Root bind:open={showRejectDialog}>
			<Dialog.Content>
				<Dialog.Header>
					<Dialog.Title>ปฏิเสธใบสมัคร</Dialog.Title>
					<Dialog.Description>
						กรุณาระบุเหตุผลที่ปฏิเสธใบสมัครของ <strong
							>{application.firstName} {application.lastName}</strong
						>
					</Dialog.Description>
				</Dialog.Header>
				<div class="space-y-2 py-2">
					<Label for="reject-reason">เหตุผล <span class="text-destructive">*</span></Label>
					<Textarea
						id="reject-reason"
						bind:value={rejectReason}
						placeholder="เช่น เอกสารไม่ครบถ้วน..."
						rows={3}
					/>
				</div>
				<Dialog.Footer>
					<Button variant="outline" onclick={() => (showRejectDialog = false)}>ยกเลิก</Button>
					<Button
						variant="destructive"
						onclick={handleRejectConfirm}
						disabled={rejecting || !rejectReason.trim()}
					>
						{#if rejecting}<Loader2 class="w-4 h-4 mr-2 animate-spin" />{/if}
						{rejecting ? 'กำลังดำเนินการ...' : 'ยืนยันการปฏิเสธ'}
					</Button>
				</Dialog.Footer>
			</Dialog.Content>
		</Dialog.Root>
	{:else}
		<Card.Root>
			<Card.Content
				class="flex flex-col items-center justify-center py-20 text-muted-foreground gap-3"
			>
				<FileText class="w-10 h-10 opacity-30" />
				<p>ไม่พบข้อมูลใบสมัคร</p>
			</Card.Content>
		</Card.Root>
	{/if}
</div>
