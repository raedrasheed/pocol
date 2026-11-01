document.getElementById('t').dataset.loaded='yes';
fetch('data.json').then(r=>r.json()).then(j=>{document.body.dataset.fetch=String(j.rows.length)});
const x=new XMLHttpRequest();x.open('GET','data.json');x.responseType='json';x.onload=()=>{document.body.dataset.xhr=String(x.response.rows.length)};x.send();
addEventListener('DOMContentLoaded',()=>{document.body.dataset.dcl='yes'});