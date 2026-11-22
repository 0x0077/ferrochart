// Minimal syntax highlighting for the code drawer and the homepage code
// tabs: JavaScript, Rust, Python and shell. Output is HTML-escaped; only
// <span class="tok-*"> and <code> are added.

const KEYWORDS = {
  js: /^(?:const|let|var|await|async|new|import|from|export|default|type|return|if|else|for|of|while|true|false|null|undefined|typeof|function)$/,
  rust: /^(?:use|fn|let|mut|struct|impl|for|pub|mod|match|if|else|return|Ok|Err|Some|None|Self|self|dyn|Box|Result|true|false)$/,
  python: /^(?:import|from|as|def|return|if|else|for|in|with|class|True|False|None|lambda)$/,
  c: /^(?:include|struct|typedef|const|int|int32_t|int64_t|uint64_t|double|float|void|if|else|return|sizeof|NULL)$/,
  sh: /^(?:npm|cargo|pip|install|add)$/,
};
const COMMENT = { js: '\\/\\/[^\\n]*', rust: '\\/\\/[^\\n]*', python: '#[^\\n]*', c: '\\/\\/[^\\n]*', sh: '#[^\\n]*' };
const esc = (s) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

/** Highlights `src` written in `lang` ('js' by default, 'rust', 'python', 'sh'). */
export function highlight(src, lang = 'js') {
  const kw = KEYWORDS[lang] ?? KEYWORDS.js;
  const re = new RegExp(
    `(${COMMENT[lang] ?? COMMENT.js})|('(?:\\\\.|[^'\\\\\\n])*'|"(?:\\\\.|[^"\\\\])*"|\`(?:\\\\.|[^\`\\\\])*\`)|(\\b\\d[\\d_.]*\\b)|([A-Za-z_$][\\w$]*!?)(?=\\s*\\()|([A-Za-z_$][\\w$]*)`,
    'g',
  );
  let out = '';
  let last = 0;
  for (const m of src.matchAll(re)) {
    out += esc(src.slice(last, m.index));
    const [text, c, s, n, f, w] = m;
    if (c) out += `<span class="tok-c">${esc(c)}</span>`;
    else if (s) out += `<span class="tok-s">${esc(s)}</span>`;
    else if (n) out += `<span class="tok-n">${esc(n)}</span>`;
    else if (f) out += kw.test(f) ? `<span class="tok-k">${f}</span>` : `<span class="tok-f">${f}</span>`;
    else if (w) out += kw.test(w) ? `<span class="tok-k">${w}</span>` : esc(w);
    else out += esc(text);
    last = m.index + text.length;
  }
  return out + esc(src.slice(last));
}

/** Escapes prose and turns `backticks` into <code>. */
highlight.inlineCode = (text) => esc(text).replace(/`([^`]+)`/g, '<code>$1</code>');
