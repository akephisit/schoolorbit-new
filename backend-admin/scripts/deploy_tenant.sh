#!/bin/bash
# Deployment script for frontend-school to Cloudflare Workers
# Usage: ./deploy_tenant.sh <subdomain> <school_id> <api_url>

set -e  # Exit on error

SUBDOMAIN=$1
SCHOOL_ID=$2
API_URL=$3

if [ -z "$SUBDOMAIN" ] || [ -z "$SCHOOL_ID" ] || [ -z "$API_URL" ]; then
    echo "Usage: $0 <subdomain> <school_id> <api_url>"
    exit 1
fi

echo "🚀 Deploying frontend-school for subdomain: $SUBDOMAIN"

# Get paths from environment or use defaults
FRONTEND_SCHOOL_PATH=${FRONTEND_SCHOOL_BUILD_PATH:-"../frontend-school"}
TEMP_DIR="/tmp/schoolorbit-deploy-$SUBDOMAIN"

# Create temporary directory
rm -rf "$TEMP_DIR"
mkdir -p "$TEMP_DIR"

echo "📦 Copying frontend-school to temp directory..."
cp -r "$FRONTEND_SCHOOL_PATH"/* "$TEMP_DIR/"

cd "$TEMP_DIR"

# Update wrangler.json with tenant-specific configuration
echo "⚙️  Updating wrangler configuration..."
cat > wrangler.json <<EOF
{
  "name": "schoolorbit-school-$SUBDOMAIN",
  "account_id": "${CLOUDFLARE_ACCOUNT_ID}",
  "main": "build/index.js",
  "build": {
    "command": "npm run build"
  },
  "compatibility_date": "2025-09-15",
  "compatibility_flags": [
    "nodejs_compat"
  ],
  "assets": {
    "directory": "build/client",
    "binding": "ASSETS"
  },
  "vars": {
    "PUBLIC_API_URL": "$API_URL",
    "SCHOOL_ID": "$SCHOOL_ID",
    "SUBDOMAIN": "$SUBDOMAIN"
  }
}
EOF

# Install dependencies if needed
if [ ! -d "node_modules" ]; then
    echo "📦 Installing dependencies..."
    npm install
fi

# Build the project
echo "🔨 Building frontend-school..."
npm run build

# Deploy to Cloudflare Workers
echo "☁️  Deploying to Cloudflare Workers..."
export CLOUDFLARE_API_TOKEN="${CLOUDFLARE_API_TOKEN}"
npx wrangler deploy

echo "✅ Deployment completed successfully!"
echo "   Subdomain: $SUBDOMAIN"
echo "   School ID: $SCHOOL_ID"
echo "   URL: https://$SUBDOMAIN.schoolorbit.app"

# Cleanup
cd /
rm -rf "$TEMP_DIR"

exit 0
