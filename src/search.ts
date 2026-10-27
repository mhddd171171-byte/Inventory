import { financialLines } from './movements';
import type { Product, SaleLine, Snapshot, StockEntry } from './types';
import { normalize, numericReference } from './utils';

export type SearchFilters = { query: string; cr: string; itemCode: string; from: string; to: string };
export const emptySearch: SearchFilters = { query:'', cr:'', itemCode:'', from:'', to:'' };
export type SearchResults = { products:Product[]; sales:SaleLine[]; entries:StockEntry[]; filters:SearchFilters };

export function searchSnapshot(snapshot:Snapshot, input:SearchFilters):SearchResults {
  if(input.query.length > 200) throw new Error('Search text must be at most 200 characters.');
  const filters = {...input, query:input.query.trim(), cr:input.cr.trim() ? numericReference(input.cr,'CR number') : '', itemCode:input.itemCode.trim() ? numericReference(input.itemCode) : ''};
  for(const date of [filters.from,filters.to]) if(date && (!/^\d{4}-\d{2}-\d{2}$/.test(date) || !Number.isFinite(Date.parse(`${date}T12:00:00Z`)) || new Date(`${date}T12:00:00Z`).toISOString().slice(0,10) !== date)) throw new Error('Choose a valid date.');
  if(filters.from && filters.to && filters.from > filters.to) throw new Error('Start date must be before or equal to end date.');
  const terms = normalize(filters.query).split(/\s+/).filter(Boolean);
  const textMatches = (...fields:(string|null|undefined)[]) => {const haystack=normalize(fields.filter(Boolean).join(' '));return terms.every(term=>haystack.includes(term));};
  const inRange = (stamp:string) => {const d=new Date(stamp);if(!Number.isFinite(d.getTime())) return false;const local=`${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,'0')}-${String(d.getDate()).padStart(2,'0')}`;return (!filters.from || local>=filters.from)&&(!filters.to || local<=filters.to);};
  const sales = financialLines(snapshot.sale_lines).filter(l => (!filters.cr || l.cr_number===filters.cr) && (!filters.itemCode || l.item_code===filters.itemCode) && inRange(l.created_at) && textMatches(l.cr_number,l.item_code,l.product_name,l.model_description,l.legacy_reference));
  const byId = new Map(snapshot.products.map(p=>[p.id,p]));
  // CR belongs to a sale, never an incoming delivery. Both exact filters must
  // match the SAME sale line, including repeated CRs across separate sales.
  const entries = filters.cr ? [] : snapshot.entries.filter(e => (!filters.itemCode || e.item_code===filters.itemCode) && inRange(e.created_at) && textMatches(e.item_code,e.product_name,byId.get(e.product_id)?.model_description));
  const linked = new Set([...sales,...entries].map(row=>row.product_id));
  const products = snapshot.products.filter(p => (!filters.itemCode || p.item_code===filters.itemCode) &&
    (filters.cr || filters.from || filters.to ? linked.has(p.id) : textMatches(p.item_code,p.name,p.model_description) || sales.some(l=>l.product_id===p.id)));
  return {products, sales:sales.sort((a,b)=>b.created_at.localeCompare(a.created_at)), entries:entries.sort((a,b)=>b.created_at.localeCompare(a.created_at)), filters};
}

export function searchSuggestions(snapshot:Snapshot) {
  return { cr:[...new Set(snapshot.sale_lines.map(l=>l.cr_number).filter((v):v is string=>!!v))].slice(0,20),
    codes:snapshot.products.slice(0,20).map(p=>p.item_code), names:[...new Set(snapshot.products.flatMap(p=>[p.name,p.model_description]))].slice(0,30) };
}
