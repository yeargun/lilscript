export const ChildProperties = new Set(["innerHTML","textContent","innerText","children"])
export const DOMElements = new Set(["a","abbr","acronym","address","applet","area","article","aside","audio","b","base","basefont","bdi","bdo","bgsound","big","blink","blockquote","body","br","button","canvas","caption","center","cite","code","col","colgroup","content","data","datalist","dd","del","details","dfn","dialog","dir","div","dl","dt","em","embed","fieldset","figcaption","figure","font","footer","form","frame","frameset","h1","h2","h3","h4","h5","h6","head","header","hgroup","hr","html","i","iframe","image","img","input","ins","isindex","kbd","keygen","label","legend","li","link","listing","main","map","mark","marquee","math","menu","menuitem","meta","meter","multicol","nav","nextid","nobr","noembed","noframes","noindex","noscript","object","ol","optgroup","option","output","p","param","picture","plaintext","portal","pre","progress","q","rb","rp","rt","rtc","ruby","s","samp","script","search","section","select","shadow","slot","small","source","spacer","span","strike","strong","style","sub","summary","sup","svg","table","tbody","td","template","textarea","tfoot","th","thead","time","title","tr","track","tt","u","ul","var","video","wbr","webview","xmp"])
export const DelegatedEvents = new Set(["beforeinput","click","dblclick","contextmenu","focusin","focusout","input","keydown","keyup","mousedown","mousemove","mouseout","mouseover","mouseup","pointerdown","pointermove","pointerout","pointerover","pointerup","touchend","touchmove","touchstart"])
export const SVGElements = new Set(["altGlyph","altGlyphDef","altGlyphItem","animate","animateColor","animateMotion","animateTransform","circle","clipPath","color-profile","cursor","defs","desc","ellipse","feBlend","feColorMatrix","feComponentTransfer","feComposite","feConvolveMatrix","feDiffuseLighting","feDisplacementMap","feDistantLight","feDropShadow","feFlood","feFuncA","feFuncB","feFuncG","feFuncR","feGaussianBlur","feImage","feMerge","feMergeNode","feMorphology","feOffset","fePointLight","feSpecularLighting","feSpotLight","feTile","feTurbulence","filter","font","font-face","font-face-format","font-face-name","font-face-src","font-face-uri","foreignObject","g","glyph","glyphRef","hkern","image","line","linearGradient","marker","mask","metadata","missing-glyph","mpath","path","pattern","polygon","polyline","radialGradient","rect","set","stop","svg","switch","symbol","text","textPath","tref","tspan","use","view","vkern"])
export const MathMLElements = new Set(["annotation","annotation-xml","maction","math","menclose","merror","mfenced","mfrac","mi","mmultiscripts","mn","mo","mover","mpadded","mphantom","mprescripts","mroot","mrow","ms","mspace","msqrt","mstyle","msub","msubsup","msup","mtable","mtd","mtext","mtr","munder","munderover","semantics"])
export const VoidElements = new Set(["area","base","br","col","embed","hr","img","input","link","meta","param","source","track","wbr"])
export const RawTextElements = new Set(["style","script","noscript","template","textarea","title"])
export const Namespaces = {
  "svg": "http://www.w3.org/2000/svg",
  "mathml": "http://www.w3.org/1998/Math/MathML",
  "xlink": "http://www.w3.org/1999/xlink",
  "xml": "http://www.w3.org/XML/1998/namespace"
}
export const DOMWithState = {
  "INPUT": {
    "value": 1,
    "defaultValue": 2,
    "checked": 1,
    "defaultChecked": 2
  },
  "SELECT": {
    "value": 1
  },
  "OPTION": {
    "value": 1,
    "selected": 1,
    "defaultSelected": 2
  },
  "TEXTAREA": {
    "value": 1,
    "defaultValue": 2
  },
  "VIDEO": {
    "muted": 1,
    "defaultMuted": 2
  },
  "AUDIO": {
    "muted": 1,
    "defaultMuted": 2
  }
}
