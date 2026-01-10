#!/bin/bash

# ============================================================================
# Cloudflare R2 Setup Helper Script
# ============================================================================
# This script helps you set up and configure Cloudflare R2 for SchoolOrbit
# ============================================================================

set -e

echo "============================================"
echo "🚀 SchoolOrbit - R2 Setup Helper"
echo "============================================"
echo ""

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Function to print colored messages
info() {
    echo -e "${GREEN}ℹ${NC} $1"
}

warn() {
    echo -e "${YELLOW}⚠${NC} $1"
}

error() {
    echo -e "${RED}✗${NC} $1"
}

success() {
    echo -e "${GREEN}✓${NC} $1"
}

echo "This script will help you configure Cloudflare R2 for file storage."
echo ""

# ============================================================================
# Step 1: Check if .env file exists
# ============================================================================
if [ ! -f .env ]; then
    info "No .env file found. Creating from .env.example..."
    cp .env.example .env
    success ".env file created"
else
    info "Using existing .env file"
fi

echo ""
echo "============================================"
echo "📝 R2 Configuration"
echo "============================================"
echo ""
echo "You'll need the following from Cloudflare Dashboard:"
echo "  1. Account ID"
echo "  2. R2 Access Key ID"
echo "  3. R2 Secret Access Key"
echo "  4. Bucket Name"
echo "  5. Public URL (from R2 settings)"
echo ""
echo "Get these from: https://dash.cloudflare.com"
echo "Navigate to: R2 Object Storage > Manage R2 API Tokens"
echo ""

read -p "Do you have your R2 credentials ready? (y/n): " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    warn "Please get your R2 credentials first, then run this script again."
    exit 0
fi

# ============================================================================
# Step 2: Collect R2 credentials
# ============================================================================
echo ""
info "Enter your R2 credentials:"
echo ""

read -p "Account ID: " R2_ACCOUNT_ID
read -p "Access Key ID: " R2_ACCESS_KEY_ID
read -s -p "Secret Access Key: " R2_SECRET_ACCESS_KEY
echo ""
read -p "Bucket Name [schoolorbit-files]: " R2_BUCKET_NAME
R2_BUCKET_NAME=${R2_BUCKET_NAME:-schoolorbit-files}

read -p "R2 Public URL (e.g., https://pub-xxxxx.r2.dev): " R2_PUBLIC_URL

# ============================================================================
# Step 3: Optional CDN setup
# ============================================================================
echo ""
read -p "Do you have a CDN URL configured? (y/n): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    read -p "CDN URL (e.g., https://cdn.schoolorbit.app): " CDN_URL
else
    CDN_URL=""
fi

# ============================================================================
# Step 4: Update .env file
# ============================================================================
echo ""
info "Updating .env file..."

# Function to update or add env variable
update_env() {
    local key=$1
    local value=$2
    
    if grep -q "^${key}=" .env; then
        # Update existing
        sed -i "s|^${key}=.*|${key}=${value}|" .env
    else
        # Add new
        echo "${key}=${value}" >> .env
    fi
}

update_env "R2_ACCOUNT_ID" "$R2_ACCOUNT_ID"
update_env "R2_ACCESS_KEY_ID" "$R2_ACCESS_KEY_ID"
update_env "R2_SECRET_ACCESS_KEY" "$R2_SECRET_ACCESS_KEY"
update_env "R2_BUCKET_NAME" "$R2_BUCKET_NAME"
update_env "R2_PUBLIC_URL" "$R2_PUBLIC_URL"
update_env "R2_REGION" "auto"

if [ ! -z "$CDN_URL" ]; then
    update_env "CDN_URL" "$CDN_URL"
fi

success ".env file updated with R2 credentials"

# ============================================================================
# Step 5: Test R2 connection (if AWS CLI with R2 creds is available)
# ============================================================================
echo ""
read -p "Would you like to test the R2 connection? (requires aws-cli) (y/n): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    if command -v aws &> /dev/null; then
        info "Testing R2 connection..."
        
        # Configure AWS CLI for R2
        AWS_ACCESS_KEY_ID=$R2_ACCESS_KEY_ID \
        AWS_SECRET_ACCESS_KEY=$R2_SECRET_ACCESS_KEY \
        aws s3 ls s3://$R2_BUCKET_NAME \
            --endpoint-url https://$R2_ACCOUNT_ID.r2.cloudflarestorage.com \
            2>/dev/null && success "R2 connection successful!" || warn "Could not connect to R2. Please verify your credentials."
    else
        warn "aws-cli not found. Skipping connection test."
        info "Install aws-cli with: brew install awscli (macOS) or apt install awscli (Ubuntu)"
    fi
fi

# ============================================================================
# Step 6: Run migration
# ============================================================================
echo ""
read -p "Would you like to run the file storage migration now? (y/n): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    info "Migration will be run when you start the backend service."
    warn "Make sure to run: cargo run --release"
fi

# ============================================================================
# Summary
# ============================================================================
echo ""
echo "============================================"
echo "✅ R2 Setup Complete!"
echo "============================================"
echo ""
success "R2 credentials configured in .env"
success "Bucket: $R2_BUCKET_NAME"
success "Public URL: $R2_PUBLIC_URL"
if [ ! -z "$CDN_URL" ]; then
    success "CDN URL: $CDN_URL"
fi
echo ""
echo "Next steps:"
echo "  1. Start your backend: cargo run --release"
echo "  2. The migration (020_file_storage_system.sql) will run automatically"
echo "  3. Test file upload functionality"
echo ""
echo "📖 For more information, see: docs/FILE_STORAGE.md"
echo ""
