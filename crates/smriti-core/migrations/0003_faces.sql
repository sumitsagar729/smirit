-- Face pipeline version that analysed each asset (NULL = not yet).
ALTER TABLE assets ADD COLUMN faces_version INTEGER;

-- "This face is not that person": kept so recognition never re-suggests it.
CREATE TABLE face_rejections (
    face_id   INTEGER NOT NULL REFERENCES faces(id) ON DELETE CASCADE,
    person_id INTEGER NOT NULL REFERENCES persons(id) ON DELETE CASCADE,
    PRIMARY KEY (face_id, person_id)
);

CREATE INDEX faces_assign ON faces (assign_kind, person_id);

-- Photos indexed before faces existed get analysed in the background.
INSERT INTO jobs (asset_id, stage, priority)
SELECT id, 'faces', -mtime FROM assets
WHERE kind = 'photo' AND thumb_state = 'ok' AND status = 'active'
ON CONFLICT (asset_id, stage) DO NOTHING;
