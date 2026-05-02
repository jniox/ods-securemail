SET search_path = securemail, public;

CREATE TABLE IF NOT EXISTS emails (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL,
    to_addresses JSONB NOT NULL,
    cc_addresses JSONB NOT NULL DEFAULT '[]',
    bcc_addresses JSONB NOT NULL DEFAULT '[]',
    subject TEXT NOT NULL,
    body_html TEXT NOT NULL,
    body_text TEXT,
    status VARCHAR(20) NOT NULL DEFAULT 'queued'
        CHECK (status IN ('queued', 'sending', 'sent', 'delivered', 'opened', 'bounced', 'failed')),
    encryption_mode VARCHAR(20) NOT NULL DEFAULT 'none'
        CHECK (encryption_mode IN ('none', 'sign', 'encrypt', 'sign_and_encrypt')),
    mail_config_id UUID NOT NULL REFERENCES mail_configs(id),
    template_id UUID REFERENCES templates(id),
    key_id UUID REFERENCES encryption_keys(id),
    batch_id UUID,
    idempotency_key VARCHAR(255),
    scheduled_at TIMESTAMPTZ,
    sent_at TIMESTAMPTZ,
    delivered_at TIMESTAMPTZ,
    opened_at TIMESTAMPTZ,
    bounced_at TIMESTAMPTZ,
    failed_at TIMESTAMPTZ,
    error_message TEXT,
    retry_count INTEGER NOT NULL DEFAULT 0,
    max_retries INTEGER NOT NULL DEFAULT 3,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_emails_tenant_status_created
    ON emails (tenant_id, status, created_at DESC);

CREATE INDEX idx_emails_tenant_created
    ON emails (tenant_id, created_at DESC);

CREATE UNIQUE INDEX emails_tenant_idempotency_key
    ON emails (tenant_id, idempotency_key) WHERE idempotency_key IS NOT NULL;

CREATE INDEX idx_emails_batch
    ON emails (batch_id) WHERE batch_id IS NOT NULL;

CREATE INDEX idx_emails_scheduled
    ON emails (scheduled_at) WHERE status = 'queued' AND scheduled_at IS NOT NULL;

-- RLS
ALTER TABLE emails ENABLE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation ON emails
    USING (tenant_id = current_setting('app.tenant_id', true)::uuid);

CREATE POLICY tenant_isolation_insert ON emails
    FOR INSERT WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);
