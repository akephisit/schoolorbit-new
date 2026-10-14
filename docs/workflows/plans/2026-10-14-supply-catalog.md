# Supply catalog implementation plan

Approved user scope is recorded in the adjacent dated design and chat.

1. Refresh origin/main and isolate Coordinator, Backend/Data and Frontend branches. Coordinator owns contracts, file-platform integration, router composition, docs and generated artifacts. Backend owns the supply module/policy and migration 110. Frontend owns catalog routes/components/API and focused browser fixtures.
2. Implement typed category/item/version/favorite/bundle/proposal models and services, resource policy, transactional file relationships, additive migration and focused database tests. Share DTOs with the frontend before consumers are built.
3. Extend canonical file purposes/access/delete guard, image normalization and focused inspector/service tests. Generate permissions and OpenAPI from owning contracts.
4. Implement read-first responsive catalog, lazy detail/version/category workflows, favorites, bundles and proposal submission/review. Compress/convert image inputs with actionable failures and clean disposable uploads.
5. Integrate child commits, freeze candidate, run read-only Rules/Security review and local verifier with one heavy Cargo owner. Fix findings, rerun affected checks and review exact tested tree.
6. Record actual evidence in PR, attach it, keep draft if required checks cannot run, otherwise follow repository integration/production acceptance workflow. Remove completed workflow artifacts after outcome is recorded in PR.
