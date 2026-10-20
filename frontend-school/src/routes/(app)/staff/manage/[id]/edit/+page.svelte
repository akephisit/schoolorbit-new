<script lang="ts">
	import { organizationPrimaryError, organizationPrimaryLabel, setOrganizationPrimary } from '#lib/features/staff/organization-members.js';
	import { careerFieldErrors, type StaffCareerCorrectionReasons } from '#lib/forms/staff-career.js';
	import { beforeNavigate, goto, invalidate } from '$app/navigation';
	import { page } from '$app/state';
	import { staffReturnHref, withStaffReturn } from '#lib/navigation/staff-management.js';
	import StaffBreadcrumb from '#lib/components/staff/StaffBreadcrumb.svelte';
	import * as Card from '#lib/components/ui/card/index.js';
	import StaffNameFields from '#lib/components/staff/StaffNameFields.svelte';
	import { STAFF_NAME_FIELDS, staffNameDraft, staffNameErrors } from '#lib/forms/staff-name.js';
	import StaffLicensePreview from '#lib/components/staff/StaffLicensePreview.svelte';
	import StaffPersonnelFields from '#lib/components/staff/StaffPersonnelFields.svelte';
	import {
		buildStaffPersonnelPatch,
		staffPersonnelDraft,
		staffPersonnelErrors,
		orderedStaffEducations,
		EDUCATION_LEVEL_LABELS,
		type StaffPersonnelDraft
	} from '#lib/forms/staff-personnel.js';
	import { staffStatusLabel } from '#lib/forms/staff-status.js';
	import { resolve } from '$app/paths';
	import { toast } from 'svelte-sonner';
	import type { PageProps } from './$types';
	import {
		getStaffProfile,
		updateStaff,
		listOrganizationUnits,
		type StaffProfileResponse,
		type UpdateStaffRequest,
		type OrganizationUnit
	} from '#lib/api/staff.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { DatePicker } from '#lib/components/ui/date-picker/index.js';
	import ProfileImageUpload from '#lib/components/forms/ProfileImageUpload.svelte';
	import { Building2, LoaderCircle, Save, Shield } from '@lucide/svelte';
	import { onDestroy, untrack } from 'svelte';
	import { SvelteURLSearchParams } from 'svelte/reactivity';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { captureRouteLoad } from '#lib/navigation/route-load.js';
	import { requireApiData } from '#lib/api/client.js';
	import { PERMISSIONS, PERMISSION_MODULES } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';
	import { authStore } from '#lib/stores/auth.js';

	let { data }: PageProps = $props();
	const staffId = $derived(data.staffId),
		source = $derived(data.staff);
	const staffRequest = new LatestRequest();
	let activeId = '';
	const currentUserId = $derived($authStore.user?.id ?? '');
	const canReadStaff = $derived(
		$can.hasAny(
			PERMISSIONS.STAFF_PROFILE_READ_ORGANIZATION_UNIT,
			PERMISSIONS.STAFF_PROFILE_READ_ORGANIZATION_TREE,
			PERMISSIONS.STAFF_PROFILE_READ_SCHOOL
		) ||
			(currentUserId === staffId && $can.has(PERMISSIONS.STAFF_PROFILE_READ_OWN))
	);

	// Form state
	let currentStep = $state(1);
	const sections = [
		{ value: 'personal', label: 'ข้อมูลส่วนตัว' },
		{ value: 'education', label: 'งานและการศึกษา' },
		{ value: 'organizations', label: 'สังกัดหน่วยงาน' }
	];
	const returnHref = $derived(staffReturnHref(new URL(page.url.href)));
	const profileHref = $derived(withStaffReturn(`/staff/manage/${staffId}`, returnHref));
	function sectionHref(value: string) {
		const query = new SvelteURLSearchParams(page.url.search);
		query.set('section', value);
		return resolve(`staff/manage/${staffId}/edit?${query}`);
	}
	$effect.pre(() => {
		const section = page.url.searchParams.get('section');
		currentStep = Math.max(1, sections.findIndex((item) => item.value === section) + 1);
	});

	// Loading states
	let loadingProfile = $state(true);
	let saving = $state(false);
	let uploadingLicense = $state(false);
	let latestLicense = $state<{ fileId: string | null } | null>(null);

	let loadingOrganizationUnits = $state(false);

	const organizationRequest = new LatestRequest();
	const canReadOptions = $derived($can.has(PERMISSIONS.ROLES_READ_ALL));
	const canMutateStaff = $derived($can.has(PERMISSIONS.STAFF_UPDATE_ALL));
	const canManageLicense = $derived(
		canMutateStaff &&
			($can.has(PERMISSIONS.STAFF_PII_READ_SCHOOL) ||
				(currentUserId === staffId && $can.has(PERMISSIONS.STAFF_PII_READ_OWN)))
	);
	let organizationsLoaded = $state(false),
		organizationError = $state('');
	let disposed = false,
		mutationEpoch = 0;
	// Data
	let staff: StaffProfileResponse | null = $state(null);
	let organizationUnits: OrganizationUnit[] = $state([]);

	// Form data
	let formData = $state({
		// Personal Information
		profile_image_file_id: '',
		username: '',
		...staffNameDraft(),
		nickname: '',
		email: '',
		phone: '',
		emergency_contact: '',
		line_id: '',
		date_of_birth: '',
		gender: '',
		address: '',
		hired_date: '',
		status: 'active',

		// Staff Info
		personnel: staffPersonnelDraft(null),

		// Organization Units
		organization_assignments: [] as Array<{
			organization_unit_id: string;
			position_code: string;
			is_primary: boolean;
			responsibilities: string;
		}>
	});

	let selectedPosition =
		$state<NonNullable<StaffProfileResponse['staff_info']>['job_position']>(null);
	let originalForm: typeof formData | null = $state(null);
	const dirty = $derived(
		originalForm !== null && JSON.stringify(formData) !== JSON.stringify(originalForm)
	);

	beforeNavigate(({ to, cancel, willUnload, shallow }) => {
		if (shallow) return;
		if (!dirty || saving || to?.url.pathname === page.url.pathname) return;
		if (willUnload || !window.confirm('มีข้อมูลที่ยังไม่บันทึก ต้องการออกจากหน้านี้หรือไม่?'))
			cancel();
	});
	function discardChanges() {
		if (!originalForm || saving) return;
		formData = structuredClone($state.snapshot(originalForm));
		errors = {};
	}
	// Validation errors
	let errors = $state<Record<string, string>>({});
	let correctionReasons = $state<StaffCareerCorrectionReasons>({});
	let careerConflict = $state(false);
	let latestEducation = $state<StaffPersonnelDraft | null>(null);

	$effect.pre(() => {
		const id = staffId,
			read = source;
		untrack(() => {
			if (activeId !== id) {
				activeId = id;
				latestEducation = null;
				careerConflict = false;
				latestLicense = null;
				mutationEpoch++;
				staff = null;
				saving = false;
				originalForm = null;
				organizationRequest.abort();
				organizationUnits = [];
				organizationsLoaded = false;
				errors = {};
			}
			const ticket = staffRequest.begin();
			loadingProfile = true;
			errors.load = '';
			void read.then((result) => applyStaff(result, ticket.revision));
		});
		return () => staffRequest.abort();
	});
	onDestroy(() => {
		disposed = true;
		mutationEpoch++;
		staffRequest.abort();
		organizationRequest.abort();
	});
	function applyStaff(result: Awaited<typeof data.staff>, revision: number) {
		if (!staffRequest.isCurrent(revision)) return;
		loadingProfile = false;
		if (!result.ok) {
			errors.load = result.error;
			return;
		}
		if (!result.data) {
			staff = null;
			return;
		}
		const initial = staff === null;
		staff = result.data;
		if (initial) {
			selectedPosition = staff.staff_info?.job_position ?? null;
			// Populate form
			formData = {
				profile_image_file_id: staff.profile_image_file_id || '',
				username: staff.username || '',
				...staffNameDraft(staff),
				nickname: staff.nickname || '',
				email: staff.email || '',
				phone: staff.phone || '',
				emergency_contact: staff.emergency_contact || '',
				line_id: staff.line_id || '',
				date_of_birth: staff.date_of_birth || '',
				gender: staff.gender || '',
				address: staff.address || '',
				hired_date: staff.hired_date || '',
				status: staff.status,
				personnel: staffPersonnelDraft(staff.staff_info),
				organization_assignments:
					staff.organization_units?.map((d) => ({
						organization_unit_id: d.id,
						position_code: d.position_code || 'member',
						is_primary: d.is_primary || false,
						responsibilities: d.responsibilities || ''
					})) || []
			};
			originalForm = structuredClone($state.snapshot(formData));
		}
	}
	async function loadStaffProfile() {
		if (!canReadStaff) return;
		const ticket = staffRequest.begin();
		loadingProfile = true;
		errors.load = '';
		applyStaff(
			await captureRouteLoad(
				getStaffProfile(staffId, { signal: ticket.signal }).then((reply) =>
					requireApiData(reply, 'โหลดข้อมูลบุคลากรไม่สำเร็จ')
				),
				'โหลดข้อมูลบุคลากรไม่สำเร็จ'
			),
			ticket.revision
		);
	}

	$effect.pre(() => {
		const step = currentStep,
			allowed = canReadOptions;
		untrack(() => {
			if (!allowed) {
				organizationRequest.abort();
				organizationUnits = [];
				organizationsLoaded = false;
				loadingOrganizationUnits = false;
			} else if (step === 3 && !organizationsLoaded) void loadOrganizationOptions();
		});
		return () => {
			if (step === 3) organizationRequest.abort();
		};
	});
	async function loadOrganizationOptions() {
		if (!canReadOptions || currentStep !== 3) return;
		const ticket = organizationRequest.begin();
		loadingOrganizationUnits = true;
		organizationError = '';
		const result = await captureRouteLoad(
			listOrganizationUnits(undefined, { signal: ticket.signal }).then((reply) =>
				requireApiData(reply, 'โหลดตัวเลือกหน่วยงานไม่สำเร็จ')
			),
			'โหลดตัวเลือกหน่วยงานไม่สำเร็จ'
		);
		if (!organizationRequest.isCurrent(ticket.revision)) return;
		loadingOrganizationUnits = false;
		if (result.ok) {
			organizationUnits = result.data;
			organizationsLoaded = true;
		} else organizationError = result.error;
	}
	// Validation functions
	function validateStep1(): boolean {
		errors = {};

		if (
			originalForm &&
			STAFF_NAME_FIELDS.some((field) => formData[field].trim() !== originalForm![field].trim())
		)
			Object.assign(errors, staffNameErrors(formData));

		if (formData.email && !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(formData.email)) {
			errors.email = 'รูปแบบอีเมลไม่ถูกต้อง';
		}

		if (formData.phone && !/^[0-9-]+$/.test(formData.phone)) {
			errors.phone = 'หมายเลขโทรศัพท์ไม่ถูกต้อง';
		}

		Object.assign(errors, staffPersonnelErrors(formData.personnel));
		Object.assign(errors, careerFieldErrors(formData.personnel.career));
		if (originalForm) {
			try {
				buildStaffPersonnelPatch(originalForm.personnel, formData.personnel, correctionReasons);
			} catch (error) {
				errors.career = error instanceof Error ? error.message : 'ข้อมูลไม่ถูกต้อง';
			}
		}
		return Object.keys(errors).length === 0;
	}

	function validateOrganizations(): boolean {
		errors = {};
		if (
			!formData.organization_assignments.length ||
			formData.organization_assignments.some((item) => !item.organization_unit_id)
		)
			errors.organization_units = 'กรุณาเลือกหน่วยงานให้ครบทุกแถว';
		else if (
			new Set(formData.organization_assignments.map((item) => item.organization_unit_id)).size !==
			formData.organization_assignments.length
		)
			errors.organization_units = 'เลือกหน่วยงานซ้ำกัน กรุณาตรวจสอบสังกัด';
		else errors.organization_units = organizationPrimaryError(formData.organization_assignments, organizationUnits);
		if (!errors.organization_units) delete errors.organization_units;
		return Object.keys(errors).length === 0;
	}
	// OrganizationUnit management
	function addOrganizationUnit() {
		const isFirst = formData.organization_assignments.length === 0;
		formData.organization_assignments = [
			...formData.organization_assignments,
			{
				organization_unit_id: '',
				position_code: 'member',
				is_primary: isFirst,
				responsibilities: ''
			}
		];
	}

	function removeOrganizationUnit(index: number) {
		const removed = formData.organization_assignments[index];
		formData.organization_assignments = formData.organization_assignments.filter(
			(_, i) => i !== index
		);
		if (removed?.is_primary) {
			const type = organizationUnits.find((unit) => unit.id === removed.organization_unit_id)?.unit_type === 'subject_group';
			const replacement = formData.organization_assignments.findIndex((item) => (organizationUnits.find((unit) => unit.id === item.organization_unit_id)?.unit_type === 'subject_group') === type);
			if (replacement >= 0) setPrimaryOrganizationUnit(replacement);
		}
	}

	function setPrimaryOrganizationUnit(index: number) {
		formData.organization_assignments = setOrganizationPrimary(formData.organization_assignments, index, organizationUnits);
	}

	// Submit form
	async function handleSubmit() {
		if (
			!canMutateStaff ||
			saving ||
			uploadingLicense ||
			careerConflict ||
			!staff ||
			!originalForm ||
			!dirty
		)
			return;
		if (!validateStep1()) {
			await goto(
				sectionHref(
					errors.major ||
						errors.university ||
						Object.keys(errors).some((key) => key === 'career' || key.includes('.'))
						? 'education'
						: 'personal'
				),
				{
					reset: false
				}
			);
			return;
		}
		const organizationsChanged =
			JSON.stringify(formData.organization_assignments) !==
			JSON.stringify(originalForm?.organization_assignments);
		if (organizationsChanged && !validateOrganizations()) {
			await goto(sectionHref('organizations'), { reset: false });
			return;
		}
		const owner = staffId,
			epoch = ++mutationEpoch;
		const current = () => !disposed && owner === staffId && epoch === mutationEpoch;
		staffRequest.abort();
		loadingProfile = false;
		if (!staffId) return;

		saving = true;
		errors = {};

		try {
			const payload: UpdateStaffRequest = {};
			const personalFields = [
				...STAFF_NAME_FIELDS,
				'nickname',
				'email',
				'phone',
				'emergency_contact',
				'line_id',
				'date_of_birth',
				'gender',
				'address',
				'hired_date',
				'status'
			] as const;
			for (const field of personalFields) {
				if (formData[field] !== originalForm[field]) {
					const value =
						field === 'first_name' || field === 'last_name'
							? formData[field].trim()
							: formData[field];
					payload[field] = STAFF_NAME_FIELDS.some((name) => name === field)
						? value
						: value || undefined;
				}
			}
			const hrPatch = buildStaffPersonnelPatch(
				originalForm.personnel,
				formData.personnel,
				correctionReasons
			);
			if (hrPatch) payload.staff_info = hrPatch;
			if (organizationsChanged)
				payload.organization_assignments = formData.organization_assignments;

			const result = await updateStaff(owner, payload);
			if (!current()) return;

			if (result.success) {
				toast.success('บันทึกข้อมูลสำเร็จ');
				originalForm = structuredClone($state.snapshot(formData));
				await invalidate(`school:staff-profile:${owner}`);
				if (!current()) return;
				await goto(
					resolve(
						withStaffReturn(`/staff/manage/${owner}`, returnHref).slice(
							1
						) as `staff/manage/${string}`
					)
				);
			} else {
				careerConflict = result.status === 409;
				toast.error(result.error || 'เกิดข้อผิดพลาดในการบันทึกข้อมูล');
			}
		} catch (e) {
			if (current())
				toast.error(e instanceof Error ? e.message : 'เกิดข้อผิดพลาดในการบันทึกข้อมูล');
		} finally {
			if (current()) saving = false;
		}
	}

	async function reconcileCareerDraft() {
		if (saving || !originalForm) return;
		const owner = staffId,
			ticket = staffRequest.begin();
		saving = true;
		try {
			const latest = await getStaffProfile(owner, { signal: ticket.signal });
			if (disposed || owner !== staffId || !staffRequest.isCurrent(ticket.revision)) return;
			const latestPersonnel = staffPersonnelDraft(
				requireApiData(latest, 'โหลดข้อมูลล่าสุดไม่สำเร็จ').staff_info
			);
			const updated = latestPersonnel.career;
			const previous = originalForm.personnel.career;
			for (const key of ['personnelType', 'jobPosition', 'academicRank'] as const) {
				const draft = formData.personnel.career[key];
				const fresh = updated[key];
				if (draft.value === previous[key].value) {
					if (key === 'personnelType')
						formData.personnel.career.personnelType.value = updated.personnelType.value;
					else if (key === 'academicRank')
						formData.personnel.career.academicRank.value = updated.academicRank.value;
					else formData.personnel.career.jobPosition.value = updated.jobPosition.value;
				}
				for (const field of ['effectiveDate', 'orderDate', 'orderNumber', 'note'] as const)
					if (draft[field] === previous[key][field]) draft[field] = fresh[field];
				draft.reference = fresh.reference;
			}
			originalForm.personnel.career = updated;
			if (latestPersonnel.education_revision !== originalForm.personnel.education_revision) {
				const changed =
					JSON.stringify(formData.personnel.educations) !==
					JSON.stringify(originalForm.personnel.educations);
				if (changed) latestEducation = latestPersonnel;
				else {
					formData.personnel.educations = structuredClone(latestPersonnel.educations);
					formData.personnel.education_revision = latestPersonnel.education_revision;
					originalForm.personnel.educations = structuredClone(latestPersonnel.educations);
					originalForm.personnel.education_revision = latestPersonnel.education_revision;
				}
			}
			if (
				latestPersonnel.teaching_license_file_id !== originalForm.personnel.teaching_license_file_id
			) {
				if (
					formData.personnel.teaching_license_file_id !==
					originalForm.personnel.teaching_license_file_id
				)
					latestLicense = { fileId: latestPersonnel.teaching_license_file_id };
				else {
					formData.personnel.teaching_license_file_id = latestPersonnel.teaching_license_file_id;
					originalForm.personnel.teaching_license_file_id =
						latestPersonnel.teaching_license_file_id;
				}
			}
			careerConflict = latestEducation !== null || latestLicense !== null;
			toast.success('โหลดข้อมูลล่าสุดแล้ว กรุณาตรวจสอบร่างก่อนบันทึก');
		} catch (error) {
			if (!disposed && owner === staffId)
				toast.error(error instanceof Error ? error.message : 'โหลดข้อมูลล่าสุดไม่สำเร็จ');
		} finally {
			if (!disposed && owner === staffId) saving = false;
		}
	}

	function useLatestEducation() {
		if (!latestEducation || !originalForm || saving) return;
		formData.personnel.educations = structuredClone($state.snapshot(latestEducation.educations));
		originalForm.personnel.educations = structuredClone(
			$state.snapshot(latestEducation.educations)
		);
		formData.personnel.education_revision = latestEducation.education_revision;
		originalForm.personnel.education_revision = latestEducation.education_revision;
		latestEducation = null;
		careerConflict = latestLicense !== null;
	}
	function resolveLicenseConflict(useLatest: boolean) {
		if (!latestLicense || !originalForm || saving) return;
		originalForm.personnel.teaching_license_file_id = latestLicense.fileId;
		if (useLatest) formData.personnel.teaching_license_file_id = latestLicense.fileId;
		latestLicense = null;
		careerConflict = latestEducation !== null;
	}
	// Get display label for value
	function getGenderLabel(value: string): string {
		const labels: Record<string, string> = {
			male: 'ชาย',
			female: 'หญิง',
			other: 'อื่นๆ'
		};
		return labels[value] || 'เลือกเพศ';
	}

	function getPositionLabel(value: string): string {
		const labels: Record<string, string> = {
			member: 'สมาชิก',
			head: 'หัวหน้า'
		};
		return labels[value] || 'เลือกตำแหน่ง';
	}

	async function updateProfileImage(fileId: string | null, owner: string) {
		if (disposed || owner !== staffId || !canMutateStaff || currentStep !== 1) return;
		const epoch = mutationEpoch;
		const current = () => !disposed && epoch === mutationEpoch && owner === staffId;
		const toastId = toast.loading('กำลังอัปเดตรูปโปรไฟล์...');
		staffRequest.abort();
		loadingProfile = false;
		try {
			const reply = await updateStaff(owner, { profile_image_file_id: fileId });
			if (!current()) {
				toast.dismiss(toastId);
				return;
			}
			if (!reply.success) throw new Error(reply.error || 'ไม่สามารถอัปเดตรูปภาพได้');
			formData.profile_image_file_id = fileId ?? '';
			if (originalForm) originalForm.profile_image_file_id = fileId ?? '';
			if (staff) staff = { ...staff, profile_image_file_id: fileId };
			toast.success('อัปเดตรูปโปรไฟล์เรียบร้อยแล้ว', { id: toastId });
		} catch (e) {
			if (current())
				toast.error(e instanceof Error ? e.message : 'ไม่สามารถอัปเดตรูปภาพได้', { id: toastId });
			else toast.dismiss(toastId);
		}
	}
