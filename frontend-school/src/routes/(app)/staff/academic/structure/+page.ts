/**
 * Academic Structure Management Page
 */

export const _meta = {
    menu: {
        title: 'โครงสร้างวิชาการ',
        icon: 'Framer',
        group: 'academic',
        order: 10,
        user_type: 'staff',
        permission: 'academic_structure.read.all'
    }
};

export const load = async () => {
    return {
        title: 'ตั้งค่าโครงสร้างวิชาการ'
    };
};
