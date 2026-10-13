-- Migration 008 verified preservation and retired the source credentials.
-- These provider-transfer checkpoints have no ongoing runtime or recovery consumer.
-- SQLx history, encrypted tenant credentials and business data remain authoritative.
ALTER TABLE schools DROP COLUMN db_credentials_reconciled_at;
ALTER TABLE schools DROP COLUMN db_credentials_reconciliation;
