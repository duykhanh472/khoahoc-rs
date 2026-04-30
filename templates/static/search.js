// Client-side search using the pre-built search-index.json
(function () {
  const toggle = document.getElementById('search-toggle');
  const overlay = document.getElementById('search-overlay');
  const closeBtn = document.getElementById('search-close');
  const input = document.getElementById('search-input');
  const results = document.getElementById('search-results');

  if (!toggle || !overlay) return;

  let index = null;

  // Load index lazily on first open
  async function loadIndex() {
    if (index) return;
    try {
      const res = await fetch('/search-index.json');
      index = await res.json();
    } catch (e) {
      console.error('Could not load search index', e);
      index = [];
    }
  }

  function open() {
    overlay.hidden = false;
    toggle.setAttribute('aria-expanded', 'true');
    input.focus();
    loadIndex();
  }

  function close() {
    overlay.hidden = true;
    toggle.setAttribute('aria-expanded', 'false');
    input.value = '';
    results.innerHTML = '';
  }

  toggle.addEventListener('click', open);
  closeBtn.addEventListener('click', close);

  // Close on backdrop click
  overlay.addEventListener('click', e => { if (e.target === overlay) close(); });

  // Close on Escape
  document.addEventListener('keydown', e => {
    if (e.key === 'Escape' && !overlay.hidden) close();
    if ((e.key === 'k' && (e.metaKey || e.ctrlKey))) { e.preventDefault(); open(); }
  });

  // Search logic
  let debounceTimer;
  input.addEventListener('input', () => {
    clearTimeout(debounceTimer);
    debounceTimer = setTimeout(runSearch, 150);
  });

  function runSearch() {
    const q = input.value.trim().toLowerCase();
    results.innerHTML = '';
    if (!q || !index) return;

    const matches = index.filter(entry =>
      entry.title.toLowerCase().includes(q) ||
      entry.excerpt.toLowerCase().includes(q) ||
      entry.course.toLowerCase().includes(q)
    ).slice(0, 10);

    if (matches.length === 0) {
      results.innerHTML = '<p style="padding:16px 20px;color:var(--text-muted);font-size:.875rem;">No results found.</p>';
      return;
    }

    matches.forEach(entry => {
      const a = document.createElement('a');
      a.className = 'search-result-item';
      a.href = entry.url;
      a.innerHTML = `
        <div class="search-result-title">${escapeHtml(entry.title)}</div>
        <div class="search-result-meta">${escapeHtml(entry.path)} › ${escapeHtml(entry.course)}</div>
        <div class="search-result-excerpt">${escapeHtml(entry.excerpt)}</div>
      `;
      a.addEventListener('click', close);
      results.appendChild(a);
    });
  }

  function escapeHtml(str) {
    return str.replace(/[&<>"']/g, c => ({ '&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;' }[c]));
  }
})();
