import {writeFileSync} from 'node:fs';
const cases=[
 ['ascii','k','v',66],['delete','k',null,65],['empty','','',64],
 ['arabic','س','ع',68],['emoji','😀','𐐀',72],['combining','e\u0301','é',69],
 ['high-surrogate','\ud800','x',68],['low-surrogate','x','\udc00',68],
 ['surrogate-pair','\ud83d\ude00','x',69],['mixed','a\ud800b','\udc00',72],
 ['max','k'.repeat(256),'v'.repeat(61440),61760],
 ['BR21-M','k00','a'.repeat(61440),61507],['BR21-m','k00','x',68],
 ['BR21e-boundary','k50','a'.repeat(53905),53972],
];
const e=new TextEncoder();
const rows=cases.map(([id,key,value,expected])=>{const actual=64+e.encode(key).length+(value===null?0:e.encode(value).length);if(actual!==expected)throw Error(id+' hand expectation wrong '+actual);return {id,key,value,expected,actual};});
writeFileSync('D:/PoCol-Development/coordination/review-001/storequeue-utf8-oracle.json',JSON.stringify({oracle:'Installed Node TextEncoder, independent of author Python model',node:process.version,cases:rows},null,2));
console.log(JSON.stringify({cases:rows.length,passed:rows.length,node:process.version}));
