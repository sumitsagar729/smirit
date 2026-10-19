-- Thumbnail outcome per asset: 'ok', 'failed' or 'unsupported' (e.g. RAW until its decoder lands).
ALTER TABLE assets ADD COLUMN thumb_state TEXT;

-- Timeline reads: newest first among visible items.
CREATE INDEX assets_timeline ON assets (status, is_hidden, taken_at DESC);
