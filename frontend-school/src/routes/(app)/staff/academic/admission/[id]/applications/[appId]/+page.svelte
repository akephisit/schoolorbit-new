<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import {
		getApplication,
		verifyApplication,
		rejectApplication,
		updateApplicationByStaff,
		unverifyApplication,
		DOC_TYPE_LABELS,
		type AdmissionApplication,
		type ApplicationDocument,
		applicationStatusLabel
	} from '$lib/api/admission';
	import { Button } from '$lib/components/ui/button';
	import { Badge } from '$lib/components/ui/badge';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Card from '$lib/components/ui/card';
	import { Separator } from '$lib/components/ui/separator';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Textarea } from '$lib/components/ui/textarea';
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
		LoaderCircle,
		ImageIcon,
		ZoomIn,
		ZoomOut,
		Pencil,
		RotateCcw,
		Save
	} from 'lucide-svelte';

	let { data } = $props();

	let roundId = $derived($page.params.id);
	let appId = $derived($page.params.appId);

	let application: AdmissionApplication | null = $state(null);
	let documents: ApplicationDocument[] = $state([]);
	let loading = $state(true);

	// Reject
	let showRejectDialog = $state(false);
	let rejectReason = $state('');
	let rejecting = $state(false);

	// Unverify
	let showUnverifyDialog = $state(false);
	let unverifying = $state(false);

	// Edit mode
	let editMode = $state(false);
	let saving = $state(false);
	let editData = $state<Partial<AdmissionApplication>>({});

	// Lightbox
	let lightboxDoc = $state<ApplicationDocument | null>(null);
	let lbZoom = $state(1);
	let lbPan = $state({ x: 0, y: 0 });
	let lbDragging = $state(false);
	let lbDragStart = { mx: 0, my: 0, px: 0, py: 0 };

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
			const res = await getApplication(appId);
			application = res.application;
			documents = res.documents;
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
			toast.success('ยืนยันข้อมูลแล้ว');
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

	async function handleUnverifyConfirm() {
		if (!application) return;
		unverifying = true;
		try {
			await unverifyApplication(application.id);
			toast.success('ยกเลิกการอนุมัติแล้ว');
			showUnverifyDialog = false;
			await loadApp();
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'ยกเลิกการอนุมัติไม่สำเร็จ');
		} finally {
			unverifying = false;
		}
	}

	function startEdit() {
		if (!application) return;
		editData = { ...application };
		editMode = true;
	}

	function cancelEdit() {
		editMode = false;
		editData = {};
	}

	async function handleSave() {
		if (!application) return;
		saving = true;
		try {
			await updateApplicationByStaff(application.id, editData);
			toast.success('บันทึกข้อมูลแล้ว');
			editMode = false;
			editData = {};
			await loadApp();
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'บันทึกไม่สำเร็จ');
		} finally {
			saving = false;
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
		const months = ['มกราคม','กุมภาพันธ์','มีนาคม','เมษายน','พฤษภาคม','มิถุนายน','กรกฎาคม','สิงหาคม','กันยายน','ตุลาคม','พฤศจิกายน','ธันวาคม'];
		return `${date.getDate()} ${months[date.getMonth()]} ${date.getFullYear() + 543}`;
	}

	function formatCurrency(n?: number) {
		if (!n) return '-';
		return n.toLocaleString('th-TH') + ' บาท/เดือน';
	}

	function formatHomeAddress(app: AdmissionApplication): string {
		return [
			app.homeHouseNo,
			app.homeMoo ? `หมู่ ${app.homeMoo}` : '',
			app.homeSoi ? `ซ.${app.homeSoi}` : '',
			app.homeRoad ? `ถ.${app.homeRoad}` : '',
			app.addressLine,
			app.subDistrict ? `ต.${app.subDistrict}` : '',
			app.district ? `อ.${app.district}` : '',
			app.province ? `จ.${app.province}` : '',
			app.postalCode
		].filter(Boolean).join(' ').trim() || '-';
	}

	function formatCurrentAddress(app: AdmissionApplication): string {
		return [
			app.currentHouseNo,
			app.currentMoo ? `หมู่ ${app.currentMoo}` : '',
			app.currentSoi ? `ซ.${app.currentSoi}` : '',
			app.currentRoad ? `ถ.${app.currentRoad}` : '',
			app.currentSubDistrict ? `ต.${app.currentSubDistrict}` : '',
			app.currentDistrict ? `อ.${app.currentDistrict}` : '',
			app.currentProvince ? `จ.${app.currentProvince}` : '',
			app.currentPostalCode
		].filter(Boolean).join(' ').trim() || '-';
	}

	// Lightbox controls
	function openLightbox(doc: ApplicationDocument) {
		lightboxDoc = doc;
		lbZoom = 1;
		lbPan = { x: 0, y: 0 };
	}

	function closeLightbox() {
		lightboxDoc = null;
	}

	function onLbWheel(e: WheelEvent) {
		e.preventDefault();
		const delta = e.deltaY > 0 ? -0.2 : 0.2;
		lbZoom = Math.max(0.5, Math.min(8, lbZoom + delta));
	}

	function onLbMouseDown(e: MouseEvent) {
		e.preventDefault();
		lbDragging = true;
		lbDragStart = { mx: e.clientX, my: e.clientY, px: lbPan.x, py: lbPan.y };
		window.addEventListener('mousemove', onLbMouseMove);
		window.addEventListener('mouseup', onLbMouseUp, { once: true });
	}

	function onLbMouseMove(e: MouseEvent) {
		if (!lbDragging) return;
		lbPan = { x: lbDragStart.px + e.clientX - lbDragStart.mx, y: lbDragStart.py + e.clientY - lbDragStart.my };
	}

	function onLbMouseUp() {
		lbDragging = false;
		window.removeEventListener('mousemove', onLbMouseMove);
	}

	function onLbKeyDown(e: KeyboardEvent) {
		if (e.key === 'Escape') closeLightbox();
	}

	onMount(loadApp);
