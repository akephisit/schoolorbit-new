<script lang="ts">
	import { untrack } from 'svelte';
	import type { PageProps } from './$types';
	import {
		getRound,
		listEnrollmentPending,
		completeEnrollment,
		type AdmissionEnrollmentFormData,
		type AdmissionEnrollmentParentData,
		type EnrollmentPending
	} from '#lib/api/admission.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { RegionUpdatingState } from '#lib/components/app-state/index.js';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';
	import { toast } from 'svelte-sonner';
	import { Check, Loader2, Plus, Trash2, Copy } from '@lucide/svelte';

	interface ParentEntry extends AdmissionEnrollmentParentData {
		title: string;
		firstName: string;
		lastName: string;
		phone: string;
		relationship: string;
	}

	interface EnrollmentFormFields extends AdmissionEnrollmentFormData {
		father: Omit<ParentEntry, 'relationship'>;
		mother: Omit<ParentEntry, 'relationship'>;
		guardians: ParentEntry[];
	}

	type EnrollRow = EnrollmentPending;

	let { data }: PageProps = $props();
	let id = $derived(data.id);
	const canEnrollAdmission = $derived($can.has(PERMISSIONS.ADMISSION_ENROLL_ALL));
	const roundRequest = new LatestRequest();
	const pendingRequest = new LatestRequest();
	let renderedId = '';

	let round: Awaited<ReturnType<typeof getRound>> | null = $state(null);
	let roundLoaded = $state(false);
	let roundLoading = $state(true);
	let roundError = $state('');
	let list: EnrollRow[] = $state([]);
	let pendingLoaded = $state(false);
	let pendingLoading = $state(true);
	let pendingError = $state('');

	let showEnrollDialog = $state(false);
	let enrollingApp = $state<EnrollRow | null>(null);
	let studentCode = $state('');
	let enrolling = $state(false);

	let enrollFormData = $state<EnrollmentFormFields>({
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
		if (!canEnrollAdmission) return;
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
		if (!canEnrollAdmission) return;
		enrollFormData.guardians = [
			...enrollFormData.guardians,
			{ title: '', firstName: '', lastName: '', phone: '', relationship: '' }
		];
	}

	function removeGuardian(index: number) {
		if (!canEnrollAdmission) return;
		enrollFormData.guardians = enrollFormData.guardians.filter((_, i) => i !== index);
	}

	function copyFromFather(index: number) {
		if (!canEnrollAdmission) return;
		const g = enrollFormData.guardians[index];
		g.title = enrollFormData.father.title;
		g.firstName = enrollFormData.father.firstName;
		g.lastName = enrollFormData.father.lastName;
		g.phone = enrollFormData.father.phone;
		g.relationship = 'บิดา';
		enrollFormData.guardians = [...enrollFormData.guardians];
	}

	function copyFromMother(index: number) {
		if (!canEnrollAdmission) return;
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

	async function loadRound() {
		if (!id || !canEnrollAdmission) return;
		const sourceId = id;
		const { revision, signal } = roundRequest.begin();
		roundLoading = true;
		roundError = '';
		try {
			const value = await getRound(sourceId, { signal });
			if (!roundRequest.isCurrent(revision) || sourceId !== id) return;
			round = value;
			roundLoaded = true;
		} catch (cause) {
			if (!isAbortError(cause) && roundRequest.isCurrent(revision))
				roundError = cause instanceof Error ? cause.message : 'โหลดรอบรับสมัครไม่สำเร็จ';
		} finally {
			if (roundRequest.isCurrent(revision)) roundLoading = false;
		}
	}

	async function loadPending() {
		if (!id || !canEnrollAdmission) return;
		const sourceId = id;
		const { revision, signal } = pendingRequest.begin();
		pendingLoading = true;
		pendingError = '';
		try {
			const value = await listEnrollmentPending(sourceId, { signal });
			if (!pendingRequest.isCurrent(revision) || sourceId !== id) return;
			list = value ?? [];
			pendingLoaded = true;
		} catch (cause) {
			if (!isAbortError(cause) && pendingRequest.isCurrent(revision))
				pendingError = cause instanceof Error ? cause.message : 'โหลดรายชื่อมอบตัวไม่สำเร็จ';
		} finally {
			if (pendingRequest.isCurrent(revision)) pendingLoading = false;
		}
	}

	async function handleEnroll() {
		if (!enrollingApp || !canEnrollAdmission) return;
		const sourceId = id;
		const applicationId = enrollingApp.id;
		enrolling = true;
		try {
			const fd = needsForm ? enrollFormData : undefined;

			const res = await completeEnrollment(enrollingApp.id, studentCode || undefined, fd);
			if (sourceId !== id) return;
			pendingRequest.abort();
			pendingLoading = false;
			toast.success(`มอบตัวสำเร็จ! Username: ${res?.username}`);
			showEnrollDialog = false;
			resetDialog();
			list = list.map((row) =>
				row.id === applicationId
					? { ...row, status: 'enrolled', assignedStudentId: res.studentCode }
					: row
			);
		} catch (e) {
			if (sourceId === id) toast.error(e instanceof Error ? e.message : 'มอบตัวไม่สำเร็จ');
		} finally {
			if (sourceId === id) enrolling = false;
		}
	}

	$effect.pre(() => {
		const routeId = data.id;
		const routeRound = data.round;
		const routePending = data.pending;
		const { revision: roundRevision } = roundRequest.begin();
		const { revision: pendingRevision } = pendingRequest.begin();
		untrack(() => {
			if (renderedId !== routeId) {
				renderedId = routeId;
				round = null;
				roundLoaded = false;
				list = [];
				pendingLoaded = false;
				showEnrollDialog = false;
				resetDialog();
				enrolling = false;
			}
			roundLoading = true;
			pendingLoading = true;
			roundError = '';
			pendingError = '';
		});
		void routeRound.then((result) => {
			if (!roundRequest.isCurrent(roundRevision)) return;
			untrack(() => {
				if (result.ok && result.data) {
					round = result.data;
					roundLoaded = true;
				} else if (!result.ok) roundError = result.error;
				roundLoading = false;
			});
		});
		void routePending.then((result) => {
			if (!pendingRequest.isCurrent(pendingRevision)) return;
			untrack(() => {
				if (result.ok && result.data) {
					list = result.data;
					pendingLoaded = true;
				} else if (!result.ok) pendingError = result.error;
				pendingLoading = false;
			});
		});
	});
</script>

<PageShell
	title="รับมอบตัว"
	description={round?.name ?? 'ตรวจสอบผู้ผ่านคัดเลือกและสร้างข้อมูลนักเรียน'}
	backHref={`/staff/academic/admission/${id}`}
>
	{#if !canEnrollAdmission}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์รับมอบตัว"
			description="หน้านี้ต้องใช้สิทธิ์รับมอบตัวของงานรับสมัครก่อนจึงจะแสดงรายชื่อและสร้างบัญชีนักเรียนได้"
		/>
	{:else}
		{#if roundLoading && !roundLoaded}
			<PageSkeleton variant="detail" />
		{:else if roundError}
			<PageState
				variant="error"
				title="โหลดชื่อรอบรับสมัครไม่สำเร็จ"
				description={roundError}
				actionLabel="ลองอีกครั้ง"
				onaction={loadRound}
			/>
		{:else if roundLoading}
			<RegionUpdatingState class="static" label="กำลังอัปเดตรอบรับสมัคร..." />
		{/if}
		<!-- Stats -->
		{#if pendingLoaded}
			<div class="grid grid-cols-2 md:grid-cols-4 gap-3">
				<Card.Root class="gap-0 py-0">
					<Card.Content class="pt-5 pb-5 text-center">
						<p class="text-3xl font-bold">{list.length}</p>
						<p class="text-xs text-muted-foreground mt-1">ได้รับคัดเลือกทั้งหมด</p>
					</Card.Content>
				</Card.Root>
				<Card.Root class="gap-0 border-green-200 bg-green-50 py-0 dark:bg-green-950/20">
					<Card.Content class="pt-5 pb-5 text-center">
						<p class="text-3xl font-bold text-green-700">
							{list.filter((a) => a.studentConfirmed).length}
						</p>
						<p class="text-xs text-green-600 mt-1">ยืนยันแล้ว</p>
					</Card.Content>
				</Card.Root>
				<Card.Root class="gap-0 border-blue-200 bg-blue-50 py-0 dark:bg-blue-950/20">
					<Card.Content class="pt-5 pb-5 text-center">
						<p class="text-3xl font-bold text-blue-700">
							{list.filter((a) => a.preSubmitted).length}
						</p>
						<p class="text-xs text-blue-600 mt-1">กรอกฟอร์มล่วงหน้า</p>
					</Card.Content>
				</Card.Root>
				<Card.Root class="gap-0 border-purple-200 bg-purple-50 py-0 dark:bg-purple-950/20">
					<Card.Content class="pt-5 pb-5 text-center">
						<p class="text-3xl font-bold text-purple-700">
							{list.filter((a) => a.status === 'enrolled').length}
						</p>
						<p class="text-xs text-purple-600 mt-1">มอบตัวแล้ว</p>
					</Card.Content>
				</Card.Root>
			</div>
		{/if}

		<div aria-busy={pendingLoading}>
			{#if pendingLoading && !pendingLoaded}
				<PageSkeleton variant="table" rows={5} columns={7} />
			{:else if pendingError && !pendingLoaded}
				<PageState
					variant="error"
					title="โหลดรายชื่อมอบตัวไม่สำเร็จ"
					description={pendingError}
					actionLabel="ลองอีกครั้ง"
					onaction={loadPending}
				/>
			{:else if pendingLoaded}
				{#if pendingLoading}<RegionUpdatingState
						class="static"
						label="กำลังอัปเดตรายชื่อมอบตัว..."
					/>{/if}
				{#if pendingError}
					<p role="alert" class="text-sm text-destructive">
						{pendingError}
						<Button size="sm" variant="outline" onclick={loadPending}>ลองใหม่</Button>
					</p>
				{/if}
				{#if list.length === 0}
					<PageState title="ยังไม่มีรายชื่อที่รอมอบตัว" description="ต้องผ่านขั้นตอนจัดห้องก่อน" />
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
											<Table.Cell class="font-mono text-xs"
												>{app.applicationNumber ?? '-'}</Table.Cell
											>
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
														<Badge
															variant={app.studentConfirmed ? 'default' : 'secondary'}
															class="w-fit"
														>
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
			{/if}
		</div>
	{/if}
</PageShell>

<!-- Enroll Dialog -->
{#if canEnrollAdmission}
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
												<Select.Trigger class="h-7 text-xs">{guardian.title || '--'}</Select.Trigger
												>
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
											<Input
												bind:value={guardian.firstName}
												class="h-7 text-xs"
												placeholder="ชื่อ"
											/>
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
{/if}
