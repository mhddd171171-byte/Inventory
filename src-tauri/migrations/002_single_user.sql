-- FIFO ledger. Integer baisa; NULL denotes unavailable legacy cost/price, never zero.
PRAGMA application_id = 1230392653;
CREATE TABLE products (
 id TEXT PRIMARY KEY NOT NULL,
 item_code TEXT NOT NULL UNIQUE CHECK(length(item_code) BETWEEN 1 AND 30 AND item_code NOT GLOB '*[^0-9]*' AND substr(item_code,1,1) BETWEEN '1' AND '9'),
 name TEXT NOT NULL CHECK(length(name) BETWEEN 1 AND 200),
 model_description TEXT NOT NULL CHECK(length(model_description) BETWEEN 1 AND 2000),
 created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE stock_entries (
 id TEXT PRIMARY KEY NOT NULL, product_id TEXT NOT NULL REFERENCES products(id),
 quantity INTEGER NOT NULL CHECK(typeof(quantity)='integer' AND quantity BETWEEN 1 AND 1000000),
 unit_cost_baisa INTEGER CHECK(unit_cost_baisa IS NULL OR (typeof(unit_cost_baisa)='integer' AND unit_cost_baisa BETWEEN 0 AND 1000000000)),
 created_at TEXT NOT NULL, legacy INTEGER NOT NULL DEFAULT 0 CHECK(legacy IN (0,1)),
 CHECK(legacy=1 OR unit_cost_baisa IS NOT NULL)
);
CREATE TABLE sales (
 id TEXT PRIMARY KEY NOT NULL,
 cr_number TEXT CHECK(cr_number IS NULL OR (length(cr_number) BETWEEN 1 AND 30 AND cr_number NOT GLOB '*[^0-9]*' AND substr(cr_number,1,1) BETWEEN '1' AND '9')),
 created_at TEXT NOT NULL, revenue_baisa INTEGER, cost_baisa INTEGER, profit_baisa INTEGER,
 legacy INTEGER NOT NULL DEFAULT 0 CHECK(legacy IN (0,1)), legacy_reference TEXT NOT NULL DEFAULT '',
 CHECK(legacy=1 OR (cr_number IS NOT NULL AND revenue_baisa IS NOT NULL)),
 CHECK(revenue_baisa IS NULL OR (typeof(revenue_baisa)='integer' AND revenue_baisa BETWEEN 0 AND 9007199254740991)),
 CHECK(cost_baisa IS NULL OR (typeof(cost_baisa)='integer' AND cost_baisa BETWEEN 0 AND 9007199254740991)),
 CHECK(profit_baisa IS (revenue_baisa-cost_baisa))
);
CREATE TABLE sale_lines (
 id TEXT PRIMARY KEY NOT NULL, sale_id TEXT NOT NULL REFERENCES sales(id), product_id TEXT NOT NULL REFERENCES products(id),
 item_code TEXT NOT NULL, product_name TEXT NOT NULL, model_description TEXT NOT NULL,
 quantity INTEGER NOT NULL CHECK(typeof(quantity)='integer' AND quantity BETWEEN 1 AND 1000000),
 unit_price_baisa INTEGER CHECK(unit_price_baisa IS NULL OR (typeof(unit_price_baisa)='integer' AND unit_price_baisa BETWEEN 0 AND 1000000000)),
 cost_baisa INTEGER CHECK(cost_baisa IS NULL OR (typeof(cost_baisa)='integer' AND cost_baisa BETWEEN 0 AND 9007199254740991)),
 revenue_baisa INTEGER, profit_baisa INTEGER,
 CHECK(revenue_baisa IS (quantity*unit_price_baisa)), CHECK(profit_baisa IS (revenue_baisa-cost_baisa))
);
CREATE TABLE fifo_allocations (
 line_id TEXT NOT NULL REFERENCES sale_lines(id), entry_id TEXT NOT NULL REFERENCES stock_entries(id),
 quantity INTEGER NOT NULL CHECK(typeof(quantity)='integer' AND quantity BETWEEN 1 AND 1000000), PRIMARY KEY(line_id,entry_id)
);
-- Business records removed from the active UI remain available for recovery.
-- No login credentials or password hashes are retained in this archive.
CREATE TABLE legacy_archive (kind TEXT NOT NULL, record_id TEXT NOT NULL, record_json TEXT NOT NULL, PRIMARY KEY(kind,record_id));
CREATE TABLE migration_info (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL);
CREATE INDEX idx_products_name ON products(name COLLATE NOCASE);
CREATE INDEX idx_entries_product ON stock_entries(product_id,created_at);
CREATE INDEX idx_sales_date ON sales(created_at);
CREATE INDEX idx_sales_cr ON sales(cr_number);
CREATE INDEX idx_lines_product ON sale_lines(product_id);
CREATE INDEX idx_allocations_entry ON fifo_allocations(entry_id);
CREATE VIEW entry_balances AS SELECT e.*, e.quantity-COALESCE((SELECT SUM(a.quantity) FROM fifo_allocations a WHERE a.entry_id=e.id),0) AS remaining_quantity FROM stock_entries e;
CREATE VIEW product_balances AS SELECT p.id AS product_id,COALESCE(SUM(e.remaining_quantity),0) AS quantity,
 COALESCE(SUM(CASE WHEN e.unit_cost_baisa IS NULL THEN e.remaining_quantity ELSE 0 END),0) AS unknown_cost_units,
 COALESCE(SUM(e.remaining_quantity*e.unit_cost_baisa),0) AS known_value_baisa
 FROM products p LEFT JOIN entry_balances e ON e.product_id=p.id GROUP BY p.id;
CREATE TRIGGER entry_validate BEFORE INSERT ON stock_entries BEGIN
 SELECT CASE WHEN NEW.quantity+(SELECT quantity FROM product_balances WHERE product_id=NEW.product_id)>1000000 THEN RAISE(ABORT,'The maximum stock quantity is 1000000.') END;
END;
CREATE TRIGGER allocation_validate BEFORE INSERT ON fifo_allocations BEGIN
 SELECT CASE WHEN (SELECT product_id FROM stock_entries WHERE id=NEW.entry_id)!=(SELECT product_id FROM sale_lines WHERE id=NEW.line_id) THEN RAISE(ABORT,'FIFO product mismatch.') END;
 SELECT CASE WHEN NEW.quantity>(SELECT remaining_quantity FROM entry_balances WHERE id=NEW.entry_id) THEN RAISE(ABORT,'Insufficient stock.') END;
 SELECT CASE WHEN NEW.quantity+COALESCE((SELECT SUM(quantity) FROM fifo_allocations WHERE line_id=NEW.line_id),0)>(SELECT quantity FROM sale_lines WHERE id=NEW.line_id) THEN RAISE(ABORT,'FIFO line quantity exceeded.') END;
END;
CREATE TRIGGER product_no_delete BEFORE DELETE ON products BEGIN SELECT RAISE(ABORT,'Products with permanent history cannot be deleted.'); END;
CREATE TRIGGER entry_no_update BEFORE UPDATE ON stock_entries BEGIN SELECT RAISE(ABORT,'Purchase batches are permanent.'); END;
CREATE TRIGGER entry_no_delete BEFORE DELETE ON stock_entries BEGIN SELECT RAISE(ABORT,'Purchase batches are permanent.'); END;
CREATE TRIGGER sale_no_update BEFORE UPDATE ON sales BEGIN SELECT RAISE(ABORT,'Sales are permanent.'); END;
CREATE TRIGGER sale_no_delete BEFORE DELETE ON sales BEGIN SELECT RAISE(ABORT,'Sales are permanent.'); END;
CREATE TRIGGER line_no_update BEFORE UPDATE ON sale_lines BEGIN SELECT RAISE(ABORT,'Sale lines are permanent.'); END;
CREATE TRIGGER line_no_delete BEFORE DELETE ON sale_lines BEGIN SELECT RAISE(ABORT,'Sale lines are permanent.'); END;
CREATE TRIGGER allocation_no_update BEFORE UPDATE ON fifo_allocations BEGIN SELECT RAISE(ABORT,'FIFO allocations are permanent.'); END;
CREATE TRIGGER allocation_no_delete BEFORE DELETE ON fifo_allocations BEGIN SELECT RAISE(ABORT,'FIFO allocations are permanent.'); END;
PRAGMA user_version = 2;
