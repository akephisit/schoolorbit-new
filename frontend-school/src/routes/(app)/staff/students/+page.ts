/**
 * Student Management Page (Staff)
 */

export const _meta = {
    menu: {
        title: 'นักเรียน',
        icon: 'GraduationCap',
        group: 'main',
        order: 20,
        permission: 'student.read.all'
    }
};

export const load = async () => {
    return {
        title: 'จัดการนักเรียน'
    };
};
