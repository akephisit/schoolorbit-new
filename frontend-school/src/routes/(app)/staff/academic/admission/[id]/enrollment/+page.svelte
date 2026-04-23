<script lang="ts">
	import { onMount } from 'svelte';
	import type { PageProps } from './$types';
	import { getRound, listEnrollmentPending, completeEnrollment } from '$lib/api/admission';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Badge } from '$lib/components/ui/badge';
	import { Textarea } from '$lib/components/ui/textarea';
	import * as Card from '$lib/components/ui/card';
	import * as Table from '$lib/components/ui/table';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Select from '$lib/components/ui/select';
	import { toast } from 'svelte-sonner';
	import {
		ArrowLeft,
		ClipboardCheck,
		Check,
		UserCheck,
		Loader2,
		Plus,
		Trash2,
		Copy
	} from 'lucide-svelte';

	interface ParentEntry {
		title: string;
		firstName: string;
		lastName: string;
		phone: string;
		relationship: string;
	}

	interface EnrollRow {
		id: string;
		applicationNumber?: string;
		nationalId: string;
		fullName: string;
		trackName?: string;
		roomName?: string;
		status: string;
		studentConfirmed?: boolean;
		preSubmitted: boolean;
		assignedStudentId?: string;
		formData?: {
			bloodType?: string;
			medicalConditions?: string;
			allergies?: string;
			father?: { title?: string; firstName?: string; lastName?: string; phone?: string };
			mother?: { title?: string; firstName?: string; lastName?: string; phone?: string };
			guardians?: ParentEntry[];
			// legacy fields
			emergencyContact?: string;
			emergencyPhone?: string;
			shirtSize?: string;
		};
	}

	let { data, params }: PageProps = $props();
	let id = $derived(params.id);

	let round: Awaited<ReturnType<typeof getRound>> | null = $state(null);
	let list: EnrollRow[] = $state([]);
	let loading = $state(true);

	let showEnrollDialog = $state(false);
	let enrollingApp = $state<EnrollRow | null>(null);
	let studentCode = $state('');
	let enrolling = $state(false);

	let enrollFormData = $state({
		bloodType: '',
		medicalConditions: '',
		allergies: '',
		father: { title: 'นาย', firstName: '', lastName: '', phone: '' },
		mother: { title: 'นาง', firstName: '', lastName: '', phone: '' },
		guardians: [] as ParentEntry[]
	});

	function resetDialog() {
		enrollingApp = null;
		studentCode = '';
		enrollFormData = {
			bloodType: '',
			medicalConditions: '',
			allergies: '',
			father: { title: 'นาย', firstName: '', lastName: '', phone: '' },
			mother: { title: 'นาง', firstName: '', lastName: '', phone: '' },
			guardians: []
		};
	}

	function openEnrollDialog(app: EnrollRow) {
		enrollingApp = app;
		studentCode = app.assignedStudentId ?? '';

		// Pre-fill from form data if available
		if (app.formData) {
			const fd = app.formData;
			enrollFormData.bloodType = fd.bloodType ?? '';
			enrollFormData.medicalConditions = fd.medicalConditions ?? '';
			enrollFormData.allergies = fd.allergies ?? '';
			if (fd.father) {
				enrollFormData.father = {
					title: fd.father.title ?? 'นาย',
					firstName: fd.father.firstName ?? '',
					lastName: fd.father.lastName ?? '',
					phone: fd.father.phone ?? ''
				};
			}
			if (fd.mother) {
				enrollFormData.mother = {
					title: fd.mother.title ?? 'นาง',
					firstName: fd.mother.firstName ?? '',
					lastName: fd.mother.lastName ?? '',
					phone: fd.mother.phone ?? ''
				};
			}
			if (fd.guardians && fd.guardians.length > 0) {
				enrollFormData.guardians = fd.guardians.map((g) => ({
					title: g.title ?? '',
					firstName: g.firstName ?? '',
					lastName: g.lastName ?? '',
					phone: g.phone ?? '',
					relationship: g.relationship ?? ''
				}));
			}
		}

		showEnrollDialog = true;
	}

	function addGuardian() {
		enrollFormData.guardians = [
			...enrollFormData.guardians,
			{ title: '', firstName: '', lastName: '', phone: '', relationship: '' }
		];
	}

	function removeGuardian(index: number) {
		enrollFormData.guardians = enrollFormData.guardians.filter((_, i) => i !== index);
	}

	function copyFromFather(index: number) {
		const g = enrollFormData.guardians[index];
		g.title = enrollFormData.father.title;
		g.firstName = enrollFormData.father.firstName;
		g.lastName = enrollFormData.father.lastName;
		g.phone = enrollFormData.father.phone;
		g.relationship = 'บิดา';
		enrollFormData.guardians = [...enrollFormData.guardians];
	}

	function copyFromMother(index: number) {
		const g = enrollFormData.guardians[index];
		g.title = enrollFormData.mother.title;
		g.firstName = enrollFormData.mother.firstName;
		g.lastName = enrollFormData.mother.lastName;
		g.phone = enrollFormData.mother.phone;
		g.relationship = 'มารดา';
		enrollFormData.guardians = [...enrollFormData.guardians];
	}

	let needsForm = $derived(enrollingApp !== null && enrollingApp.preSubmitted === false);
	let formValid = $derived(() => {
		if (!needsForm) return true;
		return (
			enrollFormData.guardians.length > 0 &&
			enrollFormData.guardians.every((g) => g.firstName.trim() !== '' && g.phone.trim() !== '')
		);
	});

	async function load() {
		if (!id) return;
		loading = true;
		try {
			const [r, l] = await Promise.all([getRound(id), listEnrollmentPending(id)]);
			round = r;
			list = (l as EnrollRow[]) ?? [];
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'โหลดไม่สำเร็จ');
		} finally {
			loading = false;
		}
	}

	async function handleEnroll() {
		if (!enrollingApp) return;
		enrolling = true;
		try {
			const fd = needsForm ? enrollFormData : undefined;

			const res = (await completeEnrollment(
				enrollingApp.id,
				studentCode || undefined,
				fd as Record<string, unknown> | undefined
			)) as { username?: string };
			toast.success(`มอบตัวสำเร็จ! Username: ${res?.username}`);
			showEnrollDialog = false;
			resetDialog();
			await load();
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'มอบตัวไม่สำเร็จ');
		} finally {
			enrolling = false;
		}
	}

	onMount(load);
