# วิธี Login VPS ด้วย SSH Key (PuTTY) โดยไม่ต้องใส่รหัสผ่าน

## สิ่งที่ต้องมี
- PuTTY (ดาวน์โหลดได้จาก https://www.putty.org/)
- PuTTYgen (มาพร้อมกับ PuTTY)
- IP address และ username ของ VPS

---

## ขั้นตอนที่ 1: สร้าง SSH Key ด้วย PuTTYgen

1. เปิดโปรแกรม **PuTTYgen**
2. ในส่วน "Type of key to generate" เลือก **Ed25519**
3. กด **Generate**
4. ขยับเมาส์ในกล่องสีเทาเพื่อสร้าง randomness จนแถบเต็ม
5. กด **Save private key** → บันทึกไฟล์เป็น `my-vps-key.ppk` เก็บไว้ในที่ปลอดภัย
6. **คัดลอก** ข้อความทั้งหมดในกล่อง "Public key for pasting into OpenSSH authorized_keys file" ไว้ในคลิปบอร์ด

> ข้อความ public key จะมีหน้าตาแบบนี้:
> ```
> ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
> ```

---

## ขั้นตอนที่ 2: อัปโหลด Public Key ไปยัง VPS

### 2.1 Login เข้า VPS ด้วยรหัสผ่านก่อน (ครั้งสุดท้าย)

1. เปิด **PuTTY**
2. ใส่ **Host Name** = IP ของ VPS
3. กด **Open** แล้ว Login ด้วย username และ password ตามปกติ

### 2.2 เพิ่ม Public Key บน VPS

รันคำสั่งต่อไปนี้ทีละบรรทัด:

```bash
mkdir -p ~/.ssh
```

```bash
echo "วาง-public-key-ที่-copy-มา-ตรงนี้" >> ~/.ssh/authorized_keys
```

**ตัวอย่าง:**
```bash
echo "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx" >> ~/.ssh/authorized_keys
```

ตั้งค่า permission ให้ถูกต้อง:
```bash
chmod 700 ~/.ssh
chmod 600 ~/.ssh/authorized_keys
```

> หากใช้ user **root** ให้ใช้ path `/root/.ssh/authorized_keys`

---

## ขั้นตอนที่ 3: ตั้งค่า PuTTY ให้ใช้ Private Key

1. เปิด **PuTTY**
2. ใส่ **Host Name** = IP ของ VPS
3. ไปที่เมนูด้านซ้าย: **Connection → SSH → Auth → Credentials**
4. ในช่อง **Private key file for authentication** กด **Browse** → เลือกไฟล์ `my-vps-key.ppk`
5. กลับไปที่ **Session** (เมนูบนสุด)
6. ตั้งชื่อใน **Saved Sessions** เช่น `my-vps`
7. กด **Save** เพื่อบันทึก

---

## ขั้นตอนที่ 4: ทดสอบ Login

1. เลือก Session ที่บันทึกไว้ → กด **Load** → กด **Open**
2. ใส่ username แล้วกด Enter
3. ถ้าเข้าได้โดยไม่ถามรหัสผ่าน แสดงว่าสำเร็จ

---

## (ทางเลือก) ปิด Password Login เพื่อความปลอดภัย

หลังจากยืนยันว่า SSH Key ใช้งานได้แล้ว สามารถปิด Password Login ได้:

```bash
sudo nano /etc/ssh/sshd_config
```

แก้ไขบรรทัดต่อไปนี้:
```
PasswordAuthentication no
PubkeyAuthentication yes
```

Restart SSH service:
```bash
sudo systemctl restart sshd
```

> **คำเตือน:** ทดสอบให้แน่ใจว่า SSH Key ใช้งานได้ก่อนปิด Password Login มิฉะนั้นอาจล็อคตัวเองออกจาก VPS
