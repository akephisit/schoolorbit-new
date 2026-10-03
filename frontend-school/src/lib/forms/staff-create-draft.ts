import { z } from 'zod';
const rank = z.enum([
	'none',
	'not_applicable',
	'proficient',
	'senior_proficient',
	'expert',
	'senior_expert'
]);
const personnelType = z.enum([
	'civil_servant',
	'government_employee',
	'contract_employee',
	'permanent_employee',
	'other'
]);
const reference = z
	.object({ id: z.string().uuid(), revision: z.number().int().positive() })
	.strict()
	.nullable();
const detailFields = {
	effectiveDate: z.string(),
	orderDate: z.string(),
	orderNumber: z.string(),
	note: z.string(),
	reference
};
const career = z
	.object({
		personnelType: z.object({ value: personnelType.nullable(), ...detailFields }).strict(),
		jobPosition: z.object({ value: z.string().uuid().nullable(), ...detailFields }).strict(),
		academicRank: z.object({ value: rank.nullable(), ...detailFields }).strict()
	})
	.strict();
const fields = z
	.object({
		selected_position: z
			.object({
				id: z.string().uuid(),
				code: z.string(),
				name: z.string(),
				isActive: z.boolean(),
				isSelectable: z.boolean()
			})
			.nullable(),
		personnel: z
			.object({
				career: career.optional(),
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
				major: z.string().nullable().optional(),
				university: z.string().nullable().optional()
			})
			.strict(),
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
export type StaffCreateDraft = z.infer<typeof fields>;
type DraftStorage = Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>;
interface DraftOwner {
	origin: string;
	userId: string;
}
const lifetime = 30 * 60 * 1000;
const key = (owner: DraftOwner) => `staff-create-draft:v4:${owner.origin}:${owner.userId}`;
const obsoleteKey = (owner: DraftOwner) => `staff-create-draft:v2:${owner.origin}:${owner.userId}`;
const obsoleteCareerKey = (owner: DraftOwner) =>
	`staff-create-draft:v3:${owner.origin}:${owner.userId}`;
const envelope = z.object({ version: z.literal(4), expiresAt: z.number(), fields });
function removeObsoleteKeys(storage: DraftStorage, owner: DraftOwner): void {
	storage.removeItem('staff-create-draft');
	storage.removeItem(obsoleteKey(owner));
	storage.removeItem(obsoleteCareerKey(owner));
}
export function saveStaffCreateDraft(
	storage: DraftStorage,
	owner: DraftOwner,
	draft: StaffCreateDraft,
	now = Date.now()
): void {
	const current = fields.parse(draft);
	removeObsoleteKeys(storage, owner);
	storage.setItem(
		key(owner),
		JSON.stringify({ version: 4, expiresAt: now + lifetime, fields: current })
	);
}
export function readStaffCreateDraft(
	storage: DraftStorage,
	owner: DraftOwner,
	now = Date.now()
): StaffCreateDraft | null {
	removeObsoleteKeys(storage, owner);

	const stored = storage.getItem(key(owner));
	if (stored) {
		const current = parseStored(envelope, stored, now);
		if (current) return current.fields;
		storage.removeItem(key(owner));
	}
	return null;
}
function parseStored<T extends { expiresAt: number }>(
	schema: z.ZodType<T>,
	stored: string,
	now: number
): T | null {
	try {
		const result = schema.safeParse(JSON.parse(stored));
		return result.success && result.data.expiresAt > now && result.data.expiresAt <= now + lifetime
			? result.data
			: null;
	} catch (error) {
		if (!(error instanceof SyntaxError)) throw error;
		return null;
	}
}

export function clearStaffCreateDraft(storage: DraftStorage, owner: DraftOwner): void {
	removeObsoleteKeys(storage, owner);
	storage.removeItem(key(owner));
}
