export function normalizeSchoolSubdomain(value: string | undefined): string | null {
	if (
		!value ||
		value.trim() !== value ||
		value.length > 63 ||
		value.startsWith('-') ||
		value.endsWith('-')
	) {
		return null;
	}

	const subdomain = value.toLowerCase();
	if (subdomain === 'www' || !/^[a-z0-9-]+$/.test(subdomain)) return null;
	return subdomain;
}