</script>

<PageShell
	title="แก้ไขข้อมูลบุคลากร"
	description={staff && canReadStaff
		? `${staff.display_name} • เลือกหมวดที่ต้องการแก้ไข`
		: 'แก้ไขข้อมูลบุคลากร'}
	backHref={profileHref}
	backLabel="กลับข้อมูลบุคลากร"
	backPreload="tap"
>
	{#snippet meta()}<StaffBreadcrumb
			{returnHref}
			name={staff && canReadStaff ? staff.display_name : undefined}
			{profileHref}
			current="แก้ไข"
		/>{/snippet}

	<div data-testid="staff-edit-profile" aria-busy={loadingProfile} class="space-y-6">
		{#if errors.load && staff && canReadStaff}<PageState
				title="อัปเดตข้อมูลบุคลากรไม่สำเร็จ"
				description={errors.load}
				actionLabel="ลองอีกครั้ง"
				onaction={loadStaffProfile}
			/>{/if}

		{#if !canReadStaff}<PageState variant="permission" title="ไม่มีสิทธิ์ดูข้อมูลบุคลากร" />
		{:else if loadingProfile && !staff}
			<div role="status" aria-label="กำลังโหลดข้อมูลบุคลากร">
				<PageSkeleton variant="form" rows={8} />
			</div>
		{:else if errors.load && !staff}
			<PageState
				variant="error"
				title="โหลดข้อมูลบุคลากรไม่สำเร็จ"
				description={errors.load}
				actionLabel="ลองอีกครั้ง"
				onaction={loadStaffProfile}
			/>
		{:else if staff}
			<nav
				aria-label="หมวดข้อมูลที่แก้ไข"
				class="flex flex-wrap gap-2 rounded-xl border bg-card p-3"
			>
				{#each sections as section, index (section.value)}
					<Button
						href={sectionHref(section.value)}
						data-sveltekit-preload-data={false}
						variant={currentStep === index + 1 ? 'default' : 'ghost'}
						aria-current={currentStep === index + 1 ? 'page' : undefined}>{section.label}</Button
					>
				{/each}
				{#if $can.hasModule(PERMISSION_MODULES.ROLES)}<Button
						href={withStaffReturn(`/staff/manage/${staffId}/roles`, returnHref)}
						variant="outline"
						class="gap-2"><Shield class="size-4" />บทบาทและสิทธิ์</Button
					>{/if}
			</nav>

			<form
				onsubmit={(e) => {
					e.preventDefault();
					void handleSubmit();
				}}
			>
				<Card.Root class="gap-0 py-0"
					><Card.Content class="p-4 sm:p-5">
						{#if currentStep === 1}
							{@const imageOwner = staffId}
							<!-- Step 1: Personal Information -->
							<h2 class="text-base font-semibold mb-4">ข้อมูลส่วนตัว</h2>

							<div class="mb-5 flex flex-wrap items-center gap-4 rounded-xl bg-muted/30 p-3">
								{#key staffId}<ProfileImageUpload
										size="sm"
										currentFileId={formData.profile_image_file_id}
										resourceId={staffId}
										disabled={saving}
										onsuccess={(upload) => updateProfileImage(upload.fileId, imageOwner)}
										onerror={(message) => {
											if (!disposed && imageOwner === staffId) errors.profile_image = message;
										}}
										>{#snippet helper()}<span class="sr-only">เปลี่ยนรูปบุคลากร</span
											>{/snippet}</ProfileImageUpload
									>{/key}
								<div class="min-w-0 flex-1">
									<p class="break-words text-sm font-semibold">{staff.display_name}</p>
									<dl class="mt-1 text-xs text-muted-foreground">
										<dt class="inline">รหัสบุคลากร:</dt>
										<dd class="inline">{formData.username}</dd>
									</dl>
									<p class="mt-2 text-xs text-muted-foreground">
										JPG, PNG, WebP, HEIC · สูงสุด 10 MB
									</p>
								</div>
							</div>

							<div class="space-y-4">
								<StaffNameFields
									bind:value={formData}
									{errors}
									disabled={saving || !canMutateStaff}
								/>
								<div class="max-w-sm space-y-2">
									<Label for="staff-gender">เพศ</Label><Select.Root
										type="single"
										bind:value={formData.gender}
									>
										<Select.Trigger id="staff-gender"
											>{getGenderLabel(formData.gender)}</Select.Trigger
										>
										<Select.Content>
											<Select.Item value="male">ชาย</Select.Item>
											<Select.Item value="female">หญิง</Select.Item>
											<Select.Item value="other">อื่นๆ</Select.Item>
										</Select.Content>
									</Select.Root>
								</div>
								<div>
									<Label for="staff-nickname" class="mb-2">ชื่อเล่น</Label>
									<Input
										id="staff-nickname"
										type="text"
										bind:value={formData.nickname}
										placeholder="ชื่อเล่น"
									/>
								</div>

								<div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
									<div>
										<Label for="staff-email" class="mb-2">อีเมล</Label>
										<Input
											id="staff-email"
											type="email"
											bind:value={formData.email}
											aria-label="อีเมล"
											aria-invalid={Boolean(errors.email)}
											placeholder="email@school.ac.th (ไม่บังคับ)"
											class="w-full px-3 py-2 border border-border rounded-md
										{errors.email ? 'border-destructive' : ''}"
										/>
										{#if errors.email}
											<p class="text-xs text-destructive mt-1">{errors.email}</p>
										{/if}
									</div>

									<div>
										<Label for="staff-phone" class="mb-2">หมายเลขโทรศัพท์</Label>
										<Input
											id="staff-phone"
											type="tel"
											bind:value={formData.phone}
											aria-label="เบอร์โทรศัพท์"
											aria-invalid={Boolean(errors.phone)}
											placeholder="081-234-5678"
											class="w-full px-3 py-2 border border-border rounded-md
										{errors.phone ? 'border-destructive' : ''}"
										/>
										{#if errors.phone}
											<p class="text-xs text-destructive mt-1">{errors.phone}</p>
										{/if}
									</div>
								</div>

								<div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
									<div>
										<Label for="staff-date_of_birth" class="mb-2">วันเกิด</Label>
										<DatePicker
											id="staff-date_of_birth"
											ariaLabel="วันเกิด"
											bind:value={formData.date_of_birth}
											placeholder="เลือกวันเกิด"
										/>
									</div>

									<div>
										<Label for="staff-hired_date" class="mb-2">วันที่เริ่มงาน</Label>
										<DatePicker
											ariaLabel="วันที่เริ่มงาน"
											id="staff-hired_date"
											bind:value={formData.hired_date}
											placeholder="เลือกวันที่เริ่มงาน"
										/>
									</div>
								</div>

								<div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
									<div>
										<Label for="staff-line_id" class="mb-2">Line ID</Label>
										<Input
											id="staff-line_id"
											type="text"
											bind:value={formData.line_id}
											placeholder="@lineid"
										/>
									</div>

									<div>
										<Label for="staff-emergency_contact" class="mb-2">เบอร์ติดต่อฉุกเฉิน</Label>
										<Input
											id="staff-emergency_contact"
											type="tel"
											bind:value={formData.emergency_contact}
											placeholder="081-234-5678"
										/>
									</div>
								</div>

								<div>
									<Label for="staff-address" class="mb-2">ที่อยู่</Label>
									<Textarea
										id="staff-address"
										bind:value={formData.address}
										placeholder="ที่อยู่ปัจจุบัน"
										rows={3}
									/>
								</div>

								<div>
									<Label class="mb-2">สถานะ</Label>
									<Select.Root type="single" bind:value={formData.status}>
										<Select.Trigger>{staffStatusLabel(formData.status)}</Select.Trigger>
										<Select.Content>
											<Select.Item value="active">ใช้งาน</Select.Item>
											<Select.Item value="inactive">ปิดการใช้งาน</Select.Item>
											<Select.Item value="suspended">ระงับ</Select.Item>
											<Select.Item value="resigned">ลาออก</Select.Item>
											<Select.Item value="retired">เกษียณ</Select.Item>
										</Select.Content>
									</Select.Root>
								</div>
							</div>
						{:else if currentStep === 2}
							<h2 class="text-base font-semibold mb-4">ตำแหน่งและการศึกษา</h2>
							{#if careerConflict}<div
									class="mb-4 space-y-3 rounded-xl border bg-muted/40 p-4"
									role="alert"
								>
									<p class="text-sm">
										ข้อมูลบุคลากรเปลี่ยนไปแล้ว ร่างของคุณยังอยู่
										กรุณาโหลดข้อมูลล่าสุดและตรวจสอบก่อนบันทึก
									</p>
									<Button
										type="button"
										variant="outline"
										disabled={saving}
										onclick={reconcileCareerDraft}>โหลดข้อมูลล่าสุดและตรวจสอบร่าง</Button
									>
									{#if latestLicense && canManageLicense}
										<div class="space-y-2 border-t pt-3">
											<p class="text-sm font-medium">เอกสารล่าสุดในระบบ</p>
											{#if latestLicense.fileId}<StaffLicensePreview
													{staffId}
													fileId={latestLicense.fileId}
												/>{:else}<p class="text-sm">ไม่มีเอกสารแนบ</p>{/if}
											<p class="text-xs text-muted-foreground">
												ร่างเอกสารของคุณยังอยู่ เลือกว่าจะเก็บเอกสารใดก่อนบันทึก
											</p>
											<div class="flex flex-wrap gap-2">
												<Button
													type="button"
													variant="outline"
													onclick={() => resolveLicenseConflict(true)}>ใช้เอกสารล่าสุด</Button
												><Button
													type="button"
													variant="outline"
													onclick={() => resolveLicenseConflict(false)}
													>ใช้ร่างแทนเอกสารล่าสุด</Button
												>
											</div>
										</div>
									{/if}
									{#if latestEducation}
										<div class="space-y-3 border-t pt-3">
											<p class="text-sm font-medium">รายการวุฒิล่าสุดในระบบ</p>
											<ul class="space-y-2 text-sm">
												{#each orderedStaffEducations(latestEducation.educations) as row (row.key)}<li
													>
														{row.education_level
															? EDUCATION_LEVEL_LABELS[row.education_level]
															: 'ยังไม่ระบุระดับ'} · {row.major || 'ยังไม่ระบุสาขา'} · {row.university ||
															'ยังไม่ระบุสถาบัน'}
													</li>{:else}<li>ยังไม่มีรายการ</li>{/each}
											</ul>
											<p class="text-xs text-muted-foreground">
												ร่างวุฒิของคุณยังอยู่ด้านล่าง
												เปรียบเทียบและจดข้อมูลที่ต้องการเก็บก่อนใช้รายการล่าสุด แล้วจึงแก้ไขต่อ
												ข้อมูลส่วนอื่นในร่างจะยังอยู่
											</p>
											<Button type="button" variant="outline" onclick={useLatestEducation}
												>ใช้รายการวุฒิล่าสุดแทนร่างวุฒิ</Button
											>
										</div>
									{/if}
								</div>{/if}

							<StaffPersonnelFields
								{staffId}
								{canManageLicense}
								onUploadPendingChange={(pending) => (uploadingLicense = pending)}
								bind:value={formData.personnel}
								originalCareer={originalForm?.personnel.career}
								bind:correctionReasons
								bind:selectedPosition
								{errors}
								disabled={saving || !canMutateStaff}
							/>
						{:else if currentStep === 3}
							<!-- Step 4: Organization Units -->
							<h2 class="text-base font-semibold mb-4">สังกัดหน่วยงาน</h2>

							{#if !canReadOptions}<PageState
									variant="permission"
									title="ไม่มีสิทธิ์อ่านตัวเลือกหน่วยงาน"
								/>
							{:else if loadingOrganizationUnits}<div
									role="status"
									aria-label="กำลังโหลดตัวเลือกหน่วยงาน"
								>
									<PageSkeleton variant="form" rows={3} />
								</div>
							{:else if organizationError}<PageState
									variant="error"
									title="โหลดตัวเลือกหน่วยงานไม่สำเร็จ"
									description={organizationError}
									actionLabel="ลองอีกครั้ง"
									onaction={loadOrganizationOptions}
								/>
							{:else}
								<div class="space-y-4">
									<p class="text-sm text-muted-foreground">
										ระบุหน่วยงาน/กลุ่มที่บุคลากรสังกัดและตำแหน่งในหน่วยงาน
									</p>

									{#if errors.organization_units}
										<p class="text-sm text-destructive">{errors.organization_units}</p>
									{/if}

									{#each formData.organization_assignments as dept, i (i)}
										<div class="p-4 border border-border rounded-lg">
											<div class="flex items-start justify-between mb-4">
												<h3 class="font-medium">หน่วยงานที่ {i + 1}</h3>
												{#if formData.organization_assignments.length > 1}
													<Button
														variant="ghost"
														size="sm"
														type="button"
														onclick={() => removeOrganizationUnit(i)}
														class="text-destructive hover:text-destructive/80 text-sm"
													>
														ลบ
													</Button>
												{/if}
											</div>

											<div class="space-y-3">
												<div>
													<Label class="mb-2">ชื่อหน่วยงาน</Label>
													<Select.Root type="single" bind:value={dept.organization_unit_id}>
														<Select.Trigger>
															{#if dept.organization_unit_id}
																{organizationUnits.find((d) => d.id === dept.organization_unit_id)
																	?.name || 'เลือกหน่วยงาน'}
															{:else}
																เลือกหน่วยงาน
															{/if}
														</Select.Trigger>
														<Select.Content
															class="max-h-[300px] overflow-y-auto w-full max-w-[400px]"
														>
															<Select.Item value="">เลือกหน่วยงาน</Select.Item>

															<!-- Group by Parent Organization Units (Administrative Groups) -->
															{#each organizationUnits.filter((d) => !d.parent_unit_id) as parentDept (parentDept.id)}
																<Select.Group>
																	<Select.Label
																		class="font-bold text-primary flex items-center gap-2 bg-muted/30 px-2 py-1"
																	>
																		<Building2 class="w-3 H-3" />
																		{parentDept.name}
																	</Select.Label>

																	<!-- Parent itself (Optional, allow assigning to root group) -->
																	<Select.Item
																		value={parentDept.id}
																		class="pl-4 font-semibold text-muted-foreground/80"
																	>
																		— สังกัด {parentDept.name} (ส่วนกลาง) —
																	</Select.Item>

																	<!-- Child Organization Units -->
																	{#each organizationUnits.filter((d) => d.parent_unit_id === parentDept.id) as childDept (childDept.id)}
																		<Select.Item value={childDept.id} class="pl-6">
																			<div class="flex flex-col">
																				<span>{childDept.name}</span>
																				{#if childDept.code}
																					<span class="text-[10px] text-muted-foreground">
																						{childDept.code}
																					</span>
																				{/if}
																			</div>
																		</Select.Item>
																	{/each}
																</Select.Group>
																<Select.Separator />
															{/each}

															<!-- Orphan Organization Units (No Parent) -->
															{#if organizationUnits.some((d) => d.parent_unit_id && !organizationUnits.find((p) => p.id === d.parent_unit_id))}
																<Select.Group>
																	<Select.Label class="font-bold text-muted-foreground"
																		>อื่นๆ</Select.Label
																	>
																	{#each organizationUnits.filter((d) => d.parent_unit_id && !organizationUnits.find((p) => p.id === d.parent_unit_id)) as orphan (orphan.id)}
																		<Select.Item value={orphan.id} class="pl-6">
																			{orphan.name}
																		</Select.Item>
																	{/each}
																</Select.Group>
															{/if}
														</Select.Content>
													</Select.Root>
												</div>

												<div>
													<Label class="mb-2">ตำแหน่งในหน่วยงาน</Label>
													<Select.Root type="single" bind:value={dept.position_code}>
														<Select.Trigger>{getPositionLabel(dept.position_code)}</Select.Trigger>
														<Select.Content>
															<Select.Item value="member">สมาชิก</Select.Item>
															<Select.Item value="head">หัวหน้า</Select.Item>
														</Select.Content>
													</Select.Root>
												</div>

												<div>
													<Label class="mb-2">หน้าที่รับผิดชอบ</Label>
													<Textarea
														bind:value={dept.responsibilities}
														placeholder="ระบุหน้าที่รับผิดชอบ..."
														rows={2}
													/>
												</div>

												<div>
													<div class="flex items-center gap-2 cursor-pointer">
														<Checkbox
															aria-label={`สังกัดหลักหน่วยงานที่ ${i + 1}`} checked={dept.is_primary}
															onCheckedChange={() => setPrimaryOrganizationUnit(i)}
														/>
														<span class="text-sm">{organizationPrimaryLabel(dept.organization_unit_id, organizationUnits)}</span>
													</div>
												</div>
											</div>
										</div>
									{/each}

									<Button
										type="button"
										onclick={addOrganizationUnit}
										variant="outline"
										class="w-full"
									>
										+ เพิ่มหน่วยงาน
									</Button>
								</div>
							{/if}
						{/if}
					</Card.Content></Card.Root
				>

				<div
					class="mt-4 flex flex-wrap items-center justify-between gap-3 rounded-xl border bg-card/95 p-3"
				>
					<p class="text-sm text-muted-foreground" role="status">
						{dirty ? 'มีข้อมูลที่ยังไม่บันทึก' : 'ยังไม่มีการเปลี่ยนแปลง'}
					</p>
					<div class="flex gap-2">
						<Button
							type="button"
							variant="outline"
							onclick={discardChanges}
							disabled={saving || uploadingLicense || !dirty}>ยกเลิกการแก้ไข</Button
						>
						<Button
							type="submit"
							disabled={saving || uploadingLicense || careerConflict || !canMutateStaff || !dirty}
							class="gap-2"
							>{#if saving}<LoaderCircle class="size-4 animate-spin" />กำลังบันทึก...{:else}<Save
									class="size-4"
								/>บันทึกการเปลี่ยนแปลง{/if}</Button
						>
					</div>
				</div>
			</form>
		{:else}
			<PageState
				title="ไม่พบข้อมูลบุคลากร"
				description="ไม่พบข้อมูลบุคลากรสำหรับรายการนี้"
				actionLabel="กลับหน้าจัดการบุคลากร"
				href="/staff/manage"
			/>
		{/if}
	</div>
</PageShell>
