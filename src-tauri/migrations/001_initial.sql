-- Amounts are integer baisa (1 OMR = 1,000 baisa). Stock is derived exclusively
-- from the immutable movement ledger; there is no mutable quantity column.
PRAGMA application_id = 1230392653;

CREATE TABLE users (
    id TEXT PRIMARY KEY NOT NULL,
    username TEXT NOT NULL COLLATE NOCASE UNIQUE CHECK(length(username) BETWEEN 3 AND 64),
    display_name TEXT NOT NULL CHECK(length(display_name) BETWEEN 1 AND 120),
    password_hash TEXT NOT NULL,
    role TEXT NOT NULL CHECK(role IN ('admin', 'employee')),
    active INTEGER NOT NULL DEFAULT 1 CHECK(active IN (0, 1)),
    created_at TEXT NOT NULL
);

CREATE TABLE items (
    id TEXT PRIMARY KEY NOT NULL,
    sku TEXT NOT NULL COLLATE NOCASE UNIQUE CHECK(length(sku) BETWEEN 1 AND 100),
    name TEXT NOT NULL CHECK(length(name) BETWEEN 1 AND 200),
    description TEXT NOT NULL DEFAULT '' CHECK(length(description) <= 2000),
    category TEXT NOT NULL DEFAULT '' CHECK(length(category) <= 120),
    compatible_device TEXT NOT NULL DEFAULT '' CHECK(length(compatible_device) <= 200),
    supplier TEXT NOT NULL DEFAULT '' CHECK(length(supplier) <= 200),
    purchase_price_baisa INTEGER NOT NULL CHECK(typeof(purchase_price_baisa) = 'integer' AND purchase_price_baisa BETWEEN 0 AND 1000000000),
    selling_price_baisa INTEGER NOT NULL CHECK(typeof(selling_price_baisa) = 'integer' AND selling_price_baisa BETWEEN 0 AND 1000000000),
    storage_area TEXT NOT NULL DEFAULT '' CHECK(length(storage_area) <= 120),
    shelf TEXT NOT NULL DEFAULT '' CHECK(length(shelf) <= 120),
    box TEXT NOT NULL DEFAULT '' CHECK(length(box) <= 120),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE repair_orders (
    id TEXT PRIMARY KEY NOT NULL COLLATE NOCASE CHECK(length(id) BETWEEN 1 AND 100),
    device TEXT NOT NULL CHECK(length(device) BETWEEN 1 AND 200),
    notes TEXT NOT NULL DEFAULT '' CHECK(length(notes) <= 2000),
    created_at TEXT NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE RESTRICT
);

CREATE TABLE stock_movements (
    id TEXT PRIMARY KEY NOT NULL,
    item_id TEXT NOT NULL REFERENCES items(id) ON DELETE RESTRICT,
    kind TEXT NOT NULL CHECK(kind IN ('IN', 'OUT')),
    quantity INTEGER NOT NULL CHECK(typeof(quantity) = 'integer' AND quantity BETWEEN 1 AND 1000000),
    created_at TEXT NOT NULL,
    order_id TEXT REFERENCES repair_orders(id) ON DELETE RESTRICT,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    notes TEXT NOT NULL DEFAULT '' CHECK(length(notes) <= 2000),
    CHECK(order_id IS NULL OR kind = 'OUT')
);

CREATE INDEX idx_items_name ON items(name COLLATE NOCASE);
CREATE INDEX idx_items_category ON items(category COLLATE NOCASE);
CREATE INDEX idx_items_location ON items(storage_area COLLATE NOCASE, shelf COLLATE NOCASE, box COLLATE NOCASE);
CREATE INDEX idx_movements_item_date ON stock_movements(item_id, created_at DESC);
CREATE INDEX idx_movements_order ON stock_movements(order_id);
CREATE INDEX idx_movements_date ON stock_movements(created_at DESC);
CREATE INDEX idx_orders_date ON repair_orders(created_at DESC);

CREATE VIEW stock_balances AS
SELECT i.id AS item_id, COALESCE(SUM(CASE m.kind WHEN 'IN' THEN m.quantity ELSE -m.quantity END), 0) AS quantity
FROM items i LEFT JOIN stock_movements m ON m.item_id = i.id
GROUP BY i.id;

CREATE TRIGGER movement_validate_balance BEFORE INSERT ON stock_movements
BEGIN
    SELECT CASE WHEN NEW.kind = 'OUT' AND NEW.quantity > COALESCE(
        (SELECT quantity FROM stock_balances WHERE item_id = NEW.item_id), 0)
        THEN RAISE(ABORT, 'Insufficient stock: the quantity cannot become negative.') END;
    SELECT CASE WHEN NEW.kind = 'IN' AND NEW.quantity + COALESCE(
        (SELECT quantity FROM stock_balances WHERE item_id = NEW.item_id), 0) > 1000000
        THEN RAISE(ABORT, 'The maximum stock quantity is 1000000.') END;
END;

CREATE TRIGGER movement_no_update BEFORE UPDATE ON stock_movements
BEGIN SELECT RAISE(ABORT, 'Stock movements are permanent and cannot be edited.'); END;

CREATE TRIGGER movement_no_delete BEFORE DELETE ON stock_movements
BEGIN SELECT RAISE(ABORT, 'Stock movements are permanent and cannot be deleted.'); END;

CREATE TRIGGER movement_touch_item AFTER INSERT ON stock_movements
BEGIN UPDATE items SET updated_at = NEW.created_at WHERE id = NEW.item_id; END;

CREATE TRIGGER user_preserve_last_admin BEFORE UPDATE OF role, active ON users
WHEN OLD.role = 'admin' AND OLD.active = 1 AND (NEW.role != 'admin' OR NEW.active != 1)
BEGIN
    SELECT CASE WHEN (SELECT COUNT(*) FROM users WHERE role = 'admin' AND active = 1) <= 1
        THEN RAISE(ABORT, 'At least one active administrator is required.') END;
END;

CREATE TRIGGER user_no_delete BEFORE DELETE ON users
BEGIN SELECT RAISE(ABORT, 'Users cannot be deleted; deactivate the account instead.'); END;

CREATE TRIGGER item_no_delete BEFORE DELETE ON items
BEGIN SELECT RAISE(ABORT, 'Inventory items cannot be deleted.'); END;

CREATE TRIGGER order_no_delete BEFORE DELETE ON repair_orders
BEGIN SELECT RAISE(ABORT, 'Repair orders cannot be deleted.'); END;

PRAGMA user_version = 1;
