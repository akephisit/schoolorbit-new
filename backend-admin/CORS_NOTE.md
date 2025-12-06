# CORS Configuration - Important Note

## ⚠️ CORS is NOT handled by the backend application

This backend does **NOT** include CORS middleware. Instead, CORS is handled by:

### 🔧 Nginx Reverse Proxy (Production)

For production deployment, use nginx as a reverse proxy with CORS configuration.

**Setup Guide**: See `NGINX_SETUP.md`

**Quick Start**:
```bash
# Copy example config
sudo cp nginx.conf.example /etc/nginx/sites-available/backend-admin

# Edit and enable
sudo ln -s /etc/nginx/sites-available/backend-admin /etc/nginx/sites-enabled/
sudo nginx -t
sudo systemctl reload nginx
```

### 🧪 Development (Local)

For local development with frontend on different port:

**Option 1: Chrome with CORS disabled** (not recommended)
```bash
google-chrome --disable-web-security --user-data-dir=/tmp/chrome
```

**Option 2: Browser Extension**
- Install "CORS Unblock" extension
- Enable only for localhost

**Option 3: Local Nginx**
```bash
# Install nginx locally
sudo apt install nginx

# Use nginx.conf.example with localhost origins
# Change origins to: http://localhost:5173
```

### ✅ Benefits of Nginx CORS

1. **Centralized**: One place to manage CORS
2. **Performance**: Nginx handles CORS faster than application
3. **Security**: Nginx validates before reaching backend
4. **SSL/TLS**: Terminate HTTPS at nginx
5. **Flexibility**: Easy to add/remove origins

### 🔐 Security

Backend still provides:
- ✅ JWT authentication
- ✅ HttpOnly cookies
- ✅ Secure cookie flags
- ✅ Database encryption

Nginx adds:
- ✅ CORS validation
- ✅ HTTPS/SSL
- ✅ Security headers
- ✅ Rate limiting

---

## 📝 Configuration Example

### Allowed Origins (nginx)

```nginx
if ($http_origin ~* (https://admin\.schoolorbit\.app|http://localhost:5173)) {
    set $cors_origin $http_origin;
}
```

### Environment Variables

**Backend NO longer uses:**
- ~~`ALLOWED_ORIGINS`~~ (removed)

**Nginx config only** (see `nginx.conf.example`)

---

## 🚀 Deployment Checklist

Production:
- [ ] Nginx installed and configured
- [ ] SSL certificate obtained
- [ ] CORS origins configured in nginx
- [ ] Backend running without CORS
- [ ] Test CORS from frontend

Development:
- [ ] Use nginx locally, OR
- [ ] Use browser CORS extension, OR
- [ ] Deploy frontend and backend together

---

**For detailed setup instructions, see `NGINX_SETUP.md`**
