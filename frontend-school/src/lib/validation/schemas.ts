import { z } from 'zod';

/**
 * Common validation schemas for the application
 */

// ===== Common Fields =====

export const emailSchema = z
    .string()
    .min(1, 'กรุณากรอกอีเมล')
    .email('รูปแบบอีเมลไม่ถูกต้อง');

export const phoneSchema = z
    .string()
    .min(1, 'กรุณากรอกเบอร์โทรศัพท์')
    .regex(/^[0-9]{9,10}$/, 'เบอร์โทรศัพท์ต้องเป็นตัวเลข 9-10 หลัก');

export const passwordSchema = z
    .string()
    .min(8, 'รหัสผ่านต้องมีอย่างน้อย 8 ตัวอักษร')
    .regex(/[A-Z]/, 'รหัสผ่านต้องมีตัวพิมพ์ใหญ่อย่างน้อย 1 ตัว')
    .regex(/[a-z]/, 'รหัสผ่านต้องมีตัวพิมพ์เล็กอย่างน้อย 1 ตัว')
    .regex(/[0-9]/, 'รหัสผ่านต้องมีตัวเลขอย่างน้อย 1 ตัว');

export const thaiIdCardSchema = z
    .string()
    .regex(/^[0-9]{13}$/, 'เลขบัตรประชาชนต้องเป็นตัวเลข 13 หลัก');

// ===== Authentication Schemas =====

export const loginSchema = z.object({
    email: emailSchema,
    password: z.string().min(1, 'กรุณากรอกรหัสผ่าน')
});

export const changePasswordSchema = z
    .object({
        current_password: z.string().min(1, 'กรุณากรอกรหัสผ่านปัจจุบัน'),
        new_password: passwordSchema,
        confirm_password: z.string().min(1, 'กรุณายืนยันรหัสผ่าน')
    })
    .refine((data) => data.new_password === data.confirm_password, {
        message: 'รหัสผ่านไม่ตรงกัน',
        path: ['confirm_password']
    });

// ===== Staff Schemas =====

export const staffBasicInfoSchema = z.object({
    first_name: z.string().min(2, 'ชื่อต้องมีอย่างน้อย 2 ตัวอักษร'),
    last_name: z.string().min(2, 'นามสกุลต้องมีอย่างน้อย 2 ตัวอักษร'),
    email: emailSchema,
    phone: phoneSchema.optional().or(z.literal('')),
    date_of_birth: z.string().optional().or(z.literal('')),
    gender: z.enum(['male', 'female', 'other']).optional(),
    address: z.string().optional()
});

export const staffWorkInfoSchema = z.object({
    employee_id: z.string().min(1, 'กรุณากรอกรหัสพนักงาน'),
    hired_date: z.string().min(1, 'กรุณาเลือกวันที่เริ่มงาน'),
    position: z.string().optional().or(z.literal('')),
    department: z.string().optional(),
    salary: z.number().positive('เงินเดือนต้องมากกว่า 0').optional()
});

export const createStaffSchema = z.object({
    // Basic info
    first_name: z.string().min(2, 'ชื่อต้องมีอย่างน้อย 2 ตัวอักษร'),
    last_name: z.string().min(2, 'นามสกุลต้องมีอย่างน้อย 2 ตัวอักษร'),
    email: emailSchema,
    phone: phoneSchema.optional().or(z.literal('')),

    // Work info
    employee_id: z.string().min(1, 'กรุณากรอกรหัสพนักงาน'),
    hired_date: z.string().min(1, 'กรุณาเลือกวันที่เริ่มงาน'),
    position: z.string().optional().or(z.literal('')),

    // Optional fields
    date_of_birth: z.string().optional().or(z.literal('')),
    gender: z.enum(['male', 'female', 'other']).optional(),
    address: z.string().optional(),
    emergency_contact: z.string().optional().or(z.literal('')),
    line_id: z.string().optional().or(z.literal('')),

    // Password (for new users)
    password: passwordSchema.optional(),

    // Role and department assignments
    roles: z.array(z.string()).min(1, 'กรุณาเลือกตำแหน่งอย่างน้อย 1 ตำแหน่ง'),
    departments: z.array(z.string()).optional()
});

export const updateStaffSchema = createStaffSchema.partial({
    password: true,
    employee_id: true,
    hired_date: true
});

// ===== Role Schemas =====

export const createRoleSchema = z.object({
    name: z.string().min(2, 'ชื่อบทบาทต้องมีอย่างน้อย 2 ตัวอักษร'),
    description: z.string().optional().or(z.literal('')),
    permissions: z.array(z.string()).min(1, 'กรุณาเลือกสิทธิ์อย่างน้อย 1 สิทธิ์')
});

export const updateRoleSchema = createRoleSchema;

// ===== Department Schemas =====

export const createDepartmentSchema = z.object({
    name: z.string().min(2, 'ชื่อแผนกต้องมีอย่างน้อย 2 ตัวอักษร'),
    description: z.string().optional().or(z.literal('')),
    parent_id: z.string().uuid().optional().nullable()
});

export const updateDepartmentSchema = createDepartmentSchema;

// ===== Student Schemas (for future use) =====

export const createStudentSchema = z.object({
    first_name: z.string().min(2, 'ชื่อต้องมีอย่างน้อย 2 ตัวอักษร'),
    last_name: z.string().min(2, 'นามสกุลต้องมีอย่างน้อย 2 ตัวอักษร'),
    student_id: z.string().min(1, 'กรุณากรอกรหัสนักเรียน'),
    email: emailSchema.optional().or(z.literal('')),
    phone: phoneSchema.optional().or(z.literal('')),
    date_of_birth: z.string().min(1, 'กรุณาเลือกวันเกิด'),
    gender: z.enum(['male', 'female', 'other']),
    address: z.string().optional().or(z.literal('')),
    parent_name: z.string().min(2, 'กรุณากรอกชื่อผู้ปกครอง'),
    parent_phone: phoneSchema,
    parent_email: emailSchema.optional().or(z.literal(''))
});

// ===== Type Exports =====

export type LoginInput = z.infer<typeof loginSchema>;
export type ChangePasswordInput = z.infer<typeof changePasswordSchema>;
export type CreateStaffInput = z.infer<typeof createStaffSchema>;
export type UpdateStaffInput = z.infer<typeof updateStaffSchema>;
export type CreateRoleInput = z.infer<typeof createRoleSchema>;
export type UpdateRoleInput = z.infer<typeof updateRoleSchema>;
export type CreateDepartmentInput = z.infer<typeof createDepartmentSchema>;
export type UpdateDepartmentInput = z.infer<typeof updateDepartmentSchema>;
export type CreateStudentInput = z.infer<typeof createStudentSchema>;

// ===== Achievement Schemas =====

export const achievementSchema = z.object({
    title: z.string().min(2, 'ชื่อผลงานต้องมีอย่างน้อย 2 ตัวอักษร'),
    achievement_date: z.string().min(1, 'กรุณาเลือกวันที่ได้รับ'),
    description: z.string().optional().or(z.literal('')),
    image_path: z.string().optional().or(z.literal(''))
});

export type AchievementInput = z.infer<typeof achievementSchema>;
