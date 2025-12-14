# Changelog

All notable changes to frontend-school will be documented in this file.

## [Unreleased]

### 2025-12-14
- Test auto-deployment to all schools
- Trigger workflow: deploy-all-schools.yml

---

## How Auto-Deployment Works

When you commit changes to `frontend-school/`:
1. GitHub Actions detects the change
2. Fetches all active schools from backend-admin API
3. Deploys the updated frontend to each school's Worker
4. Each school gets the same code but with their own:
   - Subdomain (snwsb.schoolorbit.app)
   - School ID (for API calls)
   - Environment variables

This ensures all schools are always up-to-date! 🎉
