import { describe,it,expect } from 'vitest';
import { readFileSync,mkdirSync,writeFileSync } from 'node:fs';
import ExcelJS from 'exceljs';
import { emptySearch,searchSnapshot,searchSuggestions } from '../src/search';
import { createSearchReportModel,renderExcel,renderReport } from '../src/reports';
import type { Snapshot,Product,SaleLine } from '../src/types';
const date='2026-10-13T10:00:00Z';
const product=(id:string,code:string,name:string,model:string):Product=>({id,item_code:code,name,model_description:model,quantity:3,inventory_value_baisa:30000,unknown_cost_units:0,stock_status:'normal',created_at:date,updated_at:date});
const products=[product('a','11234','شاشة سامسونج','Galaxy S3 GT-I9300'),product('b','11235','iPhone battery','iPhone 14')];
const line=(id:string,p:Product,cr:string,stamp=date):SaleLine=>({id,sale_id:id,product_id:p.id,item_code:p.item_code,product_name:p.name,model_description:p.model_description,cr_number:cr,quantity:2,unit_price_baisa:15555,revenue_baisa:31110,cost_baisa:17001,profit_baisa:14109,created_at:stamp});
const snapshot:Snapshot={month:'2026-10',products,entries:[{id:'e',product_id:'a',item_code:'11234',product_name:products[0].name,quantity:5,remaining_quantity:3,unit_cost_baisa:10000,created_at:date}],sale_lines:[line('l1',products[0],'99876'),line('l2',products[1],'99876'),line('l3',products[0],'12345'),line('l4',products[0],'99876','2026-09-03T10:00:00Z')],sales:[],monthly_sales_lines:[],monthly_summary:[],low_stock:[],summary:{total_products:2,total_units:6,inventory_value_baisa:60000,known_inventory_value_baisa:60000,unknown_cost_units:0,monthly_units_sold:0,monthly_sales_baisa:0,known_monthly_sales_baisa:0,monthly_cost_baisa:0,monthly_profit_baisa:0,unknown_sales_lines:0}};
describe('workspace record search',()=>{
  it('CR and item filters match the same sale line across repeated transactions and months',()=>{
    const before=JSON.stringify(snapshot);const result=searchSnapshot(snapshot,{...emptySearch,cr:'٩٩٨٧٦',itemCode:'011234'});
    expect(result.sales.map(l=>l.id)).toEqual(['l1','l4']);expect(result.products.map(p=>p.id)).toEqual(['a']);expect(result.entries).toEqual([]);expect(JSON.stringify(snapshot)).toBe(before);
  });
  it('partial Arabic names, product models and all-reference queries work without a code',()=>{
    expect(searchSnapshot(snapshot,{...emptySearch,query:'شَاشَة سَامْسُونْج'}).sales).toHaveLength(3);
    expect(searchSnapshot(snapshot,{...emptySearch,query:'GT-I9300'}).entries).toHaveLength(1);
    expect(searchSnapshot(snapshot,{...emptySearch,query:'99876'}).sales).toHaveLength(3);
    expect(searchSnapshot(snapshot,{...emptySearch,query:'battery'}).products[0].id).toBe('b');
  });
  it('date ranges are inclusive, invalid references/dates fail and no match is honest',()=>{
    expect(searchSnapshot(snapshot,{...emptySearch,from:'2026-10-13',to:'2026-10-13'}).sales).toHaveLength(3);
    expect(()=>searchSnapshot(snapshot,{...emptySearch,from:'2026-02-30'})).toThrow(/valid date/);
    expect(()=>searchSnapshot(snapshot,{...emptySearch,from:'2026-10-14',to:'2026-10-13'})).toThrow(/before/);
    expect(()=>searchSnapshot(snapshot,{...emptySearch,cr:'0'})).toThrow(/nonzero/);
    expect(searchSnapshot(snapshot,{...emptySearch,cr:'999999'}).sales).toEqual([]);
    expect(searchSnapshot(snapshot,{...emptySearch,query:"' OR 1=1; DROP TABLE products"}).products).toEqual([]);
  });
  it('suggestions are actual distinct saved values, including empty inventory',()=>{
    expect(searchSuggestions(snapshot).cr).toEqual(['99876','12345']);expect(searchSuggestions(snapshot).codes).toEqual(['11234','11235']);
    expect(searchSuggestions({...snapshot,products:[],sale_lines:[]}).cr).toEqual([]);
  });
  it('incoming exports show actual buying price and original batch total, not remaining-stock value',async()=>{
    const report=createSearchReportModel(searchSnapshot(snapshot,{...emptySearch,itemCode:'11234'}));
    expect(report.tables[2].rows[0].slice(3)).toEqual(['5','3','10.000','50.000']);
    const unknown={...snapshot,entries:[{...snapshot.entries[0],unit_cost_baisa:null}]};
    expect(createSearchReportModel(searchSnapshot(unknown,emptySearch)).tables[2].rows[0].slice(-2)).toEqual(['Unknown','Unknown']);
    const workbook=new ExcelJS.Workbook();await workbook.xlsx.load(await renderExcel(report));
    let found=false;workbook.worksheets[2].eachRow(row=>{if(row.getCell(2).value==='11234'){found=true;expect(row.getCell(6).value).toBe(10);expect(row.getCell(7).value).toBe(50);}});expect(found).toBe(true);
    if(process.env.REPORT_VISUAL_QA==='1'){
      const fonts={regular:readFileSync('src/assets/NotoSansArabic-Regular.ttf').toString('base64'),bold:readFileSync('src/assets/NotoSansArabic-Bold.ttf').toString('base64')};
      mkdirSync('tmp/pdfs',{recursive:true});writeFileSync('tmp/pdfs/buying-prices-v2.0.3.pdf',renderReport(report,fonts));
    }
  });
  it('exports exactly the filtered records and exact baisa totals; unknown cost remains unknown',async()=>{
    const result=searchSnapshot(snapshot,{...emptySearch,cr:'99876',itemCode:'11234'});const model=createSearchReportModel(result,new Date(date));
    expect(model.tables[0].rows).toHaveLength(2);expect(model.summary).toContain('Sales revenue: OMR 62.220');expect(model.summary).toContain('Gross profit: OMR 28.218');
    const workbook=new ExcelJS.Workbook();await workbook.xlsx.load(await renderExcel(model));expect(workbook.worksheets).toHaveLength(2);
    const sales=workbook.worksheets[0];let seen=0;sales.eachRow(row=>{if(row.getCell(2).value==='99876'){seen++;expect(row.getCell(3).value).toBe('11234');expect(row.getCell(6).value).toBe(15.555);}});expect(seen).toBe(2);
    const unknown=createSearchReportModel({...result,sales:[{...result.sales[0],cost_baisa:null,profit_baisa:null}]});expect(unknown.summary).toContain('Buying cost of sold units: Unknown');expect(unknown.tables[0].rows[0][6]).toBe('Unknown');
    const fonts={regular:readFileSync('src/assets/NotoSansArabic-Regular.ttf').toString('base64'),bold:readFileSync('src/assets/NotoSansArabic-Bold.ttf').toString('base64')};
    mkdirSync('tmp/pdfs',{recursive:true});writeFileSync('tmp/pdfs/search-results.pdf',renderReport(model,fonts));
    expect(createSearchReportModel(searchSnapshot({...snapshot,products:[],sale_lines:[],entries:[]},emptySearch)).summary).toContain('Sales revenue: OMR 0.000');
  });
});
