"""Create a whitelisted, price-redacted display snapshot from an owner backup.
The original .db is opened read-only; no token or full database is bundled.
"""
import argparse,json,sqlite3,re
from datetime import datetime,timezone
from pathlib import Path
parser=argparse.ArgumentParser();parser.add_argument('backup',type=Path);parser.add_argument('--output',type=Path,default=Path('src-tauri/resources/limited/seed.json'));args=parser.parse_args()
source=args.backup.resolve();out=args.output.resolve();assert source!=out
product_sql="SELECT p.id,p.item_code,p.name,p.model_description,b.quantity FROM products p JOIN product_balances b ON b.product_id=p.id ORDER BY p.name,p.item_code"
line_sql="SELECT * FROM (SELECT l.id,l.product_id,s.created_at,CASE WHEN d.purpose IN ('personal','damage') THEN NULL ELSE s.cr_number END AS cr_number,p.name AS product_name,p.item_code,p.model_description,l.quantity,l.unit_price_baisa,COALESCE(d.purpose,'sale') AS purpose,COALESCE((SELECT json_extract(w.record_json,'$.original_cr') FROM legacy_archive w WHERE w.kind='warranty_link' AND json_extract(w.record_json,'$.replacement_line_id')=l.id LIMIT 1),'') AS original_cr FROM sale_lines l JOIN sales s ON s.id=l.sale_id JOIN products p ON p.id=l.product_id LEFT JOIN stock_out_details d ON d.line_id=l.id UNION ALL SELECT 'return:'||r.id AS id,l.product_id,r.resolved_at AS created_at,s.cr_number,p.name||' - '||r.resolution||' settlement ('||r.quantity||' returned)' AS product_name,p.item_code,p.model_description,0 AS quantity,NULL AS unit_price_baisa,'return_settlement' AS purpose,'' AS original_cr FROM damaged_returns r JOIN sale_lines l ON l.id=r.line_id JOIN sales s ON s.id=l.sale_id JOIN products p ON p.id=l.product_id WHERE r.resolved_at IS NOT NULL) ORDER BY created_at DESC,id DESC"
with sqlite3.connect(source.as_uri()+'?mode=ro',uri=True) as conn:
 assert conn.execute('PRAGMA integrity_check').fetchone()[0]=='ok';assert conn.execute('PRAGMA user_version').fetchone()[0]==3
 conn.row_factory=sqlite3.Row
 products=[dict(r) for r in conn.execute(product_sql)];lines=[dict(r) for r in conn.execute(line_sql)];entries=[dict(r) for r in conn.execute("SELECT e.id,e.product_id,p.item_code,p.name AS product_name,e.created_at,e.quantity,b.remaining_quantity FROM stock_entries e JOIN products p ON p.id=e.product_id JOIN entry_balances b ON b.id=e.id ORDER BY e.created_at DESC,e.rowid DESC")]
stamp=re.search(r'inventory-auto-(\d{8}T\d{6})\.(\d+)Z',source.name)
updated=datetime.strptime(stamp[1]+'.'+stamp[2][:6],'%Y%m%dT%H%M%S.%f').replace(tzinfo=timezone.utc).isoformat() if stamp else datetime.fromtimestamp(source.stat().st_mtime,timezone.utc).isoformat()
snapshot={'products':products,'sale_lines':lines,'entries':entries,'updated_at':updated}
assert all(set(p)=={'id','item_code','name','model_description','quantity'} for p in products)
assert all(set(p)=={'id','product_id','created_at','cr_number','product_name','item_code','model_description','quantity','unit_price_baisa','purpose','original_cr'} for p in lines)
assert all(set(e)=={'id','product_id','item_code','product_name','created_at','quantity','remaining_quantity'} for e in entries)
out.parent.mkdir(parents=True,exist_ok=True);out.write_text(json.dumps(snapshot,ensure_ascii=False,separators=(',',':')),encoding='utf-8')
print(json.dumps({'limited_products':len(products),'limited_stock_out_lines':len(lines),'output_bytes':out.stat().st_size,'original_database_unchanged':True,'hidden_financial_fields_bundled':False}))
