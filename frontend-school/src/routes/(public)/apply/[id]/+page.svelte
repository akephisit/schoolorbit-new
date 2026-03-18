<script lang="ts">
	import { page } from '$app/state';
	import { onMount } from 'svelte';
	import {
		submitApplication,
		getPublicRoundInfo,
		updateApplication,
		portalGetStatus,
		portalUploadTempFile,
		portalDeleteDocument,
		DOC_TYPE_LABELS
	} from '$lib/api/admission';
	import type {
		AdmissionRound,
		AdmissionTrack,
		ApplicationDocument
	} from '$lib/api/admission';
	import { toast } from 'svelte-sonner';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Card from '$lib/components/ui/card';
	import * as Select from '$lib/components/ui/select';
	import { Separator } from '$lib/components/ui/separator';
	import DatePicker from '$lib/components/ui/date-picker/DatePicker.svelte';
	import {
		GraduationCap,
		CircleCheck,
		CircleAlert,
		ChevronRight,
		LoaderCircle,
		Camera,
		X,
		FileText,
		Copy,
		ZoomIn,
		ZoomOut
	} from 'lucide-svelte';
	import DocumentCropperModal from '$lib/components/DocumentCropperModal.svelte';

	let { data } = $props();

	let round = $state<AdmissionRound | null>(null);
	let tracks = $state<AdmissionTrack[]>([]);
	let loadError = $state('');
	let loading = $state(true);

	let submitting = $state(false);
	let successResult: { applicationNumber?: string; message?: string } | null = $state(null);

	let isEditMode = $state(false);
	let authNid = '';
	let authDob = '';

	// ===== ข้อมูลผู้สมัคร =====
	let trackId = $state('');
	let nationalId = $state('');
	let title = $state('');
	let firstName = $state('');
	let lastName = $state('');
	let gender = $state('');
	let dob = $state('');
	let phone = $state('');
	let email = $state('');
	let religion = $state('');
	let ethnicity = $state('');
	let nationality = $state('ไทย');

	// ===== ที่อยู่ตามทะเบียนบ้าน =====
	let homeHouseNo = $state('');
	let homeMoo = $state('');
	let homeSoi = $state('');
	let homeRoad = $state('');
	let addressLine = $state(''); // sub-address line (backward compat: home address_line)
	let subDistrict = $state('');
	let district = $state('');
	let province = $state('');
	let postalCode = $state('');
	let homePhone = $state('');

	// ===== ที่อยู่ปัจจุบัน =====
	let currentHouseNo = $state('');
	let currentMoo = $state('');
	let currentSoi = $state('');
	let currentRoad = $state('');
	let currentSubDistrict = $state('');
	let currentDistrict = $state('');
	let currentProvince = $state('');
	let currentPostalCode = $state('');
	let currentPhone = $state('');

	function copyHomeAddressToCurrent() {
		currentHouseNo = homeHouseNo;
		currentMoo = homeMoo;
		currentSoi = homeSoi;
		currentRoad = homeRoad;
		currentSubDistrict = subDistrict;
		currentDistrict = district;
		currentProvince = province;
		currentPostalCode = postalCode;
		currentPhone = homePhone;
		toast.success('คัดลอกที่อยู่ตามทะเบียนบ้านแล้ว');
	}

	// ===== โรงเรียนเดิม =====
	let previousSchool = $state('');
	let previousGrade = $state('');
	let previousStudyYear = $state('');
	let previousSchoolProvince = $state('');
	let previousGpa = $state('');

	// ===== ครอบครัว =====
	let parentStatus = $state<string[]>([]);
	let parentStatusOther = $state('');

	const PARENT_STATUS_OPTIONS = [
		'อยู่ร่วมกัน',
		'แยกกันอยู่',
		'หย่าร้าง',
		'บิดาเสียชีวิต',
		'มารดาเสียชีวิต',
		'อื่นๆ'
	];

	function toggleParentStatus(val: string) {
		if (parentStatus.includes(val)) {
			parentStatus = parentStatus.filter((s) => s !== val);
		} else {
			parentStatus = [...parentStatus, val];
		}
	}

	// บิดา
	let fatherTitle = $state('');
	let fatherFirstName = $state('');
	let fatherLastName = $state('');
	let fatherPhone = $state('');
	let fatherOccupation = $state('');
	let fatherNationalId = $state('');
	let fatherIncome = $state('');

	// มารดา
	let motherTitle = $state('');
	let motherFirstName = $state('');
	let motherLastName = $state('');
	let motherPhone = $state('');
	let motherOccupation = $state('');
	let motherNationalId = $state('');
	let motherIncome = $state('');

	// ผู้ปกครอง
	let guardianIs = $state<'father' | 'mother' | 'other'>('other');
	let guardianTitle = $state('');
	let guardianFirstName = $state('');
	let guardianLastName = $state('');
	let guardianPhone = $state('');
	let guardianRelation = $state('');
	let guardianNationalId = $state('');
	let guardianOccupation = $state('');
	let guardianIncome = $state('');

	// ===== เอกสาร =====
	type CropPoint = { x: number; y: number };
	type DocSlot = {
		tempFileId?: string;
		name?: string;
		size?: number;
		url?: string;
		preview?: string; // local blob URL for thumbnail display
		blob?: Blob; // pending blob — uploaded at submit time
		originalBlob?: Blob; // original uncropped file — for re-crop
		savedCorners?: [CropPoint, CropPoint, CropPoint, CropPoint]; // last crop positions
		uploading: boolean;
	};
	let uploadedDocs = $state<Record<string, DocSlot>>({});

	// In edit mode: existing linked documents
	let existingDocs = $state<ApplicationDocument[]>([]);
	let deletingDoc = $state<string | null>(null);

	// Crop modal state
	let cropTarget = $state<{ docType: string; imageUrl: string; initialCorners?: [CropPoint, CropPoint, CropPoint, CropPoint] } | null>(null);
	let cropperOpen = $state(false);
	// Lightbox
	let lightboxSrc = $state<string | null>(null);
	let lightboxLabel = $state('');
	let lbZoom = $state(1);
	let lbPan = $state({ x: 0, y: 0 });
	let lbDragging = $state(false);
	let lbDragStart = { mx: 0, my: 0, px: 0, py: 0 };

	function openLightbox(src: string, label: string) {
		lightboxSrc = src;
		lightboxLabel = label;
		lbZoom = 1;
		lbPan = { x: 0, y: 0 };
	}

	function closeLightbox() {
		lightboxSrc = null;
	}

	function onLbWheel(e: WheelEvent) {
		e.preventDefault();
		lbZoom = Math.max(0.5, Math.min(8, lbZoom + (e.deltaY > 0 ? -0.2 : 0.2)));
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
		if (lightboxSrc && e.key === 'Escape') closeLightbox();
	}
	// File input refs per docType
	let fileInputRefs = $state<Record<string, HTMLInputElement>>({});

	const DOC_TYPE_ORDER = Object.keys(DOC_TYPE_LABELS);

	function handleDocFileSelected(docType: string, e: Event) {
		const input = e.target as HTMLInputElement;
		const file = input.files?.[0];
		if (!file) return;
		input.value = '';
		// เก็บ original ไว้เผื่อ re-crop
		uploadedDocs[docType] = { ...(uploadedDocs[docType] ?? { uploading: false }), originalBlob: file };
		const imageUrl = URL.createObjectURL(file);
		cropTarget = { docType, imageUrl };
		cropperOpen = true;
	}

	function handleReCrop(docType: string) {
		const slot = uploadedDocs[docType];
		if (!slot?.originalBlob) return;
		const imageUrl = URL.createObjectURL(slot.originalBlob);
		cropTarget = { docType, imageUrl, initialCorners: slot.savedCorners };
		cropperOpen = true;
	}

	function handleCropComplete(blob: Blob, corners: [CropPoint, CropPoint, CropPoint, CropPoint]) {
		if (!cropTarget) return;
		const { docType, imageUrl } = cropTarget;
		cropperOpen = false;
		const prev = uploadedDocs[docType]?.preview;
		if (prev) URL.revokeObjectURL(prev);
		const preview = URL.createObjectURL(blob);
		URL.revokeObjectURL(imageUrl);
		cropTarget = null;

		// เก็บ blob + corners ไว้ก่อน — จะ upload ตอนกดส่งใบสมัคร (คง originalBlob ไว้)
		uploadedDocs[docType] = {
			...uploadedDocs[docType],
			blob,
			preview,
			name: `${docType}.jpg`,
			size: blob.size,
			uploading: false,
			savedCorners: corners
		};
	}

	function handleCropCancel() {
		if (cropTarget) {
			URL.revokeObjectURL(cropTarget.imageUrl);
			cropTarget = null;
		}
	}

	function handleDocRemoveNew(docType: string) {
		const slot = uploadedDocs[docType];
		if (slot?.preview) URL.revokeObjectURL(slot.preview);
		const copy = { ...uploadedDocs };
		delete copy[docType];
		uploadedDocs = copy;
	}

	async function handleDocDeleteExisting(docType: string) {
		if (!authNid || !authDob) return;
		deletingDoc = docType;
		try {
			await portalDeleteDocument(authNid, authDob, docType);
			existingDocs = existingDocs.filter((d) => d.docType !== docType);
			toast.success('ลบเอกสารแล้ว');
		} catch (err) {
			toast.error(err instanceof Error ? err.message : 'ลบไม่สำเร็จ');
		} finally {
			deletingDoc = null;
		}
	}

	function formatBytes(bytes: number) {
		if (bytes < 1024) return `${bytes} B`;
		if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
		return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
	}

	const parseName = (
		fullName: string,
		setTitle: (v: string) => void,
		setFirst: (v: string) => void,
		setLast: (v: string) => void
	) => {
		if (!fullName) return;
		let f = fullName.trim();
		const prefixes = ['นางสาว', 'นาง', 'นาย', 'ด.ช.', 'ด.ญ.'];
		let found = false;
		for (const p of prefixes) {
			if (f.startsWith(p)) {
				setTitle(p);
				f = f.substring(p.length).trim();
				found = true;
				break;
			}
		}
		if (!found) setTitle('');
		const parts = f.split(' ').filter(Boolean);
		setFirst(parts[0] || '');
		setLast(parts.slice(1).join(' '));
	};

	onMount(async () => {
		const id = page.params.id;
		isEditMode = page.url.searchParams.get('edit') === 'true';

		if (isEditMode) {
			authNid = sessionStorage.getItem('admissionEditNid') || '';
			authDob = sessionStorage.getItem('admissionEditDob') || '';
		}

		if (!id) {
			loadError = 'ไม่พบ ID ของรอบ';
			loading = false;
			return;
		}

		try {
			const info = await getPublicRoundInfo(id);
			round = info.round;
			tracks = info.tracks;

			if (isEditMode && authNid && authDob) {
				const statusData = (await portalGetStatus(authNid, authDob)) as any;
				if (statusData?.application) {
					const app = statusData.application;
					trackId = app.admissionTrackId || '';
					nationalId = app.nationalId || '';
					title = app.title || '';
					firstName = app.firstName || '';
					lastName = app.lastName || '';
					gender = app.gender || '';
					if (app.dateOfBirth?.length === 10) dob = app.dateOfBirth;
					phone = app.phone || '';
					email = app.email || '';
					religion = app.religion || '';
					ethnicity = app.ethnicity || '';
					nationality = app.nationality || 'ไทย';

					// Home address
					homeHouseNo = app.homeHouseNo || '';
					homeMoo = app.homeMoo || '';
					homeSoi = app.homeSoi || '';
					homeRoad = app.homeRoad || '';
					addressLine = app.addressLine || '';
					subDistrict = app.subDistrict || '';
					district = app.district || '';
					province = app.province || '';
					postalCode = app.postalCode || '';
					homePhone = app.homePhone || '';

					// Current address
					currentHouseNo = app.currentHouseNo || '';
					currentMoo = app.currentMoo || '';
					currentSoi = app.currentSoi || '';
					currentRoad = app.currentRoad || '';
					currentSubDistrict = app.currentSubDistrict || '';
					currentDistrict = app.currentDistrict || '';
					currentProvince = app.currentProvince || '';
					currentPostalCode = app.currentPostalCode || '';
					currentPhone = app.currentPhone || '';

					// Previous school
					previousSchool = app.previousSchool || '';
					previousGrade = app.previousGrade || '';
					previousStudyYear = app.previousStudyYear || '';
					previousSchoolProvince = app.previousSchoolProvince || '';
					previousGpa = app.previousGpa ? app.previousGpa.toString() : '';

					// Family
					parentStatus = Array.isArray(app.parentStatus) ? app.parentStatus : [];
					parentStatusOther = app.parentStatusOther || '';
					guardianIs = app.guardianIs || 'other';

					parseName(
						app.fatherName || '',
						(v) => (fatherTitle = v),
						(v) => (fatherFirstName = v),
						(v) => (fatherLastName = v)
					);
					fatherPhone = app.fatherPhone || '';
					fatherOccupation = app.fatherOccupation || '';
					fatherNationalId = app.fatherNationalId || '';
					fatherIncome = app.fatherIncome ? app.fatherIncome.toString() : '';

					parseName(
						app.motherName || '',
						(v) => (motherTitle = v),
						(v) => (motherFirstName = v),
						(v) => (motherLastName = v)
					);
					motherPhone = app.motherPhone || '';
					motherOccupation = app.motherOccupation || '';
					motherNationalId = app.motherNationalId || '';
					motherIncome = app.motherIncome ? app.motherIncome.toString() : '';

					parseName(
						app.guardianName || '',
						(v) => (guardianTitle = v),
						(v) => (guardianFirstName = v),
						(v) => (guardianLastName = v)
					);
					guardianPhone = app.guardianPhone || '';
					guardianRelation = app.guardianRelation || '';
					guardianNationalId = app.guardianNationalId || '';
					guardianOccupation = app.guardianOccupation || '';
					guardianIncome = app.guardianIncome ? app.guardianIncome.toString() : '';
				}
				// Load existing documents
				if (statusData?.documents) {
					existingDocs = statusData.documents;
				}
			}
		} catch (e) {
			loadError = e instanceof Error ? e.message : 'ไม่สามารถโหลดข้อมูลรอบรับสมัคร';
		} finally {
			loading = false;
		}
	});

	function buildGuardianName() {
		if (guardianIs === 'father') {
			return fatherFirstName
				? `${fatherTitle}${fatherFirstName} ${fatherLastName}`.trim()
				: undefined;
		}
		if (guardianIs === 'mother') {
			return motherFirstName
				? `${motherTitle}${motherFirstName} ${motherLastName}`.trim()
				: undefined;
		}
		return guardianFirstName
			? `${guardianTitle}${guardianFirstName} ${guardianLastName}`.trim()
			: undefined;
	}

	async function handleSubmit(e: Event) {
		e.preventDefault();

		if (!trackId) {
			toast.error('กรุณาเลือกสายการเรียน');
			return;
		}
		if (nationalId.length !== 13) {
			toast.error('กรุณากรอกเลขบัตรประชาชน 13 หลัก');
			return;
		}

		submitting = true;
		try {
			// Concat addressLine for backward compat with enrollment handler
			const homeAddressLine = [homeHouseNo, homeMoo ? `หมู่ ${homeMoo}` : '', homeSoi ? `ซ.${homeSoi}` : '', homeRoad ? `ถ.${homeRoad}` : '']
				.filter(Boolean)
				.join(' ')
				.trim() || addressLine;

			const guardianName = buildGuardianName();
			const guardianPhoneVal = guardianIs === 'father' ? fatherPhone : guardianIs === 'mother' ? motherPhone : guardianPhone;
			const guardianNationalIdVal = guardianIs === 'father' ? fatherNationalId : guardianIs === 'mother' ? motherNationalId : guardianNationalId;
			const guardianOccupationVal = guardianIs === 'father' ? fatherOccupation : guardianIs === 'mother' ? motherOccupation : guardianOccupation;
			const guardianIncomeVal = guardianIs === 'father' ? (fatherIncome ? parseFloat(fatherIncome) : undefined) : guardianIs === 'mother' ? (motherIncome ? parseFloat(motherIncome) : undefined) : (guardianIncome ? parseFloat(guardianIncome) : undefined);

			// Upload pending blobs ก่อน submit
			const pendingEntries = Object.entries(uploadedDocs).filter(([, d]) => d.blob);
			if (pendingEntries.length > 0) {
				toast.info(`กำลังอัปโหลดเอกสาร ${pendingEntries.length} ไฟล์...`);
			}
			const uploadedRefs: { tempFileId: string; docType: string }[] = [];
			for (const [docType, slot] of pendingEntries) {
				uploadedDocs[docType] = { ...slot, uploading: true };
				const file = new File([slot.blob!], `${docType}.jpg`, { type: 'image/jpeg' });
				const res = await portalUploadTempFile(file, docType);
				uploadedDocs[docType] = { ...slot, uploading: false, tempFileId: res.tempFileId, blob: undefined };
				uploadedRefs.push({ tempFileId: res.tempFileId, docType });
			}

			// เพิ่ม tempFileIds ที่มีอยู่แล้ว (edit mode หรือ upload ก่อนหน้า)
			const existingRefs = Object.entries(uploadedDocs)
				.filter(([, d]) => d.tempFileId && !d.blob)
				.map(([docType, d]) => ({ tempFileId: d.tempFileId!, docType }));

			const payload = {
				admissionTrackId: trackId,
				nationalId,
				title,
				firstName,
				lastName,
				gender,
				dateOfBirth: dob || undefined,
				phone,
				email,
				religion: religion || undefined,
				ethnicity: ethnicity || undefined,
				nationality: nationality || undefined,
				// Home address
				addressLine: homeAddressLine || undefined,
				homeHouseNo: homeHouseNo || undefined,
				homeMoo: homeMoo || undefined,
				homeSoi: homeSoi || undefined,
				homeRoad: homeRoad || undefined,
				subDistrict: subDistrict || undefined,
				district: district || undefined,
				province: province || undefined,
				postalCode: postalCode || undefined,
				homePhone: homePhone || undefined,
				// Current address
				currentHouseNo: currentHouseNo || undefined,
				currentMoo: currentMoo || undefined,
				currentSoi: currentSoi || undefined,
				currentRoad: currentRoad || undefined,
				currentSubDistrict: currentSubDistrict || undefined,
				currentDistrict: currentDistrict || undefined,
				currentProvince: currentProvince || undefined,
				currentPostalCode: currentPostalCode || undefined,
				currentPhone: currentPhone || undefined,
				// Previous school
				previousSchool: previousSchool || undefined,
				previousGrade: previousGrade || undefined,
				previousGpa: previousGpa ? parseFloat(previousGpa) : undefined,
				previousStudyYear: previousStudyYear || undefined,
				previousSchoolProvince: previousSchoolProvince || undefined,
				// Father
				fatherName: fatherFirstName ? `${fatherTitle}${fatherFirstName} ${fatherLastName}`.trim() : undefined,
				fatherPhone: fatherPhone || undefined,
				fatherOccupation: fatherOccupation || undefined,
				fatherNationalId: fatherNationalId || undefined,
				fatherIncome: fatherIncome ? parseFloat(fatherIncome) : undefined,
				// Mother
				motherName: motherFirstName ? `${motherTitle}${motherFirstName} ${motherLastName}`.trim() : undefined,
				motherPhone: motherPhone || undefined,
				motherOccupation: motherOccupation || undefined,
				motherNationalId: motherNationalId || undefined,
				motherIncome: motherIncome ? parseFloat(motherIncome) : undefined,
				// Guardian
				guardianName,
				guardianPhone: guardianPhoneVal || undefined,
				guardianRelation: guardianIs === 'other' ? (guardianRelation || undefined) : undefined,
				guardianNationalId: guardianNationalIdVal || undefined,
				guardianOccupation: guardianOccupationVal || undefined,
				guardianIncome: guardianIncomeVal,
				guardianIs,
				// Family status
				parentStatus: parentStatus.length > 0 ? parentStatus : undefined,
				parentStatusOther: parentStatus.includes('อื่นๆ') ? (parentStatusOther || undefined) : undefined,
				// Documents
				documents: [...uploadedRefs, ...existingRefs],
			};

			let res;
			if (isEditMode) {
				await updateApplication(authNid, authDob, payload);
				res = { message: 'อัปเดตใบสมัครเรียบร้อยแล้ว' };
			} else {
				res = await submitApplication(page.params.id!, payload);
			}

			successResult = res;
			window.scrollTo({ top: 0, behavior: 'smooth' });
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'เกิดข้อผิดพลาด กรุณาลองใหม่');
		} finally {
			submitting = false;
		}
	}

	// Helper: title options
	const STUDENT_TITLES = ['ด.ช.', 'ด.ญ.', 'นาย', 'นางสาว'];
	const MALE_TITLES = ['นาย', 'ม.ร.ว.', 'ม.ล.', 'ดร.'];
	const FEMALE_TITLES = ['นาง', 'นางสาว', 'ม.ร.ว.', 'ม.ล.', 'ดร.'];
	const GUARDIAN_TITLES = ['นาย', 'นาง', 'นางสาว', 'ปู่', 'ย่า', 'ตา', 'ยาย', 'ดร.'];
