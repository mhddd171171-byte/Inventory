import type { Product } from './types';

export function formatBaisa(value: bigint | number): string {
  if (typeof value === 'number' && !Number.isSafeInteger(value)) throw new Error('Invalid monetary value in baisa.');
  const amount = BigInt(value), absolute = amount < 0n ? -amount : amount;
  return `${amount < 0n ? '-' : ''}${(absolute / 1000n).toLocaleString('en-GB')}.${(absolute % 1000n).toString().padStart(3, '0')}`;
}
export const money = (baisa: number | bigint | null | undefined) => baisa == null ? 'Unknown' : `OMR ${formatBaisa(baisa)}`;
export const batchBuyingTotal = (entry: {quantity:number;unit_cost_baisa:number|null}):bigint|null => entry.unit_cost_baisa == null ? null : BigInt(entry.quantity)*BigInt(entry.unit_cost_baisa);
export const dateTime = (date: string) => new Date(date).toLocaleString('en-GB', { day: '2-digit', month: 'short', year: 'numeric', hour: '2-digit', minute: '2-digit' });
export const currentMonth = () => { const date = new Date(); return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}`; };
export const monthLabel = (month: string) => new Date(`${month}-01T12:00:00`).toLocaleDateString('en-GB', { month: 'long', year: 'numeric' });
export const stockLabel = (quantity: number) => quantity === 0 ? 'Out of Stock' : quantity === 1 ? 'Critical Stock' : quantity === 2 ? 'Low Stock' : 'In Stock';
const asciiDigits = (value: string) => value.replace(/[٠-٩۰-۹]/g, c => String(c.charCodeAt(0) - (c.charCodeAt(0) >= 0x6f0 ? 0x6f0 : 0x660)));
export function numericReference(value: string, label = 'Item Code'): string {
  const normalized = asciiDigits(value.trim()).replace(/^0+/, '');
  if (!/^\d{1,30}$/.test(normalized)) throw new Error(`${label} must be a nonzero numeric reference (up to 30 digits).`);
  return normalized;
}
export function findProduct(products: Product[], code: string): Product | undefined {
  try { const normalized = numericReference(code); return products.find(p => p.item_code === normalized); } catch { return undefined; }
}
export function positiveQuantity(value: string): number {
  const text = asciiDigits(value.trim());
  if (!/^\d+$/.test(text) || !Number.isSafeInteger(Number(text)) || Number(text) < 1 || Number(text) > 1_000_000) throw new Error('Quantity must be a positive whole number, up to 1,000,000.');
  return Number(text);
}
export function toBaisa(value: string): number {
  const text = asciiDigits(value.trim()).replace('٫', '.');
  if (!/^\d+(\.\d{1,3})?$/.test(text)) throw new Error('Enter a nonnegative OMR price with up to 3 decimal places.');
  const [whole, fraction = ''] = text.split('.');
  const baisa = BigInt(whole) * 1000n + BigInt(fraction.padEnd(3, '0'));
  if (baisa > 1_000_000_000n) throw new Error('Price must not exceed OMR 1,000,000.000.');
  return Number(baisa);
}
export const normalize = (value: string) => asciiDigits(value).toLocaleLowerCase().normalize('NFKD').replace(/[\u0300-\u036f\u064b-\u065f\u0670\u0640]/g, '').replace(/[أإآٱ]/g, 'ا').replace(/ى/g, 'ي');
export const matchesProduct = (product: Product, query: string) => {
  const haystack = normalize([product.item_code, product.name, product.model_description].join(' '));
  return normalize(query).trim().split(/\s+/).every(term => haystack.includes(term));
};
