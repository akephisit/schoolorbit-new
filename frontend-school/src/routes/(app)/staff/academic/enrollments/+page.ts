/**
 * Enrollment Management Page
 */

export const _meta = {
    menu: {
        title: 'จัดห้องเรียน',
        icon: 'Users',
        group: 'academic',
        order: 30,
        user_type: 'staff',
        permission: 'academic_enrollment.read.all'
    }
};

export const load = async () => {
    return {
        title: 'จัดการรายชื่อนักเรียนในห้องเรียน'
    };
};
