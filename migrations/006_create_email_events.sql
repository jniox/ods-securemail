SET search_path = securemail, public;

CREATE TABLE IF NOT EXISTS email_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL,
    email_id UUID NOT NULL REFERENCES emails(id),
    status VARCHAR(20) NOT NULL,
    details TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_email_events_email_created
    ON email_events (email_id, created_at);

CREATE INDEX idx_email_events_tenant_created
    ON email_events (tenant_id, created_at DESC);

-- RLS
ALTER TABLE email_events ENABLE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation ON email_events
    USING (tenant_id = current_setting('app.tenant_id', true)::uuid);

CREATE POLICY tenant_isolation_insert ON email_events
    FOR INSERT WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);
