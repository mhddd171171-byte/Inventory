import { describe,it,expect } from 'vitest';
import { toBaisa, numericReference, positiveQuantity, matchesProduct, money, findProduct, batchBuyingTotal } from './utils';
import type { Product } from './types';
describe('manual inventory input and bilingual search',()=>{
  it('batch totals preserve exact buying amounts and unknown prices',()=>{
    expect(money(batchBuyingTotal({quantity:3,unit_cost_baisa:12345}))).toBe('OMR 37.035');
    expect(money(batchBuyingTotal({quantity:1000000,unit_cost_baisa:1000000000}))).toBe('OMR 1,000,000,000,000.000');
    expect(money(batchBuyingTotal({quantity:5,unit_cost_baisa:null}))).toBe('Unknown');
  });
  it('stores precise OMR and accepts Arabic digits without floating point drift',()=>{
    expect(toBaisa('12.345')).toBe(12345);expect(toBaisa('0.001')).toBe(1);expect(toBaisa('٠٫١')).toBe(100);
    for(const text of ['','-1','1.0001','1e5','NaN','1000000.001'])expect(()=>toBaisa(text)).toThrow();
    expect(money(null)).toBe('Unknown');expect(money(-1)).toBe('OMR -0.001');
  });
  it('requires nonzero numeric references and positive integer quantities',()=>{
    expect(numericReference('٠٠١١٢٣٤')).toBe('11234');expect(positiveQuantity('٢')).toBe(2);
    for(const code of ['','0','000','ABC','12.3','1'.repeat(31)])expect(()=>numericReference(code)).toThrow();
    for(const qty of ['','0','-1','1.5','1000001','1e3'])expect(()=>positiveQuantity(qty)).toThrow();
  });
  it('searches partial names, models and codes in both scripts',()=>{
    const p={id:'p',item_code:'11234',name:'شَاشة سامسونج',model_description:'Galaxy S3 GT-I9300'} as Product;
    for(const q of ['١١٢٣٤','شاشة','سامسونج','s3','gt-i9300','شاشة S3'])expect(matchesProduct(p,q)).toBe(true);
    expect(matchesProduct(p,'battery')).toBe(false);expect(findProduct([p],'0011234')).toBe(p);
  });
});
