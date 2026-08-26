export interface SchoolCardIdentity {
	id: string;
}

export interface SchoolCreationFailure<TSchool extends SchoolCardIdentity> {
	schools: TSchool[];
	message: string;
}

const NEON_CONFIGURATION_MESSAGE =
	'ไม่สามารถสร้างฐานข้อมูลโรงเรียนได้ กรุณาตรวจสอบการตั้งค่า Neon แล้วลองอีกครั้ง';
const CONNECTION_MESSAGE = 'การเชื่อมต่อระหว่างสร้างโรงเรียนขัดข้อง กรุณาลองอีกครั้ง';
const GENERIC_MESSAGE = 'ไม่สามารถสร้างโรงเรียนได้ กรุณาลองอีกครั้ง';

function schoolCreationErrorMessage(error: string): string {
	if (/subdomain/i.test(error)) {
		return 'Subdomain นี้มีในระบบแล้ว กรุณาใช้ชื่ออื่น';
	}

	if (/neon|branch not found|failed to create database/i.test(error)) {
		return NEON_CONFIGURATION_MESSAGE;
	}

	if (/connection lost|failed to start sse|no response body/i.test(error)) {
		return CONNECTION_MESSAGE;
	}

	return GENERIC_MESSAGE;
}

export function schoolCreationFailure<TSchool extends SchoolCardIdentity>(
	schools: TSchool[],
	temporarySchoolId: string,
	error: string
): SchoolCreationFailure<TSchool> {
	return {
		schools: schools.filter((school) => school.id !== temporarySchoolId),
		message: schoolCreationErrorMessage(error)
	};
}
