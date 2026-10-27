import { DataIssues, useRecordEditor } from './RecordEditor';
import { useEffect, useRef } from 'react';
import type { ReactNode } from 'react';
import { Package, Search, X } from 'lucide-react';
import type { Product, Snapshot, StockEntry } from './types';
import { batchBuyingTotal, dateTime, money, stockLabel } from './utils';
export const errorMessage = (error: unknown) => error instanceof Error ? error.message : String(error);
export function Field({label, children, wide = false}: {label: string; children: ReactNode; wide?: boolean}) {
  return <label className={`field ${wide ? 'wide' : ''}`}><span>{label}</span>{children}</label>;
}
export function Empty({title, text, children}: {title: string; text: string; children?: ReactNode}) {
  return <div className="empty"><Package size={30}/><h3>{title}</h3><p>{text}</p>{children}</div>;
}
export function Notice({text, kind = 'error'}: {text: string; kind?: string}) {
  return text ? <div className={`notice ${kind}`} role={kind === 'error' ? 'alert' : 'status'}>{text}</div> : null;
}
export function StockTag({quantity}: {quantity: number}) {
  return <span className={`tag stock-${quantity > 2 ? 'normal' : quantity}`}><span className="status-dot"/>{stockLabel(quantity)}</span>;
}
export function Dialog({title, close, children}: {title: string; close: () => void; children: ReactNode}) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => { ref.current?.showModal(); }, []);
  return <dialog ref={ref} onCancel={event=>{event.preventDefault();close();}} onClick={event => {if(event.target === ref.current) close();}}><div className="dialog-head"><h2>{title}</h2><button className="icon-btn" aria-label="Close dialog" onClick={close}><X size={20}/></button></div>{children}</dialog>;
}
export function SearchBox({value, onChange, label = 'Search by Item Code, name or model'}: {value: string; onChange: (value: string) => void; label?: string}) {
  return <div className="searchbox"><Search size={17}/><input aria-label={label} placeholder={label} value={value} onChange={e => onChange(e.target.value)}/>{value && <button className="icon-btn" aria-label="Clear search" onClick={() => onChange('')}><X size={14}/></button>}</div>;
}
export function ProductTable({products, select, edit}: {products: Product[]; select: (p: Product) => void; edit?: (p: Product) => void}) {
  const openEditor=useRecordEditor();const editProduct=edit??(openEditor?((p:Product)=>openEditor({kind:'product',record:p})):undefined);
  return <div className="table-scroll"><table className="inventory-table"><thead><tr><th>ITEM CODE</th><th>PRODUCT / MODEL</th><th>AVAILABLE</th><th>PURCHASE VALUE</th><th>STATUS</th>{editProduct && <th>ACTIONS</th>}</tr></thead><tbody>{products.map(p => <tr key={p.id}><td className="number">{p.item_code}</td><td><button className="text-link" dir="auto" onClick={() => select(p)}>{p.name}</button><small dir="auto">{p.model_description}</small><DataIssues issues={p.data_issues}/></td><td className="number">{p.quantity}</td><td className="number">{money(p.inventory_value_baisa)}</td><td><StockTag quantity={p.quantity}/></td>{editProduct && <td><button className="btn small" aria-label={`Edit product ${p.item_code}`} onClick={() => editProduct(p)}>Edit</button></td>}</tr>)}</tbody></table></div>;
}
export function SalesTable({lines}: {lines: Snapshot['sale_lines']}) {
  const edit=useRecordEditor();
  return <div className="table-scroll"><table><thead><tr><th>DATE / TIME</th><th>CR NUMBER</th><th>PRODUCT</th><th>QTY</th><th>SELLING PRICE / UNIT</th><th>TOTAL SOLD FOR</th><th title="Buying cost of the sold units, using the oldest incoming batches first (FIFO).">TOTAL BUYING COST</th><th>GROSS PROFIT</th>{edit&&<th>ACTIONS</th>}</tr></thead><tbody>{lines.map(l => <tr key={l.id}><td className="muted nowrap">{dateTime(l.created_at)}</td><td>{l.cr_number ?? (l.purpose==='personal'?'Personal use':l.purpose==='damage'?'Damage':`Legacy: ${l.legacy_reference || 'not recorded'}`)}</td><td dir="auto">{l.product_name}<small>Item Code {l.item_code}</small>{l.purpose&&l.purpose!=='sale'&&<small className="movement-label">{l.purpose==='personal'?'Personal use':l.purpose==='damage'?'Damage':l.purpose==='replacement'?'Replacement':'Return settlement'}</small>}{!!l.returned_quantity&&<small className="movement-label">{l.returned_quantity} damaged return · {l.returns?.some(r=>r.resolution==='pending')?'Pending resolution':'Resolved'}</small>}<DataIssues issues={l.data_issues}/></td><td>{l.quantity}</td><td className="number">{['personal','damage','return_settlement'].includes(l.purpose??'sale')?'—':money(l.unit_price_baisa)}</td><td className="number">{money(l.revenue_baisa)}</td><td className="number">{money(l.cost_baisa)}</td><td className="number">{['personal','damage'].includes(l.purpose??'sale')?'—':money(l.profit_baisa)}</td>{edit&&<td>{l.purpose!=='return_settlement'&&<button className="btn small" aria-label={`Edit Stock Out ${l.cr_number??l.legacy_reference??'missing CR'} item ${l.item_code}`} onClick={()=>edit({kind:'sale',record:l})}>Edit</button>}</td>}</tr>)}</tbody></table></div>;
}
export function StockEntriesTable({entries}:{entries:StockEntry[]}) {
  const edit=useRecordEditor();
  return <div className="table-scroll"><table><thead><tr><th>DATE / TIME</th><th>ITEM CODE</th><th>PRODUCT</th><th>RECEIVED</th><th>REMAINING</th><th>BUYING PRICE / UNIT</th><th>TOTAL BOUGHT FOR</th>{edit&&<th>ACTIONS</th>}</tr></thead><tbody>{entries.map(e=><tr key={e.id}><td>{dateTime(e.created_at)}</td><td>{e.item_code}</td><td dir="auto">{e.product_name}<DataIssues issues={e.data_issues}/></td><td>{e.quantity}</td><td>{e.remaining_quantity}</td><td>{money(e.unit_cost_baisa)}</td><td>{money(batchBuyingTotal(e))}</td>{edit&&<td><button className="btn small" aria-label={`Edit stock entry ${e.item_code}`} onClick={()=>edit({kind:'entry',record:e})}>Edit</button></td>}</tr>)}</tbody></table></div>;
}
