const fnExt = fnRenderer({
  compile: (src, widthPt) => chrome.runtime.sendMessage({ type: 'functor-compile', src, widthPt }),
  css: fetch(chrome.runtime.getURL('doc.css')).then((r) => r.text()),
  fontUrl: (f) => chrome.runtime.getURL(`fonts/${f}.woff2`),
  toolbar: true,
  labels: true,
  zoom: 1.1,
});

fnExt.observe(document.body);

chrome.storage.local.get('enabled').then((v) => fnExt.setEnabled(v.enabled !== false));
chrome.storage.onChanged.addListener((c) => {
  if (c.enabled) fnExt.setEnabled(c.enabled.newValue !== false);
});