</script>

<svelte:head>
	<title>{data.title} - SchoolOrbit</title>
</svelte:head>

<svelte:window onkeydown={onLbKeyDown} />

<div class="space-y-6">
	<div class="flex items-center gap-3">
		<Button href="/staff/academic/admission/{roundId}/applications" variant="ghost" size="sm">
			<ArrowLeft class="w-4 h-4 mr-1" /> ย้อนกลับ
		</Button>
		<h1 class="text-2xl font-bold flex items-center gap-2">
			<FileText class="w-6 h-6" /> รายละเอียดใบสมัคร
		</h1>
		{#if application?.status === 'submitted'}
			{#if editMode}
				<div class="ml-auto flex gap-2">
					<Button variant="outline" size="sm" onclick={cancelEdit} disabled={saving}>
						<X class="w-4 h-4 mr-1" /> ยกเลิก
					</Button>
					<Button size="sm" onclick={handleSave} disabled={saving}>
						{#if saving}<LoaderCircle class="w-4 h-4 mr-1 animate-spin" />{:else}<Save class="w-4 h-4 mr-1" />{/if}
						{saving ? 'กำลังบันทึก...' : 'บันทึก'}
					</Button>
				</div>
			{:else}
				<Button variant="outline" size="sm" class="ml-auto" onclick={startEdit}>
					<Pencil class="w-4 h-4 mr-1" /> แก้ไขใบสมัคร
				</Button>
			{/if}
		{/if}
	</div>

	{#if loading}
		<Card.Root>
			<Card.Content class="flex justify-center py-20">
				<LoaderCircle class="w-8 h-8 animate-spin text-primary" />
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
					<Card.Content class="pt-6">
						<div class="grid grid-cols-2 gap-x-6 gap-y-4">
							<div>
								<p class="text-xs text-muted-foreground">เลขที่ใบสมัคร</p>
								<p class="font-mono font-medium">{application.applicationNumber ?? '-'}</p>
							</div>
							<div>
								<p class="text-xs text-muted-foreground">เลขประจำตัวประชาชน</p>
								<p class="font-mono font-medium">{application.nationalId}</p>
							</div>
							<div>
								<p class="text-xs text-muted-foreground mb-1">คำนำหน้า</p>
								{#if editMode}
									<Input bind:value={editData.title} placeholder="นาย / นาง / นางสาว" class="h-8 text-sm" />
								{:else}
									<p class="font-medium">{application.title || '-'}</p>
								{/if}
							</div>
							<div>
								<p class="text-xs text-muted-foreground mb-1">เพศ</p>
								{#if editMode}
									<select bind:value={editData.gender} class="w-full h-8 rounded-md border border-input bg-background px-3 text-sm">
										<option value="">-- เลือก --</option>
										<option value="Male">ชาย</option>
										<option value="Female">หญิง</option>
									</select>
								{:else}
									<p class="font-medium">{application.gender === 'Male' ? 'ชาย' : application.gender === 'Female' ? 'หญิง' : '-'}</p>
								{/if}
							</div>
							<div>
								<p class="text-xs text-muted-foreground mb-1">ชื่อ</p>
								{#if editMode}
									<Input bind:value={editData.firstName} class="h-8 text-sm" />
								{:else}
									<p class="font-medium">{application.firstName}</p>
								{/if}
							</div>
							<div>
								<p class="text-xs text-muted-foreground mb-1">นามสกุล</p>
								{#if editMode}
									<Input bind:value={editData.lastName} class="h-8 text-sm" />
								{:else}
									<p class="font-medium">{application.lastName}</p>
								{/if}
							</div>
							<div>
								<p class="text-xs text-muted-foreground mb-1">วันเกิด</p>
								{#if editMode}
									<Input type="date" bind:value={editData.dateOfBirth} class="h-8 text-sm" />
								{:else}
									<p class="font-medium">{application.dateOfBirth ? formatThaiDateFull(application.dateOfBirth) : '-'}</p>
								{/if}
							</div>
							<div>
								<p class="text-xs text-muted-foreground mb-1">เบอร์โทรศัพท์</p>
								{#if editMode}
									<Input bind:value={editData.phone} class="h-8 text-sm" />
								{:else}
									<p class="font-medium">{application.phone || '-'}</p>
								{/if}
							</div>
							<div>
								<p class="text-xs text-muted-foreground mb-1">อีเมล</p>
								{#if editMode}
									<Input bind:value={editData.email} class="h-8 text-sm" />
								{:else}
									<p class="font-medium">{application.email || '-'}</p>
								{/if}
							</div>
							<div>
								<p class="text-xs text-muted-foreground mb-1">ศาสนา</p>
								{#if editMode}
									<Input bind:value={editData.religion} class="h-8 text-sm" />
								{:else}
									<p class="font-medium">{application.religion || '-'}</p>
								{/if}
							</div>
							<div>
								<p class="text-xs text-muted-foreground mb-1">เชื้อชาติ</p>
								{#if editMode}
									<Input bind:value={editData.ethnicity} class="h-8 text-sm" />
								{:else}
									<p class="font-medium">{application.ethnicity || '-'}</p>
								{/if}
							</div>
							<div>
								<p class="text-xs text-muted-foreground mb-1">สัญชาติ</p>
								{#if editMode}
									<Input bind:value={editData.nationality} class="h-8 text-sm" />
								{:else}
									<p class="font-medium">{application.nationality || '-'}</p>
								{/if}
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
					<Card.Content class="pt-6 space-y-5">
						<div>
							<p class="text-xs font-semibold uppercase tracking-wide text-muted-foreground mb-2">ที่อยู่ตามทะเบียนบ้าน</p>
							{#if editMode}
								<div class="grid grid-cols-2 gap-3">
									<div><Label class="text-xs">บ้านเลขที่</Label><Input bind:value={editData.homeHouseNo} class="h-8 text-sm mt-0.5" /></div>
									<div><Label class="text-xs">หมู่</Label><Input bind:value={editData.homeMoo} class="h-8 text-sm mt-0.5" /></div>
									<div><Label class="text-xs">ซอย</Label><Input bind:value={editData.homeSoi} class="h-8 text-sm mt-0.5" /></div>
									<div><Label class="text-xs">ถนน</Label><Input bind:value={editData.homeRoad} class="h-8 text-sm mt-0.5" /></div>
									<div><Label class="text-xs">ตำบล/แขวง</Label><Input bind:value={editData.subDistrict} class="h-8 text-sm mt-0.5" /></div>
									<div><Label class="text-xs">อำเภอ/เขต</Label><Input bind:value={editData.district} class="h-8 text-sm mt-0.5" /></div>
									<div><Label class="text-xs">จังหวัด</Label><Input bind:value={editData.province} class="h-8 text-sm mt-0.5" /></div>
									<div><Label class="text-xs">รหัสไปรษณีย์</Label><Input bind:value={editData.postalCode} class="h-8 text-sm mt-0.5" /></div>
									<div class="col-span-2"><Label class="text-xs">โทรศัพท์บ้าน</Label><Input bind:value={editData.homePhone} class="h-8 text-sm mt-0.5" /></div>
								</div>
							{:else}
								<p class="font-medium leading-relaxed">{formatHomeAddress(application)}</p>
								{#if application.homePhone}
									<p class="text-sm text-muted-foreground mt-1">โทร. {application.homePhone}</p>
								{/if}
							{/if}
						</div>
						<Separator />
						<div>
							<p class="text-xs font-semibold uppercase tracking-wide text-muted-foreground mb-2">ที่อยู่ปัจจุบัน</p>
							{#if editMode}
								<div class="grid grid-cols-2 gap-3">
									<div><Label class="text-xs">บ้านเลขที่</Label><Input bind:value={editData.currentHouseNo} class="h-8 text-sm mt-0.5" /></div>
									<div><Label class="text-xs">หมู่</Label><Input bind:value={editData.currentMoo} class="h-8 text-sm mt-0.5" /></div>
									<div><Label class="text-xs">ซอย</Label><Input bind:value={editData.currentSoi} class="h-8 text-sm mt-0.5" /></div>
									<div><Label class="text-xs">ถนน</Label><Input bind:value={editData.currentRoad} class="h-8 text-sm mt-0.5" /></div>
									<div><Label class="text-xs">ตำบล/แขวง</Label><Input bind:value={editData.currentSubDistrict} class="h-8 text-sm mt-0.5" /></div>
									<div><Label class="text-xs">อำเภอ/เขต</Label><Input bind:value={editData.currentDistrict} class="h-8 text-sm mt-0.5" /></div>
									<div><Label class="text-xs">จังหวัด</Label><Input bind:value={editData.currentProvince} class="h-8 text-sm mt-0.5" /></div>
									<div><Label class="text-xs">รหัสไปรษณีย์</Label><Input bind:value={editData.currentPostalCode} class="h-8 text-sm mt-0.5" /></div>
									<div class="col-span-2"><Label class="text-xs">โทรศัพท์</Label><Input bind:value={editData.currentPhone} class="h-8 text-sm mt-0.5" /></div>
								</div>
							{:else}
								{#if application.currentHouseNo || application.currentSubDistrict || application.currentProvince}
									<p class="font-medium leading-relaxed">{formatCurrentAddress(application)}</p>
									{#if application.currentPhone}
										<p class="text-sm text-muted-foreground mt-1">โทร. {application.currentPhone}</p>
									{/if}
								{:else}
									<p class="text-sm text-muted-foreground">ไม่มีข้อมูล</p>
								{/if}
							{/if}
						</div>
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
					<Card.Content class="pt-6">
						<div class="grid grid-cols-2 gap-x-6 gap-y-4">
							<div class="col-span-2">
								<p class="text-xs text-muted-foreground mb-1">ชื่อโรงเรียน</p>
								{#if editMode}
									<Input bind:value={editData.previousSchool} class="h-8 text-sm" />
								{:else}
									<p class="font-medium">{application.previousSchool || '-'}</p>
								{/if}
							</div>
							<div>
								<p class="text-xs text-muted-foreground mb-1">จังหวัด</p>
								{#if editMode}
									<Input bind:value={editData.previousSchoolProvince} class="h-8 text-sm" />
								{:else}
									<p class="font-medium">{application.previousSchoolProvince || '-'}</p>
								{/if}
							</div>
							<div>
								<p class="text-xs text-muted-foreground mb-1">ระดับชั้น</p>
								{#if editMode}
									<Input bind:value={editData.previousGrade} class="h-8 text-sm" />
								{:else}
									<p class="font-medium">{application.previousGrade || '-'}</p>
								{/if}
							</div>
							<div>
								<p class="text-xs text-muted-foreground mb-1">ปีการศึกษา</p>
								{#if editMode}
									<Input bind:value={editData.previousStudyYear} class="h-8 text-sm" />
								{:else}
									<p class="font-medium">{application.previousStudyYear || '-'}</p>
								{/if}
							</div>
							<div>
								<p class="text-xs text-muted-foreground mb-1">เกรดเฉลี่ยสะสม (GPA)</p>
								{#if editMode}
									<Input type="number" step="0.01" min="0" max="4" bind:value={editData.previousGpa} class="h-8 text-sm" />
								{:else}
									<p class="font-medium">{application.previousGpa ? application.previousGpa.toFixed(2) : '-'}</p>
								{/if}
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
						{#if application.parentStatus && application.parentStatus.length > 0}
							<div>
								<p class="text-xs text-muted-foreground mb-1">สถานภาพครอบครัว</p>
								<p class="font-medium">
									{application.parentStatus.join(', ')}
									{#if application.parentStatusOther} — {application.parentStatusOther}{/if}
								</p>
							</div>
							<Separator />
						{/if}

						<!-- บิดา -->
						<div>
							<p class="text-xs font-semibold uppercase tracking-wide text-muted-foreground mb-3">บิดา</p>
							<div class="grid grid-cols-2 gap-x-6 gap-y-3">
								<div>
									<p class="text-xs text-muted-foreground mb-1">ชื่อ-นามสกุล</p>
									{#if editMode}
										<Input bind:value={editData.fatherName} class="h-8 text-sm" />
									{:else}
										<p class="font-medium">{application.fatherName || '-'}</p>
									{/if}
								</div>
								<div>
									<p class="text-xs text-muted-foreground mb-1">เบอร์โทรศัพท์</p>
									{#if editMode}
										<Input bind:value={editData.fatherPhone} class="h-8 text-sm" />
									{:else}
										<p class="font-medium">{application.fatherPhone || '-'}</p>
									{/if}
								</div>
								<div>
									<p class="text-xs text-muted-foreground mb-1">เลขประชาชน</p>
									{#if editMode}
										<Input bind:value={editData.fatherNationalId} class="h-8 text-sm font-mono" />
									{:else}
										<p class="font-mono font-medium">{application.fatherNationalId || '-'}</p>
									{/if}
								</div>
								<div>
									<p class="text-xs text-muted-foreground mb-1">อาชีพ</p>
									{#if editMode}
										<Input bind:value={editData.fatherOccupation} class="h-8 text-sm" />
									{:else}
										<p class="font-medium">{application.fatherOccupation || '-'}</p>
									{/if}
								</div>
								<div>
									<p class="text-xs text-muted-foreground mb-1">รายได้ (บาท/เดือน)</p>
									{#if editMode}
										<Input type="number" bind:value={editData.fatherIncome} class="h-8 text-sm" />
									{:else}
										<p class="font-medium">{formatCurrency(application.fatherIncome)}</p>
									{/if}
								</div>
							</div>
						</div>

						<Separator />

						<!-- มารดา -->
						<div>
							<p class="text-xs font-semibold uppercase tracking-wide text-muted-foreground mb-3">มารดา</p>
							<div class="grid grid-cols-2 gap-x-6 gap-y-3">
								<div>
									<p class="text-xs text-muted-foreground mb-1">ชื่อ-นามสกุล</p>
									{#if editMode}
										<Input bind:value={editData.motherName} class="h-8 text-sm" />
									{:else}
										<p class="font-medium">{application.motherName || '-'}</p>
									{/if}
								</div>
								<div>
									<p class="text-xs text-muted-foreground mb-1">เบอร์โทรศัพท์</p>
									{#if editMode}
										<Input bind:value={editData.motherPhone} class="h-8 text-sm" />
									{:else}
										<p class="font-medium">{application.motherPhone || '-'}</p>
									{/if}
								</div>
								<div>
									<p class="text-xs text-muted-foreground mb-1">เลขประชาชน</p>
									{#if editMode}
										<Input bind:value={editData.motherNationalId} class="h-8 text-sm font-mono" />
									{:else}
										<p class="font-mono font-medium">{application.motherNationalId || '-'}</p>
									{/if}
								</div>
								<div>
									<p class="text-xs text-muted-foreground mb-1">อาชีพ</p>
									{#if editMode}
										<Input bind:value={editData.motherOccupation} class="h-8 text-sm" />
									{:else}
										<p class="font-medium">{application.motherOccupation || '-'}</p>
									{/if}
								</div>
								<div>
									<p class="text-xs text-muted-foreground mb-1">รายได้ (บาท/เดือน)</p>
									{#if editMode}
										<Input type="number" bind:value={editData.motherIncome} class="h-8 text-sm" />
									{:else}
										<p class="font-medium">{formatCurrency(application.motherIncome)}</p>
									{/if}
								</div>
							</div>
						</div>

						<Separator />

						<!-- ผู้ปกครอง -->
						<div>
							<p class="text-xs font-semibold uppercase tracking-wide text-muted-foreground mb-3">
								ผู้ปกครอง
								{#if application.guardianIs === 'father'}(บิดา){:else if application.guardianIs === 'mother'}(มารดา){:else if application.guardianIs === 'other'}(บุคคลอื่น){/if}
							</p>
							<div class="grid grid-cols-2 gap-x-6 gap-y-3">
								<div>
									<p class="text-xs text-muted-foreground mb-1">ชื่อ-นามสกุล</p>
									{#if editMode}
										<Input bind:value={editData.guardianName} class="h-8 text-sm" />
									{:else}
										<p class="font-medium">{application.guardianName || '-'}</p>
									{/if}
								</div>
								<div>
									<p class="text-xs text-muted-foreground mb-1">เบอร์โทรศัพท์</p>
									{#if editMode}
										<Input bind:value={editData.guardianPhone} class="h-8 text-sm" />
									{:else}
										<p class="font-medium">{application.guardianPhone || '-'}</p>
									{/if}
								</div>
								<div>
									<p class="text-xs text-muted-foreground mb-1">ความสัมพันธ์</p>
									{#if editMode}
										<Input bind:value={editData.guardianRelation} class="h-8 text-sm" />
									{:else}
										<p class="font-medium">{application.guardianRelation || '-'}</p>
									{/if}
								</div>
								<div>
									<p class="text-xs text-muted-foreground mb-1">เลขประชาชน</p>
									{#if editMode}
										<Input bind:value={editData.guardianNationalId} class="h-8 text-sm font-mono" />
									{:else}
										<p class="font-mono font-medium">{application.guardianNationalId || '-'}</p>
									{/if}
								</div>
								<div>
									<p class="text-xs text-muted-foreground mb-1">อาชีพ</p>
									{#if editMode}
										<Input bind:value={editData.guardianOccupation} class="h-8 text-sm" />
									{:else}
										<p class="font-medium">{application.guardianOccupation || '-'}</p>
									{/if}
								</div>
								<div>
									<p class="text-xs text-muted-foreground mb-1">รายได้ (บาท/เดือน)</p>
									{#if editMode}
										<Input type="number" bind:value={editData.guardianIncome} class="h-8 text-sm" />
									{:else}
										<p class="font-medium">{formatCurrency(application.guardianIncome)}</p>
									{/if}
								</div>
							</div>
						</div>
					</Card.Content>
				</Card.Root>

				<!-- เอกสารแนบ -->
				<Card.Root>
					<Card.Header>
						<Card.Title class="flex items-center gap-2">
							<ImageIcon class="w-5 h-5 text-muted-foreground" /> เอกสารแนบ
						</Card.Title>
					</Card.Header>
					<Separator />
					<Card.Content class="pt-6">
						{#if documents.length === 0}
							<p class="text-sm text-muted-foreground text-center py-6">ไม่มีเอกสารแนบ</p>
						{:else}
							<div class="grid grid-cols-2 sm:grid-cols-3 gap-4">
								{#each documents as doc}
									{@const label = DOC_TYPE_LABELS[doc.docType]?.label ?? doc.docType}
									<button
										type="button"
										class="group flex flex-col gap-2 text-left focus:outline-none"
										onclick={() => openLightbox(doc)}
									>
										<div class="aspect-[3/4] rounded-lg overflow-hidden border bg-muted relative group-hover:ring-2 group-hover:ring-primary transition-all">
											{#if doc.fileUrl}
												<img
													src={doc.fileUrl}
													alt={label}
													class="w-full h-full object-cover"
												/>
												<div class="absolute inset-0 bg-black/0 group-hover:bg-black/20 transition-colors flex items-center justify-center">
													<ZoomIn class="w-6 h-6 text-white opacity-0 group-hover:opacity-100 transition-opacity drop-shadow-lg" />
												</div>
											{:else}
												<div class="w-full h-full flex items-center justify-center">
													<FileText class="w-8 h-8 text-muted-foreground" />
												</div>
											{/if}
										</div>
										<p class="text-xs font-medium leading-tight line-clamp-2">{label}</p>
									</button>
								{/each}
							</div>
						{/if}
					</Card.Content>
				</Card.Root>
			</div>

			<!-- Sidebar (ขวา) -->
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
							<Badge variant={statusVariant[application.status] ?? 'outline'} class="text-sm px-3 py-1">
								{applicationStatusLabel[application.status] ?? application.status}
							</Badge>
						</div>

						<div>
							<p class="text-sm text-muted-foreground">วันที่สมัคร</p>
							<p class="font-medium text-sm">{formatDate(application.createdAt)}</p>
						</div>

						<div>
							<p class="text-sm text-muted-foreground">เอกสารแนบ</p>
							<p class="font-medium text-sm">{documents.length} ไฟล์</p>
						</div>

						{#if application.status === 'rejected' && application.rejectionReason}
							<div class="bg-destructive/10 text-destructive p-4 rounded-md text-sm border border-destructive/20">
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
									onclick={() => { showRejectDialog = true; }}
								>
									<X class="w-4 h-4 mr-1" /> ไม่อนุมัติ
								</Button>
								<Button class="w-full bg-green-600 hover:bg-green-700" onclick={handleVerify}>
									<Check class="w-4 h-4 mr-1" /> อนุมัติ
								</Button>
							</div>
						{/if}

						{#if application.status === 'verified'}
							<Separator />
							<Button
								variant="outline"
								class="w-full text-destructive border-destructive/30 hover:bg-destructive/10"
								onclick={() => { showUnverifyDialog = true; }}
							>
								<RotateCcw class="w-4 h-4 mr-1" /> ยกเลิกการอนุมัติ
							</Button>
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
						กรุณาระบุเหตุผลที่ปฏิเสธใบสมัครของ <strong>{application.firstName} {application.lastName}</strong>
					</Dialog.Description>
				</Dialog.Header>
				<div class="space-y-2 py-2">
					<Label for="reject-reason">เหตุผล <span class="text-destructive">*</span></Label>
					<Textarea id="reject-reason" bind:value={rejectReason} placeholder="เช่น เอกสารไม่ครบถ้วน..." rows={3} />
				</div>
				<Dialog.Footer>
					<Button variant="outline" onclick={() => (showRejectDialog = false)}>ยกเลิก</Button>
					<Button variant="destructive" onclick={handleRejectConfirm} disabled={rejecting || !rejectReason.trim()}>
						{#if rejecting}<LoaderCircle class="w-4 h-4 mr-2 animate-spin" />{/if}
						{rejecting ? 'กำลังดำเนินการ...' : 'ยืนยันการปฏิเสธ'}
					</Button>
				</Dialog.Footer>
			</Dialog.Content>
		</Dialog.Root>

		<!-- Unverify Dialog -->
		<Dialog.Root bind:open={showUnverifyDialog}>
			<Dialog.Content>
				<Dialog.Header>
					<Dialog.Title>ยกเลิกการอนุมัติ</Dialog.Title>
					<Dialog.Description>
						ยืนยันการยกเลิกการอนุมัติใบสมัครของ <strong>{application.firstName} {application.lastName}</strong>?
						ใบสมัครจะกลับสู่สถานะ "รอตรวจสอบ"
					</Dialog.Description>
				</Dialog.Header>
				<Dialog.Footer>
					<Button variant="outline" onclick={() => (showUnverifyDialog = false)}>ยกเลิก</Button>
					<Button variant="destructive" onclick={handleUnverifyConfirm} disabled={unverifying}>
						{#if unverifying}<LoaderCircle class="w-4 h-4 mr-2 animate-spin" />{/if}
						{unverifying ? 'กำลังดำเนินการ...' : 'ยืนยัน'}
					</Button>
				</Dialog.Footer>
			</Dialog.Content>
		</Dialog.Root>
	{:else}
		<Card.Root>
			<Card.Content class="flex flex-col items-center justify-center py-20 text-muted-foreground gap-3">
				<FileText class="w-10 h-10 opacity-30" />
				<p>ไม่พบข้อมูลใบสมัคร</p>
			</Card.Content>
		</Card.Root>
	{/if}
</div>

<!-- Lightbox -->
{#if lightboxDoc}
	<div
		class="fixed inset-0 z-50 bg-black/90 flex flex-col"
		role="dialog"
		aria-modal="true"
	>
		<!-- Header bar -->
		<div class="flex items-center justify-between px-4 py-3 bg-black/60 shrink-0">
			<p class="text-white text-sm font-medium">{DOC_TYPE_LABELS[lightboxDoc.docType]?.label ?? lightboxDoc.docType}</p>
			<div class="flex items-center gap-3">
				<button
					type="button"
					class="text-white/70 hover:text-white transition-colors"
					onclick={() => { lbZoom = Math.max(0.5, lbZoom - 0.5); }}
					title="ย่อ"
				>
					<ZoomOut class="w-5 h-5" />
				</button>
				<span class="text-white/60 text-xs w-10 text-center">{Math.round(lbZoom * 100)}%</span>
				<button
					type="button"
					class="text-white/70 hover:text-white transition-colors"
					onclick={() => { lbZoom = Math.min(8, lbZoom + 0.5); }}
					title="ขยาย"
				>
					<ZoomIn class="w-5 h-5" />
				</button>
				<button
					type="button"
					class="text-white/70 hover:text-white transition-colors ml-2"
					onclick={closeLightbox}
					title="ปิด (ESC)"
				>
					<X class="w-5 h-5" />
				</button>
			</div>
		</div>

		<!-- Image area -->
		<div
			class="flex-1 overflow-hidden flex items-center justify-center"
			style="cursor: {lbDragging ? 'grabbing' : lbZoom > 1 ? 'grab' : 'default'}"
			onwheel={onLbWheel}
			onmousedown={onLbMouseDown}
			role="presentation"
		>
			{#if lightboxDoc.fileUrl}
				<img
					src={lightboxDoc.fileUrl}
					alt={DOC_TYPE_LABELS[lightboxDoc.docType]?.label ?? lightboxDoc.docType}
					class="max-w-none pointer-events-none select-none"
					style="transform: translate({lbPan.x}px, {lbPan.y}px) scale({lbZoom}); transition: {lbDragging ? 'none' : 'transform 0.1s ease'}; max-height: 80vh; max-width: 90vw;"
					draggable="false"
				/>
			{/if}
		</div>

		<!-- Click backdrop to close -->
		<button
			type="button"
			class="absolute inset-0 -z-10 w-full h-full"
			onclick={closeLightbox}
			aria-label="ปิด"
		></button>
	</div>
{/if}
