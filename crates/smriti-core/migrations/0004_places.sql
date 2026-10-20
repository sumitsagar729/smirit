-- Where an asset's place came from: its own GPS, or a town named in its folder.
ALTER TABLE assets ADD COLUMN place_source TEXT CHECK (place_source IN ('gps', 'folder'));

ALTER TABLE places ADD COLUMN country_code TEXT;

ALTER TABLE trips ADD COLUMN cover_asset_id INTEGER REFERENCES assets(id) ON DELETE SET NULL;
ALTER TABLE trips ADD COLUMN item_count INTEGER NOT NULL DEFAULT 0;

-- "On this day": month-day of the capture time.
CREATE INDEX assets_month_day ON assets (substr(taken_at, 6, 5));