</script>

<svelte:window onkeydown={onLbKeyDown} />

<svelte:head>
	<title>{round ? `สมัครเรียน – ${round.name}` : 'สมัครเรียน'} | SchoolOrbit</title>
</svelte:head>

<div class="min-h-screen bg-gradient-to-b from-blue-50 to-white py-10 px-4">
	<div class="max-w-3xl mx-auto space-y-6">
		<!-- Loading -->
		{#if loading}
			<div class="flex flex-col items-center justify-center py-24 gap-4 text-muted-foreground">
				<LoaderCircle class="w-10 h-10 animate-spin text-blue-500" />
				<p>กำลังโหลดข้อมูลรอบรับสมัคร...</p>
			</div>

		{:else if loadError}
			<div class="flex flex-col items-center justify-center py-24 gap-4 text-center">
				<CircleAlert class="w-12 h-12 text-red-400" />
				<p class="text-gray-700 font-medium">{loadError}</p>
				<Button onclick={() => window.location.reload()} variant="outline">ลองใหม่</Button>
			</div>
		{:else}
			<!-- Header -->
			<div class="text-center space-y-2 pb-2">
				<div class="inline-flex p-3 bg-white rounded-2xl shadow border border-blue-100 mb-2">
					<GraduationCap class="w-10 h-10 text-blue-600" />
				</div>
				<h1 class="text-3xl font-bold text-gray-900">
					{isEditMode ? 'แก้ไขใบสมัครเรียน' : 'ใบสมัครเรียน'}
				</h1>
				{#if round}
					<p class="text-lg font-medium text-blue-700">{round.name}</p>
					<p class="text-sm text-gray-500">
						ระดับชั้น {round.gradeLevelName} | ปีการศึกษา {round.academicYearName}
					</p>
					<p class="text-xs text-gray-400 mt-1">
						รับสมัคร {new Date(round.applyStartDate).toLocaleDateString('th-TH', { year: 'numeric', month: 'short', day: 'numeric' })}
						– {new Date(round.applyEndDate).toLocaleDateString('th-TH', { year: 'numeric', month: 'short', day: 'numeric' })}
					</p>
				{/if}
			</div>

			<!-- Success -->
			{#if successResult}
				<Card.Root class="border-green-200 shadow-lg">
					<Card.Content class="pt-8 pb-8 text-center space-y-5">
						<CircleCheck class="w-20 h-20 text-green-500 mx-auto" />
						<div>
							<h2 class="text-2xl font-bold text-green-800 mb-1">
								{isEditMode ? 'อัปเดตใบสมัครสำเร็จ!' : 'ส่งใบสมัครสำเร็จ!'}
							</h2>
							<p class="text-gray-600">ได้รับข้อมูลใบสมัครของท่านเรียบร้อยแล้ว</p>
						</div>
						<div class="flex items-start gap-3 max-w-sm mx-auto bg-amber-50 border border-amber-200 text-amber-800 rounded-lg p-4 text-sm text-left">
							<CircleAlert class="w-5 h-5 shrink-0 mt-0.5" />
							{#if !isEditMode}
								<p>ระบบใช้ <strong>เลขบัตรประชาชน</strong> และ <strong>วัน/เดือน/ปีเกิด (พ.ศ.)</strong> ของผู้สมัครในการตรวจสอบสถานะในภายหลัง</p>
							{:else}
								<p>ข้อมูลใบสมัครได้รับการแก้ไขและอัปเดตเรียบร้อยแล้ว กรุณารอการตรวจสอบจากฝั่งเจ้าหน้าที่อีกครั้ง</p>
							{/if}
						</div>
						<Button href="/apply" class="gap-2 mt-2">
							<ChevronRight class="w-4 h-4" /> ไปหน้าตรวจสอบผลการสมัคร
						</Button>
					</Card.Content>
				</Card.Root>

			{:else}
				<form onsubmit={handleSubmit} novalidate class="space-y-5">

					<!-- ===== Card 1: เลือกสายการเรียน ===== -->
					<Card.Root class="shadow-sm">
						<Card.Header class="pb-2">
							<Card.Title class="text-base">
								1. เลือกสายการเรียน <span class="text-red-500">*</span>
							</Card.Title>
						</Card.Header>
						<Card.Content>
							{#if tracks.length === 0}
								<p class="text-sm text-muted-foreground py-4 text-center">ไม่มีสายการเรียนที่เปิดรับในรอบนี้</p>
							{:else}
								<div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
									{#each tracks as t}
										<button
											type="button"
											onclick={() => (trackId = t.id)}
											class="text-left border-2 rounded-xl p-4 transition-all
											{trackId === t.id ? 'border-primary bg-primary/5' : 'border-border hover:border-primary/40 bg-card'}"
										>
											<div class="flex items-center gap-2">
												<div class="w-4 h-4 rounded-full border-2 flex items-center justify-center shrink-0
													{trackId === t.id ? 'border-primary' : 'border-muted-foreground'}">
													{#if trackId === t.id}
														<div class="w-2 h-2 rounded-full bg-primary"></div>
													{/if}
												</div>
												<p class="font-semibold text-card-foreground">{t.name}</p>
											</div>
											{#if t.studyPlanName}
												<p class="text-xs text-muted-foreground mt-1 ml-6">{t.studyPlanName}</p>
											{/if}
										</button>
									{/each}
								</div>
							{/if}
						</Card.Content>
					</Card.Root>

					<!-- ===== Card 2: ข้อมูลผู้สมัคร ===== -->
					<Card.Root class="shadow-sm">
						<Card.Header class="pb-2">
							<Card.Title class="text-base">2. ข้อมูลผู้สมัคร</Card.Title>
						</Card.Header>
						<Card.Content class="space-y-4">
							<div class="space-y-2">
								<Label for="nationalId">เลขประจำตัวประชาชน <span class="text-red-500">*</span></Label>
								<Input
									id="nationalId"
									bind:value={nationalId}
									maxlength={13}
									required
									placeholder="X-XXXX-XXXXX-XX-X"
									class="font-mono text-lg max-w-sm"
								/>
							</div>

							<div class="grid grid-cols-12 gap-3">
								<div class="col-span-12 sm:col-span-3 space-y-2">
									<Label>คำนำหน้า <span class="text-red-500">*</span></Label>
									<Select.Root type="single" bind:value={title} required>
										<Select.Trigger>{title || '-- เลือก --'}</Select.Trigger>
										<Select.Content>
											{#each STUDENT_TITLES as t}
												<Select.Item value={t}>{t}</Select.Item>
											{/each}
										</Select.Content>
									</Select.Root>
								</div>
								<div class="col-span-12 sm:col-span-5 space-y-2">
									<Label for="firstName">ชื่อจริง <span class="text-red-500">*</span></Label>
									<Input id="firstName" bind:value={firstName} required />
								</div>
								<div class="col-span-12 sm:col-span-4 space-y-2">
									<Label for="lastName">นามสกุล <span class="text-red-500">*</span></Label>
									<Input id="lastName" bind:value={lastName} required />
								</div>
							</div>

							<div class="grid grid-cols-1 md:grid-cols-3 gap-3">
								<div class="space-y-2">
									<Label>เพศ</Label>
									<Select.Root type="single" bind:value={gender}>
										<Select.Trigger>{gender === 'Male' ? 'ชาย' : gender === 'Female' ? 'หญิง' : '-- เลือก --'}</Select.Trigger>
										<Select.Content>
											<Select.Item value="Male">ชาย</Select.Item>
											<Select.Item value="Female">หญิง</Select.Item>
										</Select.Content>
									</Select.Root>
								</div>
								<div class="space-y-2">
									<Label>วันเกิด (ปฏิทินไทย)</Label>
									<DatePicker bind:value={dob} />
								</div>
								<div class="space-y-2">
									<Label for="phone">เบอร์โทร</Label>
									<Input id="phone" type="tel" bind:value={phone} placeholder="08XXXXXXXX" />
								</div>
							</div>

							<div class="space-y-2">
								<Label for="email">อีเมล</Label>
								<Input id="email" type="email" bind:value={email} placeholder="example@email.com" />
							</div>

							<Separator />
							<p class="text-sm font-semibold text-muted-foreground">ข้อมูลเพิ่มเติม</p>

							<div class="grid grid-cols-1 sm:grid-cols-3 gap-3">
								<div class="space-y-2">
									<Label for="religion">ศาสนา</Label>
									<Input id="religion" bind:value={religion} placeholder="พุทธ, คริสต์, อิสลาม..." />
								</div>
								<div class="space-y-2">
									<Label for="ethnicity">เชื้อชาติ</Label>
									<Input id="ethnicity" bind:value={ethnicity} placeholder="ไทย..." />
								</div>
								<div class="space-y-2">
									<Label for="nationality">สัญชาติ</Label>
									<Input id="nationality" bind:value={nationality} placeholder="ไทย..." />
								</div>
							</div>
						</Card.Content>
					</Card.Root>

					<!-- ===== Card 3: ที่อยู่ ===== -->
					<Card.Root class="shadow-sm">
						<Card.Header class="pb-2">
							<Card.Title class="text-base">3. ที่อยู่</Card.Title>
						</Card.Header>
						<Card.Content class="space-y-5">
							<!-- ที่อยู่ตามทะเบียนบ้าน -->
							<p class="text-sm font-semibold">ที่อยู่ตามทะเบียนบ้าน</p>
							<div class="grid grid-cols-2 sm:grid-cols-4 gap-3">
								<div class="space-y-2">
									<Label for="homeHouseNo">บ้านเลขที่</Label>
									<Input id="homeHouseNo" bind:value={homeHouseNo} placeholder="123/4" />
								</div>
								<div class="space-y-2">
									<Label for="homeMoo">หมู่ที่</Label>
									<Input id="homeMoo" bind:value={homeMoo} placeholder="5" />
								</div>
								<div class="space-y-2">
									<Label for="homeSoi">ซอย</Label>
									<Input id="homeSoi" bind:value={homeSoi} />
								</div>
								<div class="space-y-2">
									<Label for="homeRoad">ถนน</Label>
									<Input id="homeRoad" bind:value={homeRoad} />
								</div>
							</div>
							<div class="grid grid-cols-2 sm:grid-cols-4 gap-3">
								<div class="col-span-2 space-y-2">
									<Label for="subDistrict">ตำบล/แขวง</Label>
									<Input id="subDistrict" bind:value={subDistrict} />
								</div>
								<div class="space-y-2">
									<Label for="district">อำเภอ/เขต</Label>
									<Input id="district" bind:value={district} />
								</div>
								<div class="space-y-2">
									<Label for="province">จังหวัด</Label>
									<Input id="province" bind:value={province} />
								</div>
								<div class="space-y-2">
									<Label for="postalCode">รหัสไปรษณีย์</Label>
									<Input id="postalCode" bind:value={postalCode} maxlength={5} />
								</div>
								<div class="space-y-2">
									<Label for="homePhone">โทรศัพท์</Label>
									<Input id="homePhone" type="tel" bind:value={homePhone} placeholder="02XXXXXXX" />
								</div>
							</div>

							<Separator />

							<!-- ที่อยู่ปัจจุบัน -->
							<div class="flex items-center justify-between">
								<p class="text-sm font-semibold">ที่อยู่ปัจจุบัน</p>
								<Button
									type="button"
									variant="outline"
									size="sm"
									onclick={copyHomeAddressToCurrent}
									class="gap-1.5 text-xs"
								>
									<Copy class="w-3 h-3" />
									คัดลอกจากที่อยู่ตามทะเบียนบ้าน
								</Button>
							</div>
							<div class="grid grid-cols-2 sm:grid-cols-4 gap-3">
								<div class="space-y-2">
									<Label for="currentHouseNo">บ้านเลขที่</Label>
									<Input id="currentHouseNo" bind:value={currentHouseNo} placeholder="123/4" />
								</div>
								<div class="space-y-2">
									<Label for="currentMoo">หมู่ที่</Label>
									<Input id="currentMoo" bind:value={currentMoo} placeholder="5" />
								</div>
								<div class="space-y-2">
									<Label for="currentSoi">ซอย</Label>
									<Input id="currentSoi" bind:value={currentSoi} />
								</div>
								<div class="space-y-2">
									<Label for="currentRoad">ถนน</Label>
									<Input id="currentRoad" bind:value={currentRoad} />
								</div>
							</div>
							<div class="grid grid-cols-2 sm:grid-cols-4 gap-3">
								<div class="col-span-2 space-y-2">
									<Label for="currentSubDistrict">ตำบล/แขวง</Label>
									<Input id="currentSubDistrict" bind:value={currentSubDistrict} />
								</div>
								<div class="space-y-2">
									<Label for="currentDistrict">อำเภอ/เขต</Label>
									<Input id="currentDistrict" bind:value={currentDistrict} />
								</div>
								<div class="space-y-2">
									<Label for="currentProvince">จังหวัด</Label>
									<Input id="currentProvince" bind:value={currentProvince} />
								</div>
								<div class="space-y-2">
									<Label for="currentPostalCode">รหัสไปรษณีย์</Label>
									<Input id="currentPostalCode" bind:value={currentPostalCode} maxlength={5} />
								</div>
								<div class="space-y-2">
									<Label for="currentPhone">โทรศัพท์</Label>
									<Input id="currentPhone" type="tel" bind:value={currentPhone} placeholder="02XXXXXXX" />
								</div>
							</div>
						</Card.Content>
					</Card.Root>

					<!-- ===== Card 4: การศึกษาเดิม ===== -->
					<Card.Root class="shadow-sm">
						<Card.Header class="pb-2">
							<Card.Title class="text-base">4. ข้อมูลการศึกษาเดิม</Card.Title>
						</Card.Header>
						<Card.Content class="space-y-4">
							<p class="text-sm text-muted-foreground">สำเร็จการศึกษา / กำลังศึกษาชั้น:</p>
							<div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
								<div class="space-y-2">
									<Label>ระดับชั้น</Label>
									<Select.Root type="single" bind:value={previousGrade}>
										<Select.Trigger class="w-full">{previousGrade || '-- เลือกระดับชั้น --'}</Select.Trigger>
										<Select.Content>
											<Select.Item value="อนุบาล 3">อนุบาล 3</Select.Item>
											<Select.Item value="ประถมศึกษาปีที่ 6">ประถมศึกษาปีที่ 6</Select.Item>
											<Select.Item value="มัธยมศึกษาปีที่ 3">มัธยมศึกษาปีที่ 3</Select.Item>
											<Select.Item value="เทียบเท่า">เทียบเท่า / อื่นๆ</Select.Item>
										</Select.Content>
									</Select.Root>
								</div>
								<div class="space-y-2">
									<Label for="previousStudyYear">ศึกษาปีที่ (ระบุปีที่กำลังศึกษา)</Label>
									<Input id="previousStudyYear" bind:value={previousStudyYear} placeholder="เช่น 6" />
								</div>
							</div>
							<div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
								<div class="space-y-2">
									<Label for="previousSchool">ชื่อโรงเรียนเดิม</Label>
									<Input id="previousSchool" bind:value={previousSchool} placeholder="โรงเรียน..." />
								</div>
								<div class="space-y-2">
									<Label for="previousSchoolProvince">จังหวัด</Label>
									<Input id="previousSchoolProvince" bind:value={previousSchoolProvince} placeholder="จังหวัด..." />
								</div>
							</div>
							<div class="max-w-xs space-y-2">
								<Label for="prevGpa">ผลการเรียนเฉลี่ยสะสม (GPA)</Label>
								<Input
									id="prevGpa"
									type="number"
									bind:value={previousGpa}
									step="0.01"
									min="0"
									max="4"
									placeholder="0.00 – 4.00"
								/>
							</div>
						</Card.Content>
					</Card.Root>

					<!-- ===== Card 5: ครอบครัว ===== -->
					<Card.Root class="shadow-sm">
						<Card.Header class="pb-2">
							<Card.Title class="text-base">5. ข้อมูลครอบครัว</Card.Title>
						</Card.Header>
						<Card.Content class="space-y-6">

							<!-- สถานภาพบิดามารดา -->
							<div class="space-y-3">
								<p class="text-sm font-semibold">สถานภาพบิดามารดา <span class="font-normal text-muted-foreground">(เลือกได้มากกว่า 1)</span></p>
								<div class="flex flex-wrap gap-2">
									{#each PARENT_STATUS_OPTIONS as opt}
										<button
											type="button"
											onclick={() => toggleParentStatus(opt)}
											class="px-3 py-1.5 rounded-full text-sm border transition-all
											{parentStatus.includes(opt)
												? 'bg-primary text-primary-foreground border-primary'
												: 'bg-background border-border hover:border-primary/50'}"
										>
											{opt}
										</button>
									{/each}
								</div>
								{#if parentStatus.includes('อื่นๆ')}
									<Input
										bind:value={parentStatusOther}
										placeholder="ระบุ..."
										class="max-w-xs"
									/>
								{/if}
							</div>

							<Separator />

							<!-- บิดา -->
							<div class="space-y-3">
								<p class="text-xs font-semibold uppercase tracking-wide text-muted-foreground">บิดา</p>
								<div class="grid grid-cols-12 gap-3">
									<div class="col-span-12 sm:col-span-3 space-y-2">
										<Label>คำนำหน้า</Label>
										<Select.Root type="single" bind:value={fatherTitle}>
											<Select.Trigger>{fatherTitle || '-- เลือก --'}</Select.Trigger>
											<Select.Content>
												{#each MALE_TITLES as t}<Select.Item value={t}>{t}</Select.Item>{/each}
											</Select.Content>
										</Select.Root>
									</div>
									<div class="col-span-12 sm:col-span-5 space-y-2">
										<Label for="fatherFirstName">ชื่อ</Label>
										<Input id="fatherFirstName" bind:value={fatherFirstName} />
									</div>
									<div class="col-span-12 sm:col-span-4 space-y-2">
										<Label for="fatherLastName">นามสกุล</Label>
										<Input id="fatherLastName" bind:value={fatherLastName} />
									</div>
								</div>
								<div class="grid grid-cols-2 sm:grid-cols-4 gap-3">
									<div class="space-y-2">
										<Label for="fatherNationalId">เลขประชาชน</Label>
										<Input id="fatherNationalId" bind:value={fatherNationalId} maxlength={13} class="font-mono" />
									</div>
									<div class="space-y-2">
										<Label for="fatherPhone">โทรศัพท์</Label>
										<Input id="fatherPhone" bind:value={fatherPhone} type="tel" />
									</div>
									<div class="space-y-2">
										<Label for="fatherOccupation">อาชีพ</Label>
										<Input id="fatherOccupation" bind:value={fatherOccupation} />
									</div>
									<div class="space-y-2">
										<Label for="fatherIncome">รายได้ต่อเดือน (บาท)</Label>
										<Input id="fatherIncome" type="number" bind:value={fatherIncome} min="0" placeholder="0" />
									</div>
								</div>
							</div>

							<Separator />

							<!-- มารดา -->
							<div class="space-y-3">
								<p class="text-xs font-semibold uppercase tracking-wide text-muted-foreground">มารดา</p>
								<div class="grid grid-cols-12 gap-3">
									<div class="col-span-12 sm:col-span-3 space-y-2">
										<Label>คำนำหน้า</Label>
										<Select.Root type="single" bind:value={motherTitle}>
											<Select.Trigger>{motherTitle || '-- เลือก --'}</Select.Trigger>
											<Select.Content>
												{#each FEMALE_TITLES as t}<Select.Item value={t}>{t}</Select.Item>{/each}
											</Select.Content>
										</Select.Root>
									</div>
									<div class="col-span-12 sm:col-span-5 space-y-2">
										<Label for="motherFirstName">ชื่อ</Label>
										<Input id="motherFirstName" bind:value={motherFirstName} />
									</div>
									<div class="col-span-12 sm:col-span-4 space-y-2">
										<Label for="motherLastName">นามสกุล</Label>
										<Input id="motherLastName" bind:value={motherLastName} />
									</div>
								</div>
								<div class="grid grid-cols-2 sm:grid-cols-4 gap-3">
									<div class="space-y-2">
										<Label for="motherNationalId">เลขประชาชน</Label>
										<Input id="motherNationalId" bind:value={motherNationalId} maxlength={13} class="font-mono" />
									</div>
									<div class="space-y-2">
										<Label for="motherPhone">โทรศัพท์</Label>
										<Input id="motherPhone" bind:value={motherPhone} type="tel" />
									</div>
									<div class="space-y-2">
										<Label for="motherOccupation">อาชีพ</Label>
										<Input id="motherOccupation" bind:value={motherOccupation} />
									</div>
									<div class="space-y-2">
										<Label for="motherIncome">รายได้ต่อเดือน (บาท)</Label>
										<Input id="motherIncome" type="number" bind:value={motherIncome} min="0" placeholder="0" />
									</div>
								</div>
							</div>

							<Separator />

							<!-- ผู้ปกครอง -->
							<div class="space-y-3">
								<p class="text-xs font-semibold uppercase tracking-wide text-muted-foreground">ผู้ปกครอง</p>
								<div class="flex flex-wrap gap-2">
									{#each [['father', 'บิดา'], ['mother', 'มารดา'], ['other', 'บุคคลอื่น']] as [val, label]}
										<button
											type="button"
											onclick={() => (guardianIs = val as 'father' | 'mother' | 'other')}
											class="px-4 py-2 rounded-full text-sm border transition-all
											{guardianIs === val
												? 'bg-primary text-primary-foreground border-primary'
												: 'bg-background border-border hover:border-primary/50'}"
										>
											{label}
										</button>
									{/each}
								</div>

								{#if guardianIs === 'father'}
									<p class="text-sm text-muted-foreground bg-muted/40 rounded-lg px-4 py-2">
										ใช้ข้อมูลบิดาที่กรอกไว้ข้างต้น
									</p>
								{:else if guardianIs === 'mother'}
									<p class="text-sm text-muted-foreground bg-muted/40 rounded-lg px-4 py-2">
										ใช้ข้อมูลมารดาที่กรอกไว้ข้างต้น
									</p>
								{:else}
									<div class="grid grid-cols-12 gap-3">
										<div class="col-span-12 sm:col-span-3 space-y-2">
											<Label>คำนำหน้า <span class="text-red-500">*</span></Label>
											<Select.Root type="single" bind:value={guardianTitle} required>
												<Select.Trigger>{guardianTitle || '-- เลือก --'}</Select.Trigger>
												<Select.Content>
													{#each GUARDIAN_TITLES as t}<Select.Item value={t}>{t}</Select.Item>{/each}
												</Select.Content>
											</Select.Root>
										</div>
										<div class="col-span-12 sm:col-span-5 space-y-2">
											<Label for="guardianFirstName">ชื่อ <span class="text-red-500">*</span></Label>
											<Input id="guardianFirstName" bind:value={guardianFirstName} required />
										</div>
										<div class="col-span-12 sm:col-span-4 space-y-2">
											<Label for="guardianLastName">นามสกุล <span class="text-red-500">*</span></Label>
											<Input id="guardianLastName" bind:value={guardianLastName} required />
										</div>
									</div>
									<div class="grid grid-cols-2 sm:grid-cols-4 gap-3">
										<div class="space-y-2">
											<Label for="guardianRelation">ความสัมพันธ์</Label>
											<Input id="guardianRelation" bind:value={guardianRelation} placeholder="เช่น ปู่, ย่า, ลุง" />
										</div>
										<div class="space-y-2">
											<Label for="guardianNationalId">เลขประชาชน</Label>
											<Input id="guardianNationalId" bind:value={guardianNationalId} maxlength={13} class="font-mono" />
										</div>
										<div class="space-y-2">
											<Label for="guardianPhone">โทรศัพท์</Label>
											<Input id="guardianPhone" bind:value={guardianPhone} type="tel" />
										</div>
										<div class="space-y-2">
											<Label for="guardianOccupation">อาชีพ</Label>
											<Input id="guardianOccupation" bind:value={guardianOccupation} />
										</div>
										<div class="space-y-2">
											<Label for="guardianIncome">รายได้ต่อเดือน (บาท)</Label>
											<Input id="guardianIncome" type="number" bind:value={guardianIncome} min="0" placeholder="0" />
										</div>
									</div>
								{/if}
							</div>
						</Card.Content>
					</Card.Root>

					<!-- ===== Card 6: เอกสารประกอบ ===== -->
					<Card.Root class="shadow-sm">
						<Card.Header class="pb-2">
							<Card.Title class="text-base">6. เอกสารประกอบการสมัคร</Card.Title>
							<p class="text-xs text-muted-foreground mt-1">ถ่ายรูปหรือเลือกรูปจากคลัง — สามารถ crop และปรับมุมเอียงได้</p>
						</Card.Header>
						<Card.Content class="space-y-2">
							{#each DOC_TYPE_ORDER as docType}
								{@const info = DOC_TYPE_LABELS[docType]}
								{@const existing = existingDocs.find((d) => d.docType === docType)}
								{@const newDoc = uploadedDocs[docType]}
								{@const hasFile = existing || (newDoc && (newDoc.blob || newDoc.tempFileId))}
								{@const thumbSrc = newDoc?.preview ?? existing?.fileUrl}

								<!-- Hidden file input -->
								<input
									type="file"
									class="hidden"
									accept="image/*"
									bind:this={fileInputRefs[docType]}
									onchange={(e) => handleDocFileSelected(docType, e)}
								/>

								<div class="flex items-center gap-3 p-3 rounded-lg border bg-card hover:bg-muted/20 transition-colors">
									<!-- Thumbnail or icon -->
									{#if thumbSrc}
										<button
											type="button"
											class="w-12 h-12 rounded-md overflow-hidden border bg-gray-100 shrink-0 cursor-zoom-in hover:ring-2 hover:ring-blue-400 transition-all"
											onclick={() => openLightbox(thumbSrc!, info.label)}
											title="กดดูรูป"
										>
											<img src={thumbSrc} alt={info.label} class="w-full h-full object-cover pointer-events-none" />
										</button>
									{:else}
										<FileText class="w-5 h-5 text-muted-foreground shrink-0" />
									{/if}

									<div class="flex-1 min-w-0">
										<p class="text-sm font-medium leading-tight">
											{info.label}
											{#if info.required}
												<span class="ml-1 text-xs bg-red-100 text-red-600 px-1.5 py-0.5 rounded">จำเป็น</span>
											{:else}
												<span class="ml-1 text-xs bg-gray-100 text-gray-500 px-1.5 py-0.5 rounded">ถ้ามี</span>
											{/if}
										</p>
										{#if hasFile}
											<p class="text-xs text-green-600 truncate mt-0.5">
												✓ {existing?.originalFilename ?? newDoc?.name ?? 'อัปโหลดแล้ว'}
												{#if existing?.fileSize || newDoc?.size}
													<span class="text-muted-foreground">({formatBytes((existing?.fileSize ?? newDoc?.size)!)})</span>
												{/if}
											</p>
										{/if}
									</div>

									<div class="flex items-center gap-2 shrink-0">
										{#if newDoc?.uploading}
											<LoaderCircle class="w-4 h-4 animate-spin text-blue-500" />
										{:else if hasFile}
											<!-- Re-crop button (only for newly selected files with original) -->
											{#if newDoc?.originalBlob}
												<button
													type="button"
													class="text-xs text-muted-foreground hover:text-blue-600 hover:underline"
													onclick={() => handleReCrop(docType)}
												>
													แก้ crop
												</button>
											{/if}
											<!-- Replace button -->
											<button
												type="button"
												class="text-xs text-blue-600 hover:underline"
												onclick={() => fileInputRefs[docType]?.click()}
											>
												เปลี่ยน
											</button>
											<!-- Delete button -->
											{#if existing}
												<button
													type="button"
													class="text-red-400 hover:text-red-600 disabled:opacity-50"
													disabled={deletingDoc === docType}
													onclick={() => handleDocDeleteExisting(docType)}
												>
													{#if deletingDoc === docType}
														<LoaderCircle class="w-4 h-4 animate-spin" />
													{:else}
														<X class="w-4 h-4" />
													{/if}
												</button>
											{:else}
												<button
													type="button"
													class="text-red-400 hover:text-red-600"
													onclick={() => handleDocRemoveNew(docType)}
												>
													<X class="w-4 h-4" />
												</button>
											{/if}
										{:else}
											<!-- Upload button -->
											<button
												type="button"
												class="flex items-center gap-1 text-xs text-blue-600 hover:text-blue-700 border border-blue-200 hover:border-blue-400 rounded-md px-2.5 py-1.5 transition-colors"
												onclick={() => fileInputRefs[docType]?.click()}
											>
												<Camera class="w-3.5 h-3.5" />
												เลือก/ถ่ายรูป
											</button>
										{/if}
									</div>
								</div>
							{/each}
						</Card.Content>
					</Card.Root>

					<!-- Document Cropper Modal -->
					<DocumentCropperModal
						bind:open={cropperOpen}
						imageSrc={cropTarget?.imageUrl ?? null}
						docLabel={cropTarget ? (DOC_TYPE_LABELS[cropTarget.docType]?.label ?? 'เอกสาร') : ''}
						initialCorners={cropTarget?.initialCorners}
						onComplete={handleCropComplete}
						onCancel={handleCropCancel}
					/>

					<!-- Lightbox -->
					{#if lightboxSrc}
						<div class="fixed inset-0 z-50 bg-black/90 flex flex-col" role="dialog" aria-modal="true">
							<!-- Header -->
							<div class="flex items-center justify-between px-4 py-3 bg-black/60 shrink-0">
								<p class="text-white text-sm font-medium truncate pr-4">{lightboxLabel}</p>
								<div class="flex items-center gap-3 shrink-0">
									<button type="button" class="text-white/70 hover:text-white transition-colors" onclick={() => { lbZoom = Math.max(0.5, lbZoom - 0.5); }} title="ย่อ">
										<ZoomOut class="w-5 h-5" />
									</button>
									<span class="text-white/60 text-xs w-10 text-center">{Math.round(lbZoom * 100)}%</span>
									<button type="button" class="text-white/70 hover:text-white transition-colors" onclick={() => { lbZoom = Math.min(8, lbZoom + 0.5); }} title="ขยาย">
										<ZoomIn class="w-5 h-5" />
									</button>
									<button type="button" class="text-white/70 hover:text-white transition-colors ml-2" onclick={closeLightbox} title="ปิด (ESC)">
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
								<img
									src={lightboxSrc}
									alt={lightboxLabel}
									class="max-w-none pointer-events-none select-none"
									style="transform: translate({lbPan.x}px, {lbPan.y}px) scale({lbZoom}); transition: {lbDragging ? 'none' : 'transform 0.1s ease'}; max-height: 80vh; max-width: 90vw;"
									draggable="false"
								/>
							</div>
							<!-- Backdrop close -->
							<button type="button" class="absolute inset-0 -z-10 w-full h-full" onclick={closeLightbox} aria-label="ปิด"></button>
						</div>
					{/if}

					<!-- ===== Submit Bar ===== -->
					<div class="flex flex-col sm:flex-row items-center justify-between gap-4 pt-2 pb-8">
						<p class="text-xs text-muted-foreground text-center sm:text-left max-w-sm">
							ข้าพเจ้าขอรับรองว่าข้อมูลที่กรอกทั้งหมดเป็นความจริง
						</p>
						<Button type="submit" size="lg" disabled={submitting} class="w-full sm:w-auto px-10">
							{#if submitting}
								<LoaderCircle class="w-4 h-4 animate-spin mr-2" />
								{isEditMode ? 'กำลังบันทึก...' : 'กำลังส่งข้อมูล...'}
							{:else}
								{isEditMode ? 'บันทึกการแก้ไข' : 'ส่งใบสมัคร'}
							{/if}
						</Button>
					</div>
				</form>
			{/if}
		{/if}
	</div>
</div>
