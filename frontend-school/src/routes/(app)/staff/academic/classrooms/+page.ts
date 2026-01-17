/**
 * Classroom Management Page
 */

export const _meta = {
    menu: {
        title: 'จัดการห้องเรียน',
        icon: 'LayoutGrid',
        group: 'academic',
        order: 20,
        user_type: 'staff',
        permission: 'academic_classroom.read.all'
    }
};

export const load = async () => {
    return {
        title: 'จัดการห้องเรียนและครูประจำชั้น'
    };
};
