#!/bin/bash
# Quick Fix Script for Nginx File Upload Support
# Run this on your VPS to update nginx configuration

set -e

echo "🔧 SchoolOrbit - Nginx File Upload Fix"
echo "======================================"
echo ""

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Configuration
NGINX_SITE_CONFIG="/etc/nginx/sites-enabled/school-api.schoolorbit.app"
BACKUP_DIR="/etc/nginx/backups"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

# Check if running as root
if [ "$EUID" -ne 0 ]; then 
   echo -e "${RED}❌ Please run as root (sudo)${NC}"
   exit 1
fi

# Check if config exists
if [ ! -f "$NGINX_SITE_CONFIG" ]; then
    echo -e "${RED}❌ Config file not found: $NGINX_SITE_CONFIG${NC}"
    exit 1
fi

# Create backup directory
mkdir -p "$BACKUP_DIR"

# Backup current config
echo -e "${YELLOW}📦 Backing up current config...${NC}"
cp "$NGINX_SITE_CONFIG" "$BACKUP_DIR/school-api_$TIMESTAMP.conf"
echo -e "${GREEN}✅ Backup saved to: $BACKUP_DIR/school-api_$TIMESTAMP.conf${NC}"
echo ""

# Check if already has client_max_body_size
if grep -q "client_max_body_size" "$NGINX_SITE_CONFIG"; then
    echo -e "${YELLOW}⚠️  Config already has client_max_body_size${NC}"
    echo "Current value:"
    grep "client_max_body_size" "$NGINX_SITE_CONFIG"
    echo ""
    read -p "Do you want to continue and update? (y/n) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo "Aborted."
        exit 0
    fi
fi

# Add file upload configuration
echo -e "${YELLOW}🔨 Updating nginx configuration...${NC}"

# Create temp file with updated config
TMP_CONFIG=$(mktemp)

# Insert global settings after server_name line
awk '/server_name school-api.schoolorbit.app;/ {
    print;
    print "";
    print "    # ========================================";
    print "    # 🔥 GLOBAL FILE UPLOAD SETTINGS";
    print "    # ========================================";
    print "    client_max_body_size 20M;";
    print "    client_body_timeout 300s;";
    print "    client_header_timeout 300s;";
    print "    proxy_connect_timeout 300s;";
    print "    proxy_send_timeout 300s;";
    print "    proxy_read_timeout 300s;";
    next;
}
{ print }' "$NGINX_SITE_CONFIG" > "$TMP_CONFIG"

# Test the new configuration
echo -e "${YELLOW}🧪 Testing nginx configuration...${NC}"
nginx -t -c /etc/nginx/nginx.conf

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✅ Nginx configuration test passed!${NC}"
    
    # Apply the new configuration
    cp "$TMP_CONFIG" "$NGINX_SITE_CONFIG"
    rm "$TMP_CONFIG"
    
    # Reload nginx
    echo -e "${YELLOW}🔄 Reloading nginx...${NC}"
    systemctl reload nginx
    
    if [ $? -eq 0 ]; then
        echo -e "${GREEN}✅ Nginx reloaded successfully!${NC}"
        echo ""
        echo "======================================"
        echo -e "${GREEN}✅ File upload support enabled!${NC}"
        echo "======================================"
        echo ""
        echo "📊 Configuration:"
        echo "  - Max file size: 20MB"
        echo "  - Upload timeout: 5 minutes"
        echo "  - Backup: $BACKUP_DIR/school-api_$TIMESTAMP.conf"
        echo ""
        echo "🧪 Test upload:"
        echo "  curl -X POST https://school-api.schoolorbit.app/api/files/upload \\"
        echo "    -H 'Authorization: Bearer YOUR_TOKEN' \\"
        echo "    -F 'file=@/path/to/image.jpg' \\"
        echo "    -F 'file_type=profile_image'"
    else
        echo -e "${RED}❌ Failed to reload nginx${NC}"
        echo "Restoring backup..."
        cp "$BACKUP_DIR/school-api_$TIMESTAMP.conf" "$NGINX_SITE_CONFIG"
        exit 1
    fi
else
    echo -e "${RED}❌ Nginx configuration test failed!${NC}"
    echo "Config not applied. Original config preserved."
    rm "$TMP_CONFIG"
    exit 1
fi
