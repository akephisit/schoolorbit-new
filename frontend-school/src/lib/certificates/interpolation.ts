export type CertificateInterpolationErrorKind = 'invalid_syntax' | 'missing_values';

export class CertificateInterpolationError extends Error {
	readonly kind: CertificateInterpolationErrorKind;
	readonly missingVariables: string[];

	constructor(kind: CertificateInterpolationErrorKind, missingVariables: string[] = []) {
		const message =
			kind === 'missing_values'
				? `ไม่พบค่าสำหรับตัวแปร: ${missingVariables.join(', ')}`
				: 'รูปแบบตัวแปรในข้อความไม่ถูกต้อง';
		super(message);
		this.name = 'CertificateInterpolationError';
		this.kind = kind;
		this.missingVariables = missingVariables;
	}
}

function displayVariableName(value: string): string {
	return value.normalize('NFC').trim().replace(/\s+/gu, ' ');
}

function variableKey(value: string): string {
	return displayVariableName(value).toLocaleLowerCase('th-TH');
}

export function interpolateCertificateText(
	content: string,
	values: Readonly<Record<string, string>>
): string {
	const normalizedValues = new Map<string, string>();
	for (const [name, value] of Object.entries(values)) {
		normalizedValues.set(variableKey(name), value);
	}

	const output: string[] = [];
	const missingVariables: string[] = [];
	const missingKeys = new Set<string>();
	let cursor = 0;

	while (cursor < content.length) {
		if (content.startsWith('{{', cursor)) {
			output.push('{');
			cursor += 2;
			continue;
		}
		if (content.startsWith('}}', cursor)) {
			output.push('}');
			cursor += 2;
			continue;
		}

		const character = content[cursor];
		if (character === '}') throw new CertificateInterpolationError('invalid_syntax');
		if (character !== '{') {
			output.push(character);
			cursor += 1;
			continue;
		}

		const closingBrace = content.indexOf('}', cursor + 1);
		if (closingBrace === -1) throw new CertificateInterpolationError('invalid_syntax');
		const rawName = content.slice(cursor + 1, closingBrace);
		if (rawName.includes('{')) throw new CertificateInterpolationError('invalid_syntax');
		const displayName = displayVariableName(rawName);
		if (!displayName) throw new CertificateInterpolationError('invalid_syntax');
		const key = variableKey(displayName);
		const value = normalizedValues.get(key);
		if (value === undefined) {
			if (!missingKeys.has(key)) {
				missingKeys.add(key);
				missingVariables.push(displayName);
			}
		} else {
			output.push(value);
		}
		cursor = closingBrace + 1;
	}

	if (missingVariables.length > 0) {
		throw new CertificateInterpolationError('missing_values', missingVariables);
	}
	return output.join('');
}
