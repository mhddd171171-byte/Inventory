/// <reference types="vite/client" />
import { jsPDF } from 'jspdf';
import { autoTable, type CellInput, type UserOptions } from 'jspdf-autotable';
import ExcelJS from 'exceljs';
import regularFontUrl from './assets/NotoSansArabic-Regular.ttf?url';
import boldFontUrl from './assets/NotoSansArabic-Bold.ttf?url';
import { batchBuyingTotal, formatBaisa, money, monthLabel, stockLabel } from './utils';
import type { Snapshot } from './types';
import type { SearchResults } from './search';
export { formatBaisa } from './utils';
export type ReportType = 'monthly_sales' | 'current_inventory' | 'monthly_inventory_summary';
export type ReportSnapshot = Snapshot;
export interface ReportOptions { month?: string }
export interface ReportTable {
  title: string; headings: string[]; rows: string[][]; widths: number[]; numericColumns?: number[]; moneyColumns?: number[];
}
export interface ReportModel {
  title: string; generatedAt: string; period: string; notes: string[]; summary: string[];
  tables: ReportTable[];
}
export interface ReportFonts { regular: string; bold: string }
export const reportTitles: Record<ReportType, string> = {
  monthly_sales: 'Monthly Sales Report', current_inventory: 'Current Inventory Report', monthly_inventory_summary: 'Monthly Inventory Summary',
};
const display = (value: unknown) => String(value ?? '').replace(/[\u0000-\u0008\u000b\u000c\u000e-\u001f]/g, '').trim() || '—';
const amount = (value: number | null) => value == null ? 'Unknown' : formatBaisa(value);
const stamp = (value: string) => {
  const date = new Date(value);
  if (!Number.isFinite(date.getTime())) throw new Error(`Invalid record date: ${value}`);
  return date.toLocaleString('en-GB', { year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', hour12: false });
};

/** The backend supplies an atomic SQLite snapshot for the requested calendar month. */
export function createReportModel(type: ReportType, snapshot: ReportSnapshot, options: ReportOptions = {}, now = new Date()): ReportModel {
  if (!(type in reportTitles)) throw new Error('Unknown report type.');
  const month = options.month ?? snapshot.month;
  if (!/^\d{4}-(0[1-9]|1[0-2])$/.test(month)) throw new Error('Select a valid reporting month.');
  if (type !== 'current_inventory' && snapshot.month !== month) throw new Error('Report data does not match the selected month. Refresh and try again.');
  const model: ReportModel = {
    title: reportTitles[type], generatedAt: stamp(now.toISOString()),
    period: type === 'current_inventory' ? 'Current balances at generation time' : monthLabel(month),
    notes: ['All monetary amounts are OMR (1 OMR = 1,000 baisa). Buying cost of sold units uses the oldest batches first (FIFO).'], summary: [], tables: [],
  };
  const summary = snapshot.summary;
  if (type === 'current_inventory') {
    model.tables.push({ title: 'Available stock', headings: ['Item Code', 'Product Name / Model', 'Available Quantity', 'Inventory Purchase Value\nOMR', 'Stock Status'], widths: [30, 110, 35, 50, 44], numericColumns: [2, 3], moneyColumns: [3], rows: snapshot.products.map(p => [p.item_code, `${p.name}\n${p.model_description}`, String(p.quantity), amount(p.inventory_value_baisa), stockLabel(p.quantity)]) });
    model.summary = [`Product types: ${summary.total_products}`, `Available units: ${summary.total_units}`, `Inventory purchase value: ${money(summary.inventory_value_baisa)}`];
    if (summary.unknown_cost_units) model.notes.push(`${summary.unknown_cost_units} migrated units have no recorded historical purchase cost. Their value is Unknown; known inventory value is ${money(summary.known_inventory_value_baisa)}.`);
  } else {
    model.summary = [`Units sold: ${summary.monthly_units_sold}`, `Buying cost of sold units: ${money(summary.monthly_cost_baisa)}`, `Sales revenue: ${money(summary.monthly_sales_baisa)}`, `Gross profit: ${money(summary.monthly_profit_baisa)}`];
    if (summary.unknown_sales_lines) model.notes.push(`${summary.unknown_sales_lines} migrated sale lines lack historical cost or actual selling price. Unknown amounts are never treated as zero.`);
    if (type === 'monthly_sales') {
      model.tables.push({ title: 'Sales recorded during the selected month', headings: ['Date / Time', 'Item Code', 'Product Name', 'Qty', 'CR Number', 'Actual Selling\nOMR / unit', 'Buying Cost\nOMR total', 'Total Sales\nOMR', 'Gross Profit\nOMR'], widths: [32, 25, 60, 14, 25, 29, 29, 29, 29], numericColumns: [3, 5, 6, 7, 8], moneyColumns: [5, 6, 7, 8], rows: snapshot.monthly_sales_lines.map(l => [stamp(l.created_at), l.item_code, l.product_name, String(l.quantity), l.cr_number ?? `Legacy: ${l.legacy_reference || 'not recorded'}`, amount(l.unit_price_baisa), amount(l.cost_baisa), amount(l.revenue_baisa), amount(l.profit_baisa)]) });
    } else {
      model.tables.push({ title: 'Sales by product and current stock remaining', headings: ['Item Code', 'Product Name', 'Quantity Sold', 'Quantity Remaining', 'Buying Cost\nOMR', 'Sales Revenue\nOMR', 'Gross Profit\nOMR'], widths: [30, 78, 28, 30, 35, 35, 35], numericColumns: [2, 3, 4, 5, 6], moneyColumns: [4, 5, 6], rows: snapshot.monthly_summary.map(p => [p.item_code, p.product_name, String(p.quantity_sold), String(p.quantity_remaining), amount(p.cost_baisa), amount(p.revenue_baisa), amount(p.profit_baisa)]) });
      model.notes.push('Quantity sold and financial totals refer to the selected month. Quantity remaining is the current available balance.');
    }
  }
  return model;
}
let fontPromise: Promise<ReportFonts> | undefined;

/** Search exports use the identical filtered rows shown by the workspace search. */
export function createSearchReportModel(results:SearchResults, now=new Date()):ReportModel {
  const f=results.filters;
  const total=(key:'revenue_baisa'|'cost_baisa'|'profit_baisa')=>{
    if(results.sales.some(row=>row[key]==null)) return 'Unknown';
    const value=results.sales.reduce((sum,row)=>sum+BigInt(row[key]!),0n);
    return `OMR ${formatBaisa(value)}`;
  };
  const criteria=[f.cr&&`CR: ${f.cr}`,f.itemCode&&`Item Code: ${f.itemCode}`,f.query&&`Text: ${f.query}`].filter(Boolean).join(' | ')||'All saved records';
  const model:ReportModel={title:'Search Results',generatedAt:stamp(now.toISOString()),period:`${f.from||'First recorded date'} to ${f.to||'Latest recorded date'}`,notes:[`Filters: ${criteria}`,'All filters apply together. CR identifies sold items, not incoming deliveries.','Money is OMR. Buying cost of sold units uses the oldest batches first (FIFO); selling prices are actual recorded prices. Product balances and batch remaining quantities are current.'],summary:[`Sales lines: ${results.sales.length}`,`Units sold: ${results.sales.reduce((sum,l)=>sum+BigInt(l.quantity),0n)}`,`Sales revenue: ${total('revenue_baisa')}`,`Buying cost of sold units: ${total('cost_baisa')}`,`Gross profit: ${total('profit_baisa')}`],tables:[]};
  model.tables.push({title:'Items sold',headings:['Date / Time','CR Number','Item Code','Product Name / Model','Qty','Selling OMR / unit','Buying Cost OMR total','Sold For OMR total','Profit OMR'],widths:[30,26,26,64,12,27,28,28,28],numericColumns:[4,5,6,7,8],moneyColumns:[5,6,7,8],rows:results.sales.map(l=>[stamp(l.created_at),l.cr_number??`Legacy: ${l.legacy_reference||'not recorded'}`,l.item_code,`${l.product_name}\n${l.model_description}`,String(l.quantity),amount(l.unit_price_baisa),amount(l.cost_baisa),amount(l.revenue_baisa),amount(l.profit_baisa)])});
  model.tables.push({title:'Current matching products',headings:['Item Code','Product Name / Model','Available Qty','Purchase Value OMR','Status'],widths:[30,110,35,50,44],numericColumns:[2,3],moneyColumns:[3],rows:results.products.map(p=>[p.item_code,`${p.name}\n${p.model_description}`,String(p.quantity),amount(p.inventory_value_baisa),stockLabel(p.quantity)])});
  if(!f.cr) model.tables.push({title:'Incoming stock batches',headings:['Date / Time','Item Code','Product','Received','Remaining','Buying OMR / unit','Bought For OMR total'],widths:[29,24,74,24,24,38,52],numericColumns:[3,4,5,6],moneyColumns:[5,6],rows:results.entries.map(e=>[stamp(e.created_at),e.item_code,e.product_name,String(e.quantity),String(e.remaining_quantity),amount(e.unit_cost_baisa),batchBuyingTotal(e)==null?'Unknown':formatBaisa(batchBuyingTotal(e)!)])});
  return model;
}
async function loadFonts(): Promise<ReportFonts> {
  if (!fontPromise) {
    const load = async (url: string) => {
      // Vite emits these as packaged local assets; no CDN or external font service is used.
      const response = await fetch(url);
      if (!response.ok) throw new Error('The bundled report font could not be loaded.');
      const bytes = new Uint8Array(await response.arrayBuffer());
      let binary = '';
      for (let offset = 0; offset < bytes.length; offset += 8192) binary += String.fromCharCode(...bytes.subarray(offset, offset + 8192));
      return btoa(binary);
    };
    fontPromise = Promise.all([load(regularFontUrl), load(boldFontUrl)]).then(([regular, bold]) => ({ regular, bold })).catch(error => { fontPromise = undefined; throw error; });
  }
  return fontPromise;
}

/** Draws a report with embedded, locally supplied fonts; also used by the PDF integration tests. */
export function renderReport(model: ReportModel, fonts: ReportFonts): Uint8Array {
  const doc = new jsPDF({ orientation: 'landscape', unit: 'mm', format: 'a4', compress: true, putOnlyUsedFonts: true });
  doc.addFileToVFS('NotoSansArabic-Regular.ttf', fonts.regular);
  doc.addFileToVFS('NotoSansArabic-Bold.ttf', fonts.bold);
  doc.addFont('NotoSansArabic-Regular.ttf', 'Inventory', 'normal');
  doc.addFont('NotoSansArabic-Bold.ttf', 'Inventory', 'bold');
  doc.setFont('Inventory', 'normal');
  doc.setProperties({ title: model.title, subject: 'Local inventory management report', author: 'Inventory Manager', creator: 'Inventory Manager - offline reporting' });
  doc.setLineHeightFactor(1.45);
  const width = doc.internal.pageSize.getWidth();
  const height = doc.internal.pageSize.getHeight();
  const margin = 14;
  const usableWidth = width - margin * 2;
  const arabic = /[\u0600-\u06ff\u0750-\u077f\u08a0-\u08ff\ufb50-\ufdff\ufe70-\ufeff]/;
  const headedPages = new Set<number>();
  function header() {
    const page = doc.getCurrentPageInfo().pageNumber;
    if (headedPages.has(page)) return;
    headedPages.add(page);
    doc.setDrawColor(27, 115, 100).setLineWidth(1).line(margin, 13, width - margin, 13);
    doc.setFont('Inventory', 'bold').setFontSize(17).setTextColor(20, 43, 53);
    doc.text(model.title, margin, 25);
    doc.setFont('Inventory', 'normal').setFontSize(8).setTextColor(81, 96, 105);
    doc.text(`Generated: ${model.generatedAt} | Times shown in this computer's local timezone`, margin, 33);
    doc.text(`Period: ${model.period}`, margin, 39);
  }
  header();
  let y = 47;
  function paragraph(text: string, fontSize = 8, bold = false) {
    doc.setFont('Inventory', bold ? 'bold' : 'normal').setFontSize(fontSize).setTextColor(42, 57, 66);
    const lines = doc.splitTextToSize(display(text), usableWidth) as string[];
    const lineHeight = fontSize * 0.352778 * 1.45;
    for (const line of lines) {
      if (y + lineHeight > height - 19) { doc.addPage(); header(); y = 48; }
      doc.text(line, arabic.test(line) ? width - margin : margin, y, { align: arabic.test(line) ? 'right' : 'left' });
      y += lineHeight;
    }
    y += 2;
  }
  for (const note of model.notes) paragraph(note);
  paragraph(model.summary.join('    |    '), 8.5, true);
  y += 2;
  for (const table of model.tables) {
    if (y > height - 45) { doc.addPage(); header(); y = 48; }
    paragraph(table.title, 10, true);
    const denominator = table.widths.reduce((a, b) => a + b, 0);
    const columnStyles: NonNullable<UserOptions['columnStyles']> = {};
    table.widths.forEach((w, i) => { columnStyles[i] = { cellWidth: w / denominator * usableWidth, halign: table.numericColumns?.includes(i) ? 'right' : 'left' }; });
    const body: CellInput[][] = table.rows.length ? table.rows.map(row => row.map(display)) : [[{ content: 'No records match this report.', colSpan: table.headings.length, styles: { halign: 'center', textColor: [89, 105, 115], cellPadding: 6 } }]];
    autoTable(doc, {
      startY: y, margin: { top: 47, bottom: 19, left: margin, right: margin },
      head: [table.headings], body, theme: 'striped', showHead: 'everyPage', rowPageBreak: 'avoid',
      styles: { font: 'Inventory', fontSize: 8, cellPadding: { top: 3, right: 2.4, bottom: 3, left: 2.4 }, lineColor: [225, 232, 235], lineWidth: { bottom: 0.15 }, textColor: [34, 50, 60], valign: 'top', overflow: 'linebreak' },
      headStyles: { fillColor: [25, 89, 81], textColor: [255, 255, 255], fontStyle: 'bold', minCellHeight: 12 },
      alternateRowStyles: { fillColor: [243, 247, 248] }, columnStyles,
      didParseCell: data => {
        if (data.cell.text.some(text => arabic.test(text))) data.cell.styles.halign = 'right';
      },
      willDrawPage: () => { header(); },
    });
    y = (doc as jsPDF & { lastAutoTable: { finalY: number } }).lastAutoTable.finalY + 9;
  }
  const pages = doc.getNumberOfPages();
  for (let page = 1; page <= pages; page++) {
    doc.setPage(page);
    doc.setDrawColor(213, 224, 227).setLineWidth(0.2).line(margin, height - 14, width - margin, height - 14);
    doc.setFont('Inventory', 'normal').setFontSize(7).setTextColor(94, 112, 119);
    doc.text('Inventory Manager | Generated locally | OMR', margin, height - 9);
    doc.text(`Page ${page} of ${pages}`, width - margin, height - 9, { align: 'right' });
  }
  return new Uint8Array(doc.output('arraybuffer'));
}

/** Builds an offline PDF; the native backend presents the local Save As dialog. */
export async function buildReport(type: ReportType, snapshot: ReportSnapshot, options: ReportOptions = {}): Promise<Uint8Array> {
  return renderReport(createReportModel(type, snapshot, options), await loadFonts());
}
export async function buildModelPdf(model:ReportModel):Promise<Uint8Array> {return renderReport(model,await loadFonts());}

/** XLSX cells are values, never formulas; numeric references remain text, including long codes. */
export async function renderExcel(model: ReportModel): Promise<Uint8Array> {
  const workbook = new ExcelJS.Workbook();
  workbook.creator = 'Al Mohtarif Repair Center';
  workbook.subject = 'Offline inventory report';
  workbook.title = model.title;
  for (const [tableIndex,table] of model.tables.entries()) {
  const sheet = workbook.addWorksheet(model.tables.length===1?'Inventory Report':`${tableIndex+1} ${table.title}`.slice(0,31), { views: [{ state: 'frozen', ySplit: 0 }], pageSetup: { paperSize: 9, orientation: 'landscape', fitToPage: true, fitToWidth: 1, fitToHeight: 0 } });
  const columnCount = table.headings.length;
  const addWide = (text: string, bold = false) => {
    const row = sheet.addRow([text]); sheet.mergeCells(row.number, 1, row.number, columnCount);
    row.getCell(1).alignment = { vertical: 'middle', wrapText: true };
    row.getCell(1).font = { name: 'Arial', size: bold ? 13 : 10, bold, color: { argb: 'FF35463A' } };
    row.height = Math.max(bold ? 27 : 22, Math.ceil(text.length / 130) * 16);
  };
  addWide(model.title, true); addWide(`Reporting period: ${model.period}`); addWide(`Generated: ${model.generatedAt}`);
  for (const note of model.notes) addWide(note);
  for (const value of model.summary) addWide(value, true);
  sheet.addRow([]);
    addWide(table.title, true);
    const header = sheet.addRow(table.headings.map(v => v.replace(/\n/g, ' ')));
    header.height = 35;
    header.eachCell(cell => {
      cell.fill = { type: 'pattern', pattern: 'solid', fgColor: { argb: 'FF355D45' } };
      cell.font = { name: 'Arial', bold: true, color: { argb: 'FFFFFFFF' }, size: 10 };
      cell.alignment = { vertical: 'middle', wrapText: true };
    });
    sheet.views = [{ state: 'frozen', ySplit: header.number }];
    if (!table.rows.length) addWide('No records match this report.');
    for (const [index, values] of table.rows.entries()) {
      const row = sheet.addRow(values.map((value, column) => table.numericColumns?.includes(column) && value !== 'Unknown' ? Number(value.replaceAll(',', '')) : value));
      row.height = Math.min(409,Math.max(24,...values.map((value,column)=>value.split('\n').reduce((lines,part)=>lines+Math.max(1,Math.ceil(part.length/(Math.max(12,Math.min(50,table.widths[column]/1.7))*.85))),0)*14+8)));
      row.eachCell((cell, column) => {
        cell.font = { name: 'Arial', size: 10, color: { argb: 'FF35463A' } };
        cell.alignment = { vertical: 'middle', wrapText: true, horizontal: table.numericColumns?.includes(column - 1) ? 'right' : 'left', readingOrder: /[\u0600-\u06ff]/.test(String(cell.value)) ? 'rtl' : 'ltr' };
        if (table.moneyColumns?.includes(column - 1)) cell.numFmt = '#,##0.000;[Red]-#,##0.000';
        else if (!table.numericColumns?.includes(column - 1)) cell.numFmt = '@';
        if (index % 2 === 0) cell.fill = { type: 'pattern', pattern: 'solid', fgColor: { argb: 'FFF2F5EE' } };
      });
    }
    sheet.autoFilter = { from: { row: header.number, column: 1 }, to: { row: header.number + table.rows.length, column: table.headings.length } };
    table.widths.forEach((width, index) => { sheet.getColumn(index + 1).width = Math.max(12, Math.min(50, width / 1.7)); });
  sheet.headerFooter.oddFooter = '&LAl Mohtarif Repair Center | OMR&RPage &P of &N';
  }
  return new Uint8Array(await workbook.xlsx.writeBuffer());
}
export async function buildExcelReport(type: ReportType, snapshot: ReportSnapshot, options: ReportOptions = {}): Promise<Uint8Array> {
  return renderExcel(createReportModel(type, snapshot, options));
}