</script>

<svelte:head>
	<title>{data.title} - SchoolOrbit</title>
</svelte:head>

<div class="space-y-5">
	<div class="flex items-center gap-3">
		<Button href="/staff/academic/admission/{id}" variant="ghost" size="sm">
			<ArrowLeft class="w-4 h-4 mr-1" /> ย้อนกลับ
		</Button>
		<h1 class="text-2xl font-bold flex items-center gap-2">
			<ClipboardCheck class="w-6 h-6" /> รับมอบตัว
		</h1>
	</div>

	{#if round}
		<p class="text-sm text-muted-foreground">{round.name}</p>
	{/if}

	<!-- Stats -->
	<div class="grid grid-cols-2 md:grid-cols-4 gap-3">
		<Card.Root>
			<Card.Content class="pt-5 pb-5 text-center">
				<p class="text-3xl font-bold">{list.length}</p>
				<p class="text-xs text-muted-foreground mt-1">ได้รับคัดเลือกทั้งหมด</p>
			</Card.Content>
		</Card.Root>
		<Card.Root class="border-green-200 bg-green-50 dark:bg-green-950/20">
			<Card.Content class="pt-5 pb-5 text-center">
				<p class="text-3xl font-bold text-green-700">
					{list.filter((a) => a.studentConfirmed).length}
				</p>
				<p class="text-xs text-green-600 mt-1">ยืนยันแล้ว</p>
			</Card.Content>
		</Card.Root>
		<Card.Root class="border-blue-200 bg-blue-50 dark:bg-blue-950/20">
			<Card.Content class="pt-5 pb-5 text-center">
				<p class="text-3xl font-bold text-blue-700">{list.filter((a) => a.preSubmitted).length}</p>
				<p class="text-xs text-blue-600 mt-1">กรอกฟอร์มล่วงหน้า</p>
			</Card.Content>
		</Card.Root>
		<Card.Root class="border-purple-200 bg-purple-50 dark:bg-purple-950/20">
			<Card.Content class="pt-5 pb-5 text-center">
				<p class="text-3xl font-bold text-purple-700">
					{list.filter((a) => a.status === 'enrolled').length}
				</p>
				<p class="text-xs text-purple-600 mt-1">มอบตัวแล้ว</p>
			</Card.Content>
		</Card.Root>
	</div>

	{#if loading}
		<Card.Root>
			<Card.Content class="flex justify-center py-16">
				<Loader2 class="w-8 h-8 animate-spin text-primary" />
			</Card.Content>
		</Card.Root>
	{:else if list.length === 0}
		<Card.Root>
			<Card.Content class="flex flex-col items-center py-16 gap-3 text-muted-foreground">
				<UserCheck class="w-12 h-12 opacity-40" />
				<p>ยังไม่มีรายชื่อที่รอมอบตัว</p>
				<p class="text-xs">ต้องผ่านขั้นตอนจัดห้องก่อน</p>
			</Card.Content>
		</Card.Root>
	{:else}
		<Card.Root>
			<div class="overflow-x-auto">
				<Table.Root>
					<Table.Header>
						<Table.Row>
							<Table.Head class="w-24">เลขที่</Table.Head>
							<Table.Head>ชื่อ</Table.Head>
							<Table.Head>สาย</Table.Head>
							<Table.Head>ห้อง</Table.Head>
							<Table.Head>สถานะ</Table.Head>
							<Table.Head class="text-right">จัดการ</Table.Head>
						</Table.Row>
					</Table.Header>
					<Table.Body>
						{#each list as app (app.id)}
							<Table.Row class={app.status === 'enrolled' ? 'opacity-60' : ''}>
								<Table.Cell class="font-mono text-xs">{app.applicationNumber ?? '-'}</Table.Cell>
								<Table.Cell>
									<p class="font-medium text-sm">{app.fullName}</p>
									<p class="text-xs text-muted-foreground">{app.nationalId}</p>
								</Table.Cell>
								<Table.Cell class="text-sm">{app.trackName ?? '-'}</Table.Cell>
								<Table.Cell class="text-sm">{app.roomName ?? '-'}</Table.Cell>
								<Table.Cell>
									<div class="flex flex-col gap-1">
										{#if app.status === 'enrolled'}
											<Badge variant="default" class="bg-purple-600 w-fit">มอบตัวแล้ว</Badge>
										{:else}
											<Badge variant={app.studentConfirmed ? 'default' : 'secondary'} class="w-fit">
												{app.studentConfirmed ? 'ยืนยันแล้ว' : 'ยังไม่ยืนยัน'}
											</Badge>
											{#if app.preSubmitted}
												<Badge variant="outline" class="w-fit text-xs">กรอกฟอร์มแล้ว</Badge>
											{/if}
										{/if}
									</div>
								</Table.Cell>
								<Table.Cell class="text-right">
									{#if app.status !== 'enrolled'}
										<Button
											size="sm"
											onclick={() => openEnrollDialog(app)}
											class="gap-1 h-7 text-xs"
										>
											<Check class="w-3 h-3" /> รับมอบตัว
										</Button>
									{:else}
										<span class="text-xs text-green-600 flex items-center justify-end gap-1">
											<Check class="w-3 h-3" /> เสร็จสิ้น
										</span>
									{/if}
								</Table.Cell>
							</Table.Row>
						{/each}
					</Table.Body>
				</Table.Root>
			</div>
		</Card.Root>
	{/if}
</div>

<!-- Enroll Dialog -->
<Dialog.Root
	bind:open={showEnrollDialog}
	onOpenChange={(open) => {
		if (!open) resetDialog();
	}}
>
	<Dialog.Content class="max-w-lg max-h-[90vh] overflow-y-auto">
		<Dialog.Header>
			<Dialog.Title>รับมอบตัว — สร้าง Account</Dialog.Title>
			<Dialog.Description>
				{enrollingApp?.fullName} ({enrollingApp?.nationalId})
				<br />ห้อง: {enrollingApp?.roomName ?? '-'}
			</Dialog.Description>
		</Dialog.Header>
		<div class="space-y-3 py-2">
			<div class="space-y-1.5">
				<Label for="student-code">รหัสนักเรียน (ไม่บังคับ)</Label>
				<Input
					id="student-code"
					bind:value={studentCode}
					placeholder="ระบบจะสร้างให้อัตโนมัติถ้าว่าง"
				/>
				{#if enrollingApp?.assignedStudentId}
					<p class="text-xs text-blue-600">
						กำหนดไว้ล่วงหน้า: <span class="font-semibold">{enrollingApp.assignedStudentId}</span>
					</p>
				{/if}
			</div>
			<div class="text-xs text-muted-foreground bg-muted rounded p-2 space-y-0.5">
				<p>Username และ Password เริ่มต้น: รหัสนักเรียน (ที่กรอกด้านบน หรือที่ระบบสร้างให้)</p>
			</div>

			{#if enrollingApp?.preSubmitted && enrollingApp?.formData}
				<!-- แสดงข้อมูลที่นักเรียนกรอกมา (read-only) -->
				{@const fd = enrollingApp.formData}
				<div
					class="border rounded-lg p-3 space-y-3 bg-green-50 dark:bg-green-950/20 border-green-200"
				>
					<p class="text-xs font-medium text-green-700 dark:text-green-400">
						นักเรียนกรอกข้อมูลมอบตัวแล้ว
					</p>
					<div class="grid grid-cols-2 gap-2 text-sm">
						{#if fd.bloodType}
							<div><span class="text-gray-500">กลุ่มเลือด:</span> {fd.bloodType}</div>
						{/if}
						{#if fd.medicalConditions}
							<div class="col-span-2">
								<span class="text-gray-500">โรคประจำตัว:</span>
								{fd.medicalConditions}
							</div>
						{/if}
						{#if fd.allergies}
							<div class="col-span-2">
								<span class="text-gray-500">แพ้ยา/อาหาร:</span>
								{fd.allergies}
							</div>
						{/if}
					</div>
					{#if fd.father?.firstName}
						<div class="text-sm">
							<span class="text-gray-500">บิดา:</span>
							{fd.father.title ?? ''}{fd.father.firstName}
							{fd.father.lastName ?? ''}
							{fd.father.phone ? `(${fd.father.phone})` : ''}
						</div>
					{/if}
					{#if fd.mother?.firstName}
						<div class="text-sm">
							<span class="text-gray-500">มารดา:</span>
							{fd.mother.title ?? ''}{fd.mother.firstName}
							{fd.mother.lastName ?? ''}
							{fd.mother.phone ? `(${fd.mother.phone})` : ''}
						</div>
					{/if}
					{#if fd.guardians && fd.guardians.length > 0}
						{#each fd.guardians as g, i (i)}
							<div class="text-sm">
								<span class="text-gray-500">ผู้ปกครอง {i + 1}:</span>
								{g.title ?? ''}{g.firstName}
								{g.lastName ?? ''} ({g.phone}) — {g.relationship ?? ''}
							</div>
						{/each}
					{/if}
				</div>
			{:else}
				<!-- ครูกรอกข้อมูลแทน -->
				<div
					class="border rounded-lg p-3 space-y-3 bg-amber-50 dark:bg-amber-950/20 border-amber-200"
				>
					<p class="text-xs font-medium text-amber-700 dark:text-amber-400">
						นักเรียนยังไม่ได้กรอกฟอร์มมอบตัว — กรุณากรอกข้อมูลแทน
					</p>

					<!-- ข้อมูลสุขภาพ -->
					<div class="grid grid-cols-2 gap-2">
						<div class="space-y-1">
							<Label class="text-xs">กลุ่มเลือด</Label>
							<Select.Root type="single" bind:value={enrollFormData.bloodType}>
								<Select.Trigger class="h-8 text-sm">
									{enrollFormData.bloodType || '-- เลือก --'}
								</Select.Trigger>
								<Select.Content>
									{#each ['A', 'B', 'AB', 'O'] as b (b)}
										<Select.Item value={b}>{b}</Select.Item>
									{/each}
								</Select.Content>
							</Select.Root>
						</div>
					</div>
					<div class="space-y-1">
						<Label class="text-xs">โรคประจำตัว</Label>
						<Textarea
							bind:value={enrollFormData.medicalConditions}
							rows={2}
							class="text-sm resize-none"
							placeholder="ถ้าไม่มีใส่ ไม่มี"
						/>
					</div>
					<div class="space-y-1">
						<Label class="text-xs">แพ้ยา / แพ้อาหาร</Label>
						<Textarea
							bind:value={enrollFormData.allergies}
							rows={2}
							class="text-sm resize-none"
							placeholder="ถ้าไม่มีใส่ ไม่มี"
						/>
					</div>

					<!-- บิดา -->
					<div class="border border-gray-200 rounded p-2 space-y-1.5">
						<p class="text-xs font-medium text-gray-700">บิดา</p>
						<div class="grid grid-cols-3 gap-1.5">
							<div class="space-y-0.5">
								<Label class="text-xs text-gray-500">คำนำหน้า</Label>
								<Select.Root type="single" bind:value={enrollFormData.father.title}>
									<Select.Trigger class="h-7 text-xs"
										>{enrollFormData.father.title || '--'}</Select.Trigger
									>
									<Select.Content>
										<Select.Item value="นาย">นาย</Select.Item>
									</Select.Content>
								</Select.Root>
							</div>
							<div class="space-y-0.5">
								<Label class="text-xs text-gray-500">ชื่อ</Label>
								<Input
									bind:value={enrollFormData.father.firstName}
									class="h-7 text-xs"
									placeholder="ชื่อ"
								/>
							</div>
							<div class="space-y-0.5">
								<Label class="text-xs text-gray-500">นามสกุล</Label>
								<Input
									bind:value={enrollFormData.father.lastName}
									class="h-7 text-xs"
									placeholder="นามสกุล"
								/>
							</div>
						</div>
						<div class="space-y-0.5">
							<Label class="text-xs text-gray-500">เบอร์โทร</Label>
							<Input
								bind:value={enrollFormData.father.phone}
								class="h-7 text-xs"
								placeholder="0XX-XXX-XXXX"
							/>
						</div>
					</div>

					<!-- มารดา -->
					<div class="border border-gray-200 rounded p-2 space-y-1.5">
						<p class="text-xs font-medium text-gray-700">มารดา</p>
						<div class="grid grid-cols-3 gap-1.5">
							<div class="space-y-0.5">
								<Label class="text-xs text-gray-500">คำนำหน้า</Label>
								<Select.Root type="single" bind:value={enrollFormData.mother.title}>
									<Select.Trigger class="h-7 text-xs"
										>{enrollFormData.mother.title || '--'}</Select.Trigger
									>
									<Select.Content>
										<Select.Item value="นาง">นาง</Select.Item>
										<Select.Item value="นางสาว">นางสาว</Select.Item>
									</Select.Content>
								</Select.Root>
							</div>
							<div class="space-y-0.5">
								<Label class="text-xs text-gray-500">ชื่อ</Label>
								<Input
									bind:value={enrollFormData.mother.firstName}
									class="h-7 text-xs"
									placeholder="ชื่อ"
								/>
							</div>
							<div class="space-y-0.5">
								<Label class="text-xs text-gray-500">นามสกุล</Label>
								<Input
									bind:value={enrollFormData.mother.lastName}
									class="h-7 text-xs"
									placeholder="นามสกุล"
								/>
							</div>
						</div>
						<div class="space-y-0.5">
							<Label class="text-xs text-gray-500">เบอร์โทร</Label>
							<Input
								bind:value={enrollFormData.mother.phone}
								class="h-7 text-xs"
								placeholder="0XX-XXX-XXXX"
							/>
						</div>
					</div>

					<!-- ผู้ปกครอง -->
					<div class="space-y-2">
						<div class="flex items-center justify-between">
							<p class="text-xs font-medium text-gray-700">
								ผู้ปกครอง <span class="text-red-500">*</span>
							</p>
							<Button
								type="button"
								variant="outline"
								size="sm"
								onclick={addGuardian}
								class="h-6 text-xs gap-1"
							>
								<Plus class="w-3 h-3" /> เพิ่ม
							</Button>
						</div>
						{#if enrollFormData.guardians.length === 0}
							<p class="text-xs text-gray-400 text-center py-2 border border-dashed rounded">
								กรุณาเพิ่มผู้ปกครองอย่างน้อย 1 คน
							</p>
						{/if}
						{#each enrollFormData.guardians as guardian, i (i)}
							<div class="border border-blue-200 bg-blue-50/30 rounded p-2 space-y-1.5">
								<div class="flex items-center justify-between">
									<p class="text-xs font-medium text-blue-700">ผู้ปกครอง {i + 1}</p>
									<div class="flex gap-0.5">
										<Button
											type="button"
											variant="ghost"
											size="sm"
											onclick={() => copyFromFather(i)}
											class="h-5 text-xs px-1.5 text-gray-500"
										>
											<Copy class="w-3 h-3 mr-0.5" /> บิดา
										</Button>
										<Button
											type="button"
											variant="ghost"
											size="sm"
											onclick={() => copyFromMother(i)}
											class="h-5 text-xs px-1.5 text-gray-500"
										>
											<Copy class="w-3 h-3 mr-0.5" /> มารดา
										</Button>
										<Button
											type="button"
											variant="ghost"
											size="sm"
											onclick={() => removeGuardian(i)}
											class="h-5 px-1 text-red-400"
										>
											<Trash2 class="w-3 h-3" />
										</Button>
									</div>
								</div>
								<div class="grid grid-cols-3 gap-1.5">
									<div class="space-y-0.5">
										<Label class="text-xs text-gray-500">คำนำหน้า</Label>
										<Select.Root type="single" bind:value={guardian.title}>
											<Select.Trigger class="h-7 text-xs">{guardian.title || '--'}</Select.Trigger>
											<Select.Content>
												{#each ['นาย', 'นาง', 'นางสาว'] as t (t)}
													<Select.Item value={t}>{t}</Select.Item>
												{/each}
											</Select.Content>
										</Select.Root>
									</div>
									<div class="space-y-0.5">
										<Label class="text-xs text-gray-500"
											>ชื่อ <span class="text-red-500">*</span></Label
										>
										<Input bind:value={guardian.firstName} class="h-7 text-xs" placeholder="ชื่อ" />
									</div>
									<div class="space-y-0.5">
										<Label class="text-xs text-gray-500">นามสกุล</Label>
										<Input
											bind:value={guardian.lastName}
											class="h-7 text-xs"
											placeholder="นามสกุล"
										/>
									</div>
								</div>
								<div class="grid grid-cols-2 gap-1.5">
									<div class="space-y-0.5">
										<Label class="text-xs text-gray-500"
											>เบอร์โทร <span class="text-red-500">*</span></Label
										>
										<Input
											bind:value={guardian.phone}
											class="h-7 text-xs"
											placeholder="0XX-XXX-XXXX"
										/>
									</div>
									<div class="space-y-0.5">
										<Label class="text-xs text-gray-500">ความสัมพันธ์</Label>
										<Select.Root type="single" bind:value={guardian.relationship}>
											<Select.Trigger class="h-7 text-xs"
												>{guardian.relationship || '--'}</Select.Trigger
											>
											<Select.Content>
												{#each ['บิดา', 'มารดา', 'ปู่', 'ย่า', 'ตา', 'ยาย', 'ลุง', 'ป้า', 'น้า', 'อา', 'พี่', 'อื่นๆ'] as r (r)}
													<Select.Item value={r}>{r}</Select.Item>
												{/each}
											</Select.Content>
										</Select.Root>
									</div>
								</div>
							</div>
						{/each}
					</div>
				</div>
			{/if}
		</div>
		<Dialog.Footer>
			<Button
				variant="outline"
				onclick={() => {
					showEnrollDialog = false;
					resetDialog();
				}}>ยกเลิก</Button
			>
			<Button onclick={handleEnroll} disabled={enrolling || !formValid()}>
				{#if enrolling}<Loader2 class="w-4 h-4 mr-2 animate-spin" />{/if}
				{enrolling ? 'กำลังสร้าง Account...' : 'ยืนยันมอบตัว'}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
