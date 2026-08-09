import type { SessionDto } from '$lib/api/auth';

export function removeRevokedSession(sessions: SessionDto[], id: string): SessionDto[] {
	return sessions.filter((session) => session.id !== id);
}

export function keepCurrentSession(sessions: SessionDto[]): SessionDto[] {
	return sessions.filter((session) => session.isCurrent);
}

export function passwordValidation(
	currentPassword: string,
	newPassword: string,
	confirmPassword: string
): string | null {
	if (!currentPassword || !newPassword || !confirmPassword) {
		return 'กรุณากรอกข้อมูลให้ครบถ้วน';
	}
	if (newPassword !== confirmPassword) return 'รหัสผ่านใหม่ไม่ตรงกัน';
	if ([...newPassword].length < 8 || [...newPassword].length > 128) {
		return 'รหัสผ่านต้องมี 8–128 ตัวอักษร';
	}
	if (new TextEncoder().encode(newPassword).length > 71) {
		return 'รหัสผ่านยาวเกินขีดจำกัดที่ปลอดภัย';
	}
	return null;
}
