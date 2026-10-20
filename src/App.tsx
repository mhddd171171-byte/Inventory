import RecordEditor, { RecordEditContext, type EditTarget } from './RecordEditor';
import { useEffect, useState } from 'react';
import BackupSafety, { DataSafetyNotice } from './BackupSafety';
import { ArrowDownLeft, ArrowUpRight, ArrowRight, Boxes, ChevronRight, Database, Download, FileText, LayoutDashboard, Package, PackageX, Plus, RefreshCw, Search, Settings, Sparkles } from 'lucide-react';
import { call, isDesktop } from './api';
import type { AssistantStatus, Page, Product, Snapshot } from './types';
import { currentMonth, dateTime, matchesProduct, money, monthLabel, stockLabel } from './utils';
import { Dialog, Empty, Notice, ProductTable, SalesTable, StockEntriesTable, SearchBox, errorMessage } from './ui';
import { ProductForm, StockEntryPage, StockOutPage } from './StockForms';
import ReportsPage from './ReportsPage';
import SearchPage from './SearchPage';
import AssistantPanel, { aiLabel } from './AssistantPanel';
import SettingsPanel, { applyTheme, type Preferences } from './SettingsPanel';

const navigation:[Page,typeof Package][] = [['Dashboard',LayoutDashboard],['Products',Boxes],['Out of Stock',PackageX],['Stock Entry',ArrowDownLeft],['Stock Out',ArrowUpRight],['Search',Search],['Reports',FileText],['AI Assistant',Sparkles]];
const descriptions:Record<Page,string> = {
  Dashboard:'Your products, available stock and monthly sales at a glance.',
  Products:'Find products by Item Code, name or model.',
  'Out of Stock':'Every product with no available units. Receive stock to clear an alert.',
  'Stock Entry':'Receive products and preserve the purchase cost of each batch.',
  'Stock Out':'Record the parts sold under an external customer CR number.',
  Reports:'Save PDF and Excel reports locally.',
  Search:'Find a CR, item, product model or stock movement, and export matching records.',
  'AI Assistant':'Ask about inventory and sales, in English or Arabic.',
};
type Modal = {kind:'product';product?:Product}|{kind:'detail';product:Product}|{kind:'backup';importing?:boolean}|{kind:'settings'}|{kind:'edit';target:EditTarget}|null;
export default function App() {
  const [aiBusy,setAiBusy] = useState(false);
  const [outSearch,setOutSearch] = useState('');
  const [page,setPage] = useState<Page>('Dashboard'),[snapshot,setSnapshot] = useState<Snapshot|null>(null),[status,setStatus] = useState<AssistantStatus|null>(null),[error,setError] = useState(''),[search,setSearch] = useState(''),[stockFilter,setStockFilter] = useState('all'),[modal,setModal] = useState<Modal>(null),[backupBusy,setBackupBusy] = useState(false),[backupResult,setBackupResult] = useState('');
  async function refresh() {const next = await call<Snapshot>('snapshot',{month:currentMonth()});setSnapshot(next);}
  useEffect(() => {
    let live = true;
    call<Snapshot>('snapshot',{month:currentMonth()}).then(data => {if(live) setSnapshot(data);}).catch(e => {if(live) setError(errorMessage(e));});
    const poll = () => call<AssistantStatus>('assistant_status').then(data => {if(live) setStatus(data);}).catch(() => {});
    void poll();const timer = setInterval(poll,1000);
    return () => {live = false;clearInterval(timer);};
  },[]);
  useEffect(() => {
    if(!status?.macos_controls) return;
    let live=true,theme:Preferences['theme']='system';
    const media=window.matchMedia('(prefers-color-scheme: dark)');
    const update=()=>applyTheme(theme);
    const changed=(event:Event)=>{theme=(event as CustomEvent<Preferences['theme']>).detail;update();};
    call<Preferences>('settings_get').then(p=>{if(live){theme=p.theme;update();}}).catch(e=>{if(live)setError(errorMessage(e));});
    media.addEventListener('change',update);window.addEventListener('inventory-theme',changed);
    return()=>{live=false;media.removeEventListener('change',update);window.removeEventListener('inventory-theme',changed);};
  },[status?.macos_controls]);
  async function retryAI() {
    setAiBusy(true);setError('');
    try {setStatus(await call<AssistantStatus>('assistant_set_enabled',{enabled:true}));}
    catch(e){setError(errorMessage(e));}finally{setAiBusy(false);}
  }
  async function backup(action:'backup'|'restore') {
    setBackupBusy(true);setBackupResult('');setError('');
    try {const result = await call<{path?:string;restored?:boolean;pre_restore_backup?:string;data_quality?:Snapshot['data_quality'];cancelled?:boolean}>(action);if(!result.cancelled) {if(action === 'restore') {await refresh();setBackupResult(`Backup restored. A copy of the previous database was saved to ${result.pre_restore_backup}.`+(result.data_quality&&Object.values(result.data_quality).some(n=>n>0)?` Review needed: ${result.data_quality.generated_item_codes} automatically assigned Item Codes; ${result.data_quality.stock_entries} stock entries and ${result.data_quality.sale_lines} sales have missing values. Look for the warning signs in the tables.`:''));} else setBackupResult(`Backup saved to ${result.path}.`);}}
    catch(e) {setError(errorMessage(e));} finally {setBackupBusy(false);}
  }
  const shown = snapshot?.products.filter(p => matchesProduct(p,search) && (stockFilter === 'all' || stockFilter === 'low' && p.quantity <= 2 || stockFilter === 'out' && p.quantity === 0)) ?? [];
  const outOfStock = snapshot?.products.filter(p => p.quantity === 0) ?? [];
  const shownOutOfStock = outOfStock.filter(p => matchesProduct(p,outSearch));
  const select = (product:Product) => setModal({kind:'detail',product});
  return <RecordEditContext.Provider value={target=>{setError('');setModal({kind:'edit',target});}}><div className="app-shell">
    <aside className="sidebar"><div className="brand"><span className="brand-icon"><Boxes size={25}/></span><div>Inventory<span className="brand-sub">AL MOHTARIF REPAIR CENTER</span></div></div><div className="workspace-label">WORKSPACE</div><nav>{navigation.map(([name,Icon]) => <button key={name} aria-label={name} className={`nav-item ${page === name ? 'active' : ''}`} onClick={() => {setPage(name);setError('');if(name === 'Out of Stock') setOutSearch('');}}><Icon size={18}/>{name}{name === 'Out of Stock' && outOfStock.length > 0 && <span className="nav-alert-count" data-testid="out-of-stock-count" role="status" aria-label={`${outOfStock.length} out-of-stock ${outOfStock.length === 1 ? 'product' : 'products'}`}>{outOfStock.length}</span>}{name === 'AI Assistant' && <span className="tiny-pill">LOCAL</span>}</button>)}</nav><div className="local-card"><div><i className="status-dot"/><strong>Stored on this computer</strong></div><p>Your inventory and AI work offline.</p><button className="text-link" onClick={() => {setBackupResult('');setModal({kind:'backup'});}}><Database size={14}/>Backup & restore</button></div></aside>
    <main><header className="topbar"><div><Package size={15}/><span>Al Mohtarif Repair Center</span><ChevronRight size={12}/><span>{page}</span></div><div className="topbar-right">{status?.macos_controls&&<button className="btn small" aria-label="Settings" onClick={()=>setModal({kind:'settings'})}><Settings size={15}/>Settings</button>}{status?.retry_available&&<button className="btn small" disabled={aiBusy} onClick={()=>void retryAI()}>Retry AI</button>}<span className={`ai-status ${status?.ready ? '' : status?.running ? 'starting' : 'unavailable'}`}><i className="status-dot"/>{aiLabel(status)}</span><span className="offline"><i className="status-dot"/>Offline</span></div></header><div className="page-content">
      <div className="page-heading"><div><span className="eyebrow">INVENTORY MANAGER</span><h1>{page}</h1><p>{descriptions[page]}</p></div><div className="heading-actions">{page === 'Products' && <button className="btn primary" disabled={!snapshot} onClick={() => setModal({kind:'product'})}><Plus size={16}/>Add product</button>}{page !== 'AI Assistant' && <button className="btn" onClick={() => void refresh().catch(e => setError(errorMessage(e)))}><RefreshCw size={15}/>Refresh</button>}</div></div>
      <Notice text={error}/>{snapshot?.data_quality&&Object.values(snapshot.data_quality).some(n=>n>0)&&<div className="notice migration" role="status">⚠ Some records need review: {snapshot.data_quality.generated_item_codes} automatically assigned Item Codes, {snapshot.data_quality.stock_entries} stock entries and {snapshot.data_quality.sale_lines} sales with missing values. Look for Needs review and use Edit to correct them.</div>}<DataSafetyNotice enabled={Boolean(status?.macos_controls)}/>{!isDesktop && <Notice text="Open the installed desktop application to use your local database."/>}{!snapshot && <div className="panel"><Empty title={error ? 'Database unavailable' : 'Opening your inventory…'} text={error || 'Loading local records.'}/></div>}
      {snapshot && <>
        {snapshot.migration?.message && page === 'Dashboard' && <Notice text={snapshot.migration.message} kind="migration"/>}
        {page === 'Dashboard' && <>
          <div className="stat-grid dashboard-stats">{[['Product types',String(snapshot.summary.total_products)],['Available units',String(snapshot.summary.total_units)],['Inventory purchase value',money(snapshot.summary.inventory_value_baisa)],['Monthly sales revenue',money(snapshot.summary.monthly_sales_baisa)],['Monthly gross profit',money(snapshot.summary.monthly_profit_baisa)]].map(([label,value]) => <div className="stat-card" key={label}><div><span>{label}</span><Package size={16}/></div><strong className="number">{value}</strong><small>{label.startsWith('Monthly') ? monthLabel(snapshot.month) : 'Current inventory'}</small></div>)}</div>
          <div className="quick-strip"><div className="strip-icon"><ArrowDownLeft size={21}/></div><div><h3>Keep your stock up to date</h3><p>Receive batches or record parts sold under a customer CR.</p></div><button className="btn primary" onClick={() => setPage('Stock Entry')}>Stock Entry<ArrowRight size={15}/></button><button className="btn" onClick={() => setPage('Stock Out')}>Stock Out</button></div>
          <div className="panel"><div className="panel-title"><div><h2>Low stock alerts</h2><p>2 units: Low · 1 unit: Critical · 0 units: Out of Stock</p></div><button className="text-link" onClick={() => setPage('Products')}>View products<ArrowRight size={14}/></button></div>{snapshot.low_stock.length ? <ProductTable products={snapshot.low_stock} select={select}/> : <Empty title={snapshot.products.length ? 'Stock levels look healthy' : 'Your inventory is empty'} text={snapshot.products.length ? 'Every product has more than two units available.' : 'Start by receiving a product through Stock Entry.'}><button className="btn" onClick={() => setPage('Stock Entry')}>Open Stock Entry</button></Empty>}</div>
          <div className="panel dashboard-sales"><div className="panel-title"><div><h2>Recent sales</h2><p>CR-linked sales and recorded gross profit.</p></div></div>{snapshot.sale_lines.length ? <SalesTable lines={snapshot.sale_lines.slice(0,8)}/> : <Empty title="No sales recorded" text="Use Stock Out to record quantities, CR numbers and actual selling prices."/>}</div>
        </>}
        {page === 'Products' && <div className="panel"><div className="filters"><SearchBox value={search} onChange={setSearch}/><select aria-label="Stock filter" value={stockFilter} onChange={e => setStockFilter(e.target.value)}><option value="all">All stock levels</option><option value="low">Low / Critical / Out</option><option value="out">Out of Stock</option></select><span className="count">{shown.length} products</span></div>{shown.length ? <ProductTable products={shown} select={select}/> : <Empty title="No matching products" text="Search by name or model; an Item Code is not required. Create a product or receive it through Stock Entry."/>}<div className="table-footer">All quantities and values reflect saved stock transactions.</div></div>}
        {page === 'Out of Stock' && <div className="panel"><div className="panel-title"><div><h2>Out-of-stock products</h2><p>{outOfStock.length} {outOfStock.length === 1 ? 'product has' : 'products have'} no available units.</p></div></div><div className="filters"><SearchBox value={outSearch} onChange={setOutSearch}/><span className="count">{shownOutOfStock.length} {shownOutOfStock.length === 1 ? 'product' : 'products'}</span></div>{shownOutOfStock.length ? <ProductTable products={shownOutOfStock} select={select}/> : <Empty title={outOfStock.length ? 'No matching out-of-stock products' : 'No out-of-stock items'} text={outOfStock.length ? 'Try another Item Code, name or model, or clear the search to see all out-of-stock products.' : snapshot.products.length ? 'Every product has stock available.' : 'Your inventory is empty. Add a product or receive stock through Stock Entry.'}/>}<div className="table-footer">Products leave this list automatically when you receive stock through Stock Entry.</div></div>}
        {page === 'Stock Entry' && <StockEntryPage snapshot={snapshot} saved={refresh}/>}
        {page === 'Stock Out' && <StockOutPage snapshot={snapshot} saved={refresh}/>}
        {page === 'Reports' && <ReportsPage editInventory={()=>setPage('Products')} editSales={()=>setPage('Stock Out')}/>}
        {page === 'Search' && <SearchPage snapshot={snapshot} select={select}/>}
      </>}
      <section hidden={page !== 'AI Assistant'}><AssistantPanel visible={page === 'AI Assistant'} status={status}/></section>
      <footer className="page-footer"><span>Al Mohtarif Repair Center · Inventory Manager</span><span><Database size={12}/>Local SQLite · OMR</span></footer>
    </div></main>
    {modal?.kind === 'edit' && <RecordEditor key={`${modal.target.kind}:${modal.target.record.id}`} target={modal.target} products={snapshot?.products??[]} saved={refresh} close={()=>setModal(null)}/>}
    {modal?.kind === 'product' && <ProductForm product={modal.product} saved={refresh} close={() => setModal(null)}/>}
    {modal?.kind === 'detail' && <Dialog title="Product details" close={() => setModal(null)}><div className="dialog-body"><h2 dir="auto">{modal.product.name}</h2><p className="muted" dir="auto">{modal.product.model_description}</p><dl className="details"><dt>Item Code</dt><dd>{modal.product.item_code}</dd><dt>Available units</dt><dd>{modal.product.quantity}</dd><dt>Purchase value</dt><dd>{money(modal.product.inventory_value_baisa)}</dd><dt>Stock status</dt><dd>{stockLabel(modal.product.quantity)}</dd></dl><h3>Stock entry history</h3><StockEntriesTable entries={snapshot?.entries.filter(e=>e.product_id===modal.product.id)??[]}/><h3>Stock Out history</h3><SalesTable lines={snapshot?.sale_lines.filter(l => l.product_id === modal.product.id) ?? []}/></div><div className="dialog-actions"><button className="btn" onClick={() => setModal(null)}>Close</button></div></Dialog>}
    {modal?.kind === 'settings' && <Dialog title="Settings" close={()=>setModal(null)}><SettingsPanel status={status} onStatus={setStatus} onImport={()=>{setError('');setBackupResult('');setModal({kind:'backup',importing:true});}}/><div className="dialog-actions"><button className="btn" onClick={()=>setModal(null)}>Close</button></div></Dialog>}
    {modal?.kind === 'backup' && <Dialog title={modal.importing ? "Import / restore database" : "Local database backup & restore"} close={() => {if(!backupBusy) setModal(null);}}><div className="dialog-body"><Notice text={error}/><Notice text={backupResult} kind="success"/><p>Create a backup of all products, stock batches and CR sales. Keep backups in a safe location on your computer or removable drive.</p><button className="btn primary" disabled={backupBusy} onClick={() => void backup('backup')}><Download size={16}/>Create backup</button>{status?.macos_controls&&!modal.importing&&<BackupSafety/>}<h2>Restore a backup</h2><p>Choose an Inventory Manager .db file. Excel and CSV files are not supported. Restore replaces the current database with the selected backup; it does not merge records. The application validates it and saves a recoverable copy of the current database first.</p><button className="btn" disabled={backupBusy} onClick={() => void backup('restore')}>{modal.importing ? "Choose database (.db) to restore" : "Choose backup to restore"}</button></div><div className="dialog-actions"><button className="btn" disabled={backupBusy} onClick={() => setModal(null)}>Close</button></div></Dialog>}
  </div></RecordEditContext.Provider>;
}
