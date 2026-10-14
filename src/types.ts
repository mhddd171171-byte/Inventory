export type AssistantStatus = {
  configured: boolean; running: boolean; ready: boolean;
  state: 'unconfigured' | 'stopped' | 'starting' | 'ready' | 'recovering' | 'failed';
  message: string; voice_configured: boolean; voice_ready: boolean; voice_message?: string;
  voice_interrupted?: boolean;
};
export type VoiceStatus = { configured: boolean; ready: boolean; active: boolean; message: string };
export type Product = {
  id: string; item_code: string; name: string; model_description: string; quantity: number;
  inventory_value_baisa: number | null; unknown_cost_units: number;
  stock_status: 'normal' | 'low' | 'critical' | 'out'; created_at: string; updated_at: string;
};
export type StockEntry = {
  id: string; product_id: string; item_code: string; product_name: string;
  quantity: number; remaining_quantity: number; unit_cost_baisa: number | null; created_at: string;
};
export type Sale = {
  id: string; cr_number: string | null; created_at: string; total_quantity: number;
  revenue_baisa: number | null; cost_baisa: number | null; profit_baisa: number | null;
  legacy?: boolean; legacy_reference?: string;
};
export type SaleLine = {
  id: string; sale_id: string; product_id: string; item_code: string; product_name: string;
  model_description: string; cr_number: string | null; quantity: number;
  unit_price_baisa: number | null; revenue_baisa: number | null; cost_baisa: number | null;
  profit_baisa: number | null; created_at: string; legacy?: boolean; legacy_reference?: string;
};
export type MonthlyProduct = {
  product_id: string; item_code: string; product_name: string; quantity_sold: number;
  quantity_remaining: number; cost_baisa: number | null; revenue_baisa: number | null; profit_baisa: number | null;
};
export type Snapshot = {
  month: string; products: Product[]; entries: StockEntry[]; sales: Sale[]; sale_lines: SaleLine[];
  monthly_sales_lines: SaleLine[]; monthly_summary: MonthlyProduct[]; low_stock: Product[];
  summary: {
    total_products: number; total_units: number; inventory_value_baisa: number | null;
    known_inventory_value_baisa: number; unknown_cost_units: number; monthly_units_sold: number;
    monthly_sales_baisa: number | null; known_monthly_sales_baisa: number;
    monthly_cost_baisa: number | null; monthly_profit_baisa: number | null; unknown_sales_lines: number;
  };
  migration?: { legacy_products: number; unknown_cost_units: number; message: string;
    code_changes: {old_code: string; item_code: string; product_name: string}[] };
};
export type Page = 'Dashboard' | 'Products' | 'Out of Stock' | 'Stock Entry' | 'Stock Out' | 'Search' | 'Reports' | 'AI Assistant';
