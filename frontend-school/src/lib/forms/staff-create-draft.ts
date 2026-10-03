import { z } from 'zod';
const referenceSummary = z
	.object({
		id: z.string().uuid(),
		code: z.string(),
		name: z.string(),
		isActive: z.boolean()
	})
	.nullable();
const legacyFields = z
	.object({
		personnel_references: z.object({
			job_position: referenceSummary,
			major: referenceSummary,
			university: referenceSummary
		}),
		personnel: z.object({
			job_position_id: z.string().uuid().nullable().optional(),
			academic_rank: z
				.enum([
					'none',
					'not_applicable',
					'proficient',
					'senior_proficient',
					'expert',
					'senior_expert'
				])
				.nullable()
				.optional(),
			education_level: z
				.enum([
					'primary',
					'lower_secondary',
					'upper_secondary',
					'vocational_certificate',
					'higher_vocational',
					'diploma',
					'bachelor',
					'master',
					'doctorate',
					'other'
				])
				.nullable()
				.optional(),
			major_id: z.string().uuid().nullable().optional(),
			university_id: z.string().uuid().nullable().optional()
		}),
		username: z.string(),
		email: z.string(),
		title: z.string(),
		first_name: z.string(),
		last_name: z.string(),
		nickname: z.string(),
		phone: z.string(),
		emergency_contact: z.string(),
		line_id: z.string(),
		date_of_birth: z.string(),
		gender: z.string(),
		address: z.string(),
		hired_date: z.string(),
		role_ids: z.array(z.string()),
		primary_role_id: z.string(),
		organization_assignments: z.array(
			z.object({
				organization_unit_id: z.string(),
				position_code: z.string(),
				is_primary: z.boolean(),
				responsibilities: z.string()
			})
		)
	})
	.partial();
const positionSummary = referenceSummary.unwrap().extend({ isSelectable: z.boolean() }).nullable();
const fields = legacyFields.omit({ personnel_references: true, personnel: true }).extend({
	selected_position: positionSummary.optional(),
	personnel: legacyFields.shape.personnel
		.unwrap()
		.omit({ major_id: true, university_id: true })
		.extend({
			major: z.string().nullable().optional(),
			university: z.string().nullable().optional()
		})
		.strict()
		.optional()
});
export type StaffCreateDraft = z.infer<typeof fields>;
type DraftStorage = Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>;
interface DraftOwner {
	origin: string;
	userId: string;
}
const lifetime = 30 * 60 * 1000;
const key = (owner: DraftOwner) => `staff-create-draft:v3:${owner.origin}:${owner.userId}`;
const envelope = z.object({ version: z.literal(3), expiresAt: z.number(), fields });
export function saveStaffCreateDraft(
	storage: DraftStorage,
	owner: DraftOwner,
	draft: StaffCreateDraft,
	now = Date.now()
): void {
	storage.removeItem('staff-create-draft');
	storage.setItem(
		key(owner),
		JSON.stringify({ version: 3, expiresAt: now + lifetime, fields: fields.parse(draft) })
	);
}
const legacyKey = (owner: DraftOwner) => `staff-create-draft:v2:${owner.origin}:${owner.userId}`;
const legacyEnvelope = z.object({
	version: z.literal(2),
	expiresAt: z.number(),
	fields: legacyFields
});
const standardCodes = new Set([
	'teacher',
	'assistant_teacher',
	'contract_teacher',
	'government_employee_teacher',
	'school_director',
	'deputy_school_director',
	'support_staff'
]);
export function readStaffCreateDraft(
	storage: DraftStorage,
	owner: DraftOwner,
	now = Date.now()
): { draft: StaffCreateDraft | null; migrationFailed: boolean } {
	storage.removeItem('staff-create-draft');
	const empty = { draft: null, migrationFailed: false };
	const stored = storage.getItem(key(owner));
	if (stored) {
		try {
			const result = envelope.safeParse(JSON.parse(stored));
			if (result.success && result.data.expiresAt > now && result.data.expiresAt <= now + lifetime)
				return { draft: result.data.fields, migrationFailed: false };
		} catch (error) {
			if (!(error instanceof SyntaxError)) throw error;
		}
		storage.removeItem(key(owner));
	}
	const previous = storage.getItem(legacyKey(owner));
	if (!previous) return empty;
	let parsed;
	try {
		parsed = legacyEnvelope.safeParse(JSON.parse(previous));
	} catch (error) {
		if (!(error instanceof SyntaxError)) throw error;
	}
	if (!parsed?.success || parsed.data.expiresAt <= now || parsed.data.expiresAt > now + lifetime) {
		storage.removeItem(legacyKey(owner));
		return empty;
	}
	const {
		personnel_references: references,
		personnel: oldPersonnel,
		...other
	} = parsed.data.fields;
	const text = (
		id: string | null | undefined,
		summary: z.infer<typeof referenceSummary> | undefined
	) => {
		if (id == null) return null;
		if (
			summary?.id !== id ||
			!summary.name.trim() ||
			/[\u0000-\u001f\u007f-\u009f]/u.test(summary.name) ||
			Array.from(summary.name.trim()).length > 200
		)
			throw new Error('unverifiable education');
		return summary.name.trim();
	};
	let draft: StaffCreateDraft;
	try {
		const position = references?.job_position ?? null;
		if (oldPersonnel?.job_position_id && position?.id !== oldPersonnel.job_position_id)
			throw new Error('unverifiable position');
		const { major_id, university_id, ...personnel } = oldPersonnel ?? {};
		draft = fields.parse({
			...other,
			selected_position: position
				? { ...position, isSelectable: position.isActive && standardCodes.has(position.code) }
				: null,
			personnel: {
				...personnel,
				major: text(major_id, references?.major),
				university: text(university_id, references?.university)
			}
		});
	} catch {
		return { draft: null, migrationFailed: true };
	}
	storage.setItem(
		key(owner),
		JSON.stringify({ version: 3, expiresAt: parsed.data.expiresAt, fields: draft })
	);
	storage.removeItem(legacyKey(owner));
	return { draft, migrationFailed: false };
}
export function clearStaffCreateDraft(storage: DraftStorage, owner: DraftOwner): void {
	storage.removeItem('staff-create-draft');
	storage.removeItem(key(owner));
	storage.removeItem(legacyKey(owner));
}
