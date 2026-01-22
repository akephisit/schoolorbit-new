export const _meta = {
    menu: {
        title: 'ตั้งค่าคาบเวลา',
        icon: 'Clock',
        group: 'academic',
        permission: 'academic_structure.manage.all',
        order: 50,
        user_type: 'staff'
    }
};

export const load = async () => {
    return {
        title: 'ตั้งค่าคาบเวลา'
    };
};
