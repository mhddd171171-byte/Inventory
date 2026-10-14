import { describe,expect,it } from 'vitest';
import { readFileSync,mkdirSync,writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import ExcelJS from 'exceljs';
import { createReportModel,formatBaisa,renderReport,renderExcel,type ReportSnapshot,type ReportType } from '../src/reports';
const now=new Date('2026-10-13T10:00:00Z');
const product={id:'p',item_code:'11234',name:'شاشة سامسونج S3',model_description:'Galaxy S3 GT-I9300',quantity:3,inventory_value_baisa:30000,unknown_cost_units:0,stock_status:'normal' as const,created_at:now.toISOString(),updated_at:now.toISOString()};
const line={id:'l',sale_id:'s',product_id:'p',item_code:'11234',product_name:product.name,model_description:product.model_description,cr_number:'99876',quantity:2,unit_price_baisa:15555,revenue_baisa:31110,cost_baisa:17001,profit_baisa:14109,created_at:now.toISOString()};
const snapshot:ReportSnapshot={month:'2026-10',products:[product],entries:[],sales:[],sale_lines:[line],monthly_sales_lines:[line],monthly_summary:[{product_id:'p',item_code:'11234',product_name:product.name,quantity_sold:2,quantity_remaining:3,cost_baisa:17001,revenue_baisa:31110,profit_baisa:14109}],low_stock:[],summary:{total_products:1,total_units:3,inventory_value_baisa:30000,known_inventory_value_baisa:30000,unknown_cost_units:0,monthly_units_sold:2,monthly_sales_baisa:31110,known_monthly_sales_baisa:31110,monthly_cost_baisa:17001,monthly_profit_baisa:14109,unknown_sales_lines:0}};
const fonts={regular:readFileSync(resolve('src/assets/NotoSansArabic-Regular.ttf')).toString('base64'),bold:readFileSync(resolve('src/assets/NotoSansArabic-Bold.ttf')).toString('base64')};
const types:ReportType[]=['monthly_sales','current_inventory','monthly_inventory_summary'];
describe('approved reports use recorded values',()=>{
  it('uses exact baisa and actual selling/FIFO cost values',()=>{
    const r=createReportModel('monthly_sales',snapshot,{},now);
    expect(r.tables[0].rows[0].slice(1)).toEqual(['11234',product.name,'2','99876','15.555','17.001','31.110','14.109']);
    expect(r.summary).toContain('Gross profit: OMR 14.109');
    expect(formatBaisa(9007199254740993n)).toBe('9,007,199,254,740.993');expect(()=>formatBaisa(.1)).toThrow();
  });
  it('rejects stale monthly data and labels remaining stock as current',()=>{
    expect(()=>createReportModel('monthly_sales',snapshot,{month:'2026-09'})).toThrow(/does not match/);
    expect(()=>createReportModel('monthly_sales',snapshot,{month:'2026-13'})).toThrow(/valid/);
    const r=createReportModel('monthly_inventory_summary',snapshot,{},now);
    expect(r.tables[0].rows[0].slice(2,4)).toEqual(['2','3']);expect(r.notes.join(' ')).toContain('current available balance');
  });
  it('never turns missing legacy costs/revenue into zero',()=>{
    const missing={...snapshot,products:[{...product,inventory_value_baisa:null,unknown_cost_units:3}],monthly_sales_lines:[{...line,cost_baisa:null,profit_baisa:null,unit_price_baisa:null,revenue_baisa:null,cr_number:null,legacy_reference:'RO-old'}],summary:{...snapshot.summary,inventory_value_baisa:null,unknown_cost_units:3,monthly_cost_baisa:null,monthly_profit_baisa:null,monthly_sales_baisa:null,unknown_sales_lines:1}};
    expect(createReportModel('current_inventory',missing).tables[0].rows[0][3]).toBe('Unknown');
    const r=createReportModel('monthly_sales',missing);expect(r.tables[0].rows[0].slice(5)).toEqual(['Unknown','Unknown','Unknown','Unknown']);expect(r.notes.join(' ')).toContain('never treated as zero');
  });
});
describe('real offline PDF and Excel files',()=>{
  for(const type of types) {
    it(`renders ${type} PDF with embedded Arabic fonts`,()=>{
      const bytes=renderReport(createReportModel(type,snapshot,{},now),fonts);const pdf=Buffer.from(bytes).toString('latin1');
      expect(pdf.startsWith('%PDF-')).toBe(true);expect(pdf).toContain('/FontFile2');expect(pdf).toContain('/ToUnicode');expect(pdf).toContain('%%EOF');
      if(process.env.REPORT_VISUAL_QA==='1'){mkdirSync('tmp/pdfs',{recursive:true});writeFileSync(`tmp/pdfs/${type}.pdf`,bytes);}
    });
    it(`renders ${type} Excel with text codes and numeric money cells`,async()=>{
      const model=createReportModel(type,snapshot,{},now);const bytes=await renderExcel(model);const workbook=new ExcelJS.Workbook();await workbook.xlsx.load(bytes as any);const sheet=workbook.worksheets[0];
      let found=false;sheet.eachRow(row=>{if(row.values.toString().includes(product.name)){found=true;expect(row.getCell(type==='monthly_sales'?2:1).value).toBe('11234');const moneyColumn=type==='monthly_sales'?6:type==='current_inventory'?4:5;expect(typeof row.getCell(moneyColumn).value).toBe('number');}});expect(found).toBe(true);
      if(process.env.REPORT_VISUAL_QA==='1'){mkdirSync('tmp/xlsx',{recursive:true});writeFileSync(`tmp/xlsx/${type}.xlsx`,bytes);}
    });
  }
  it('keeps spreadsheet-like names as literal text and long numeric references as text',async()=>{
    const s={...snapshot,products:[{...product,item_code:'123456789012345678901234567890',name:'=HYPERLINK("example")'}]};const bytes=await renderExcel(createReportModel('current_inventory',s));const wb=new ExcelJS.Workbook();await wb.xlsx.load(bytes as any);let found=false;
    wb.worksheets[0].eachRow(row=>{if(row.getCell(1).value==='123456789012345678901234567890'){found=true;expect(typeof row.getCell(2).value).toBe('string');expect(row.getCell(2).value).toContain('=HYPERLINK');}});expect(found).toBe(true);
  });
  it('paginates long Arabic records and provides an empty report',()=>{
    const expanded={...snapshot,monthly_sales_lines:Array.from({length:100},(_,i)=>({...line,id:`long-${i}`,product_name:product.name+' قطعة غيار لفحص الشاشة وتركيب المنتج'}))};const model=createReportModel('monthly_sales',expanded);expect(model.tables[0].rows).toHaveLength(100);const bytes=renderReport(model,fonts);expect((Buffer.from(bytes).toString('latin1').match(/\/Type \/Page\b/g)||[]).length).toBeGreaterThan(2);
    if(process.env.REPORT_VISUAL_QA==='1')writeFileSync('tmp/pdfs/multipage.pdf',bytes);
    const empty={...snapshot,products:[],monthly_sales_lines:[],monthly_summary:[]};for(const type of types)expect(renderReport(createReportModel(type,empty),fonts).length).toBeGreaterThan(1000);
  });
});
